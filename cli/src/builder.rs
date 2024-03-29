// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.7-03.23
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use crate::utils::{run_command_in_user_shell, CommandConfig, CommandPipeline};
use indexmap::{IndexSet};
use std::fs;
use std::process::{Command, Output};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbBuilderErrors {
    #[error("Unable to build from dockerfile, reason: {response}")]
    UnableToBuildDockerfile { response: String },
    #[error("Unable to build from build script, reason: {response}")]
    UnableToBuildBuildScript { response: String },
    #[error("Either dockerfile or script_path must be provided.")]
    MustDefineDockerfileOrBuildScript,
    #[error("The node has already been built. This theoretically should never be hit, so please ping the maintainers.")]
    NodeAlreadyBuilt,
}

pub struct StackBuilder<'a> {
    artifact: &'a ArtifactRepr,
    built: IndexSet<String>,
    dryrun: bool,
    build_platforms: String,
    separate_local_registry: bool,
    exempt: std::collections::HashSet<String>,
}

impl<'a> StackBuilder<'a> {
    pub fn new(
        artifact: &'a ArtifactRepr,
        build_platforms: String,
        dryrun: bool,
        separate_local_registry: bool,
    ) -> StackBuilder<'a> {
        StackBuilder {
            artifact: artifact,
            built: IndexSet::new(),
            dryrun: dryrun,
            build_platforms: build_platforms,
            separate_local_registry,
            exempt: std::collections::HashSet::new(),
        }
    }

    pub fn new_with_exempt_list(
        artifact: &'a ArtifactRepr,
        build_platforms: String,
        dryrun: bool,
        separate_local_registry: bool,
        exempt: Vec<String>
    ) -> StackBuilder<'a> {
        StackBuilder {
            artifact: artifact,
            built: IndexSet::new(),
            dryrun: dryrun,
            build_platforms: build_platforms,
            separate_local_registry,
            exempt: std::collections::HashSet::from_iter(exempt.iter().cloned()),
        }
    }

    pub fn build(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for node in self.artifact.deploys.iter() {
            if self.exempt.get(&node.fqn).is_none() {
                self.walk_artifact(node)?;
            }
        }

        Ok(())
    }

    fn build_node(&self, node: &ArtifactNodeRepr) -> Result<(), TorbBuilderErrors> {
        if let Some(step) = node.build_step.clone() {
            if step.dockerfile != "" {
                let name = node.display_name(false);

                self.build_docker(&name, step.dockerfile, step.tag, step.registry)
                    .and_then(|_| Ok(()))
            } else if step.script_path != "" {
                self.build_script(step.script_path).and_then(|_| Ok(()))
            } else {
                Err(TorbBuilderErrors::MustDefineDockerfileOrBuildScript)
            }
        } else {
            Ok(())
        }
    }

    fn build_docker(
        &self,
        name: &str,
        dockerfile: String,
        tag: String,
        registry: String,
    ) -> Result<Vec<Output>, TorbBuilderErrors> {
        let current_dir = std::env::current_dir().unwrap();
        let dockerfile_dir = current_dir.join(name);

        let label = if registry != "local" && registry != "" {
            format!("{}/{}:{}", registry, name, tag)
        } else {
            format!("{}:{}", name, tag)
        };
        // Todo(Ian): Refactor this to not be so ugly when you feel like dealing with the lifetimes. 
        let commands = if registry != "local" {
            if self.separate_local_registry {
                vec![
                    CommandConfig::new(
                        "docker",
                        vec![
                            "buildx",
                            "--builder",
                            "default",
                            "build",
                            "-t",
                            &label,
                            ".",
                            "-f",
                            &dockerfile,
                            "--push"
                        ],
                        Some(&dockerfile_dir.to_str().unwrap()),
                    ),
                ]
            } else {
                vec![
                    CommandConfig::new(
                        "docker",
                        vec![
                            "buildx",
                            "--builder",
                            "torb_builder",
                            "build",
                            "--platform",
                            &self.build_platforms,
                            "-t",
                            &label,
                            ".",
                            "-f",
                            &dockerfile,
                            "--push"
                        ],
                        Some(&dockerfile_dir.to_str().unwrap()),
                    ),
                ]
            }
        } else {
            vec![CommandConfig::new(
                "docker",
                vec![
                    "buildx",
                    "--builder",
                    "torb_builder",
                    "build",
                    "-t",
                    &label,
                    ".",
                    "-f",
                    &dockerfile,
                    "--load",
                ],
                Some(&dockerfile_dir.to_str().unwrap()),
            )]
        };

        if self.dryrun {
            println!("{:?}", commands);

            Ok(vec![])
        } else {
            let mut pipeline = CommandPipeline::new(Some(commands));

            let out = pipeline
                .execute()
                .map_err(|err| TorbBuilderErrors::UnableToBuildDockerfile {
                    response: err.to_string(),
                });

            out
        }
    }

    fn build_script(&self, script_path: String) -> Result<Output, TorbBuilderErrors> {
        let contents = fs::read_to_string(script_path).unwrap();

        if self.dryrun {
            println!("{:?}", contents);

            let out = Command::new("")
                .output()
                .expect("Failed to run nop command for build script dryrun.");

            Ok(out)
        } else {
            let lines: Vec<&str> = contents.split("\n").collect();

            let script_string = lines.join("&&");

            run_command_in_user_shell(script_string, None).map_err(|err| {
                TorbBuilderErrors::UnableToBuildBuildScript {
                    response: err.to_string(),
                }
            })
        }
    }

    fn walk_artifact(&mut self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we build.
        // This is because duplicate dependencies can exist, and we want to avoid building the same thing twice.
        // By walking to the end we ensure that whichever copy is built first will be in the set of seen nodes.
        // This let me avoid worrying about how to handle duplicate dependencies in the dependency tree data structure.
        // -Ian
        for child in node.dependencies.iter() {
            if self.exempt.get(&child.fqn).is_none() {
                self.walk_artifact(child)?
            }
        }

        if !self.built.contains(&node.fqn) {
            self.build_node(&node).and_then(|_out| {
                if self.built.insert(node.fqn.clone()) {
                    Ok(())
                } else {
                    Err(TorbBuilderErrors::NodeAlreadyBuilt)
                }
            })?;
        }

        Ok(())
    }
}
