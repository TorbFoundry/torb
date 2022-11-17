use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use crate::utils::{run_command_in_user_shell, CommandConfig, CommandPipeline};
use std::collections::HashSet;
use std::fs;
use std::process::{Command, Output};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbBuilderErrors {
    #[error("Unable to build from dockerfile, reason: {response:?}")]
    UnableToBuildDockerfile { response: String },
    #[error("Unable to build from build script, reason: {response:?}")]
    UnableToBuildBuildScript { response: String },
    #[error("Either dockerfile or script_path must be provided.")]
    MustDefineDockerfileOrBuildScript,
    #[error("The node has already been built. This theoretically should never be hit, so please ping the maintainers.")]
    NodeAlreadyBuilt,
}

pub struct StackBuilder<'a> {
    artifact: &'a ArtifactRepr,
    built: HashSet<String>,
    dryrun: bool,
}

impl<'a> StackBuilder<'a> {
    pub fn new(artifact: &'a ArtifactRepr, dryrun: bool) -> StackBuilder<'a> {
        StackBuilder {
            artifact: artifact,
            built: HashSet::new(),
            dryrun: dryrun,
        }
    }

    pub fn build(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for node in self.artifact.deploys.iter() {
            self.walk_artifact(node)?;
        }

        Ok(())
    }

    fn build_node(&self, node: &ArtifactNodeRepr) -> Result<(), TorbBuilderErrors> {
        if let Some(step) = node.build_step.clone() {
            if step.dockerfile != "" {
                self.build_docker(step.dockerfile, step.tag, step.registry)
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
        dockerfile: String,
        tag: String,
        registry: String,
    ) -> Result<Vec<Output>, TorbBuilderErrors> {
        let current_dir = std::env::current_dir().unwrap();

        let mut commands = vec![CommandConfig::new(
            "docker",
            vec!["build", ".", "-f", &dockerfile, "-t", &tag],
            Some(&current_dir.to_str().unwrap()),
        )];

        if registry != "" {
            commands.push(CommandConfig::new(
                "docker",
                vec!["push", &registry, &tag],
                Some(&current_dir.to_str().unwrap()),
            ));
        }

        if self.dryrun {
            println!("{:?}", commands);

            Ok(vec![])
        } else {
            let mut pipeline = CommandPipeline::new(Some(commands));

            pipeline
                .execute()
                .map_err(|err| TorbBuilderErrors::UnableToBuildDockerfile {
                    response: err.to_string(),
                })
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

            let script_string = lines.join(";");

            run_command_in_user_shell(script_string).map_err(|err| {
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
            self.walk_artifact(child)?
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
