use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use std::collections::HashSet;
use thiserror::Error;
use std::process::Command;

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
    #[error("Failed to build dockerfile. Reason: {reason:?}")]
    FailedDockerBuild { reason: String }
}

struct StackBuilder<'a> {
    artifact: &'a ArtifactRepr,
    built: HashSet<String>,
    dryrun: bool,
}

impl<'a> StackBuilder<'a> {
    fn new(artifact: &'a ArtifactRepr, dryrun: bool) -> StackBuilder<'a> {
        StackBuilder {
            artifact: artifact,
            built: HashSet::new(),
            dryrun: dryrun,
        }
    }

    fn build_node(&self, node: &ArtifactNodeRepr) -> Result<(), TorbBuilderErrors> {
        if let Some(step) = node.build_step.clone() {
            if step.dockerfile != "" {
                self.build_docker(step.dockerfile, step.tag, step.registry)
            } else if step.script_path != "" {
                self.build_script(step.script_path)
            } else {
                Err(TorbBuilderErrors::MustDefineDockerfileOrBuildScript)
            }
        } else {
            Ok(())
        }
    }

    fn build_docker(&self, dockerfile: String, tag: String, registry: String) -> Result<(), TorbBuilderErrors> {
        let command = Command::new("docker")
            .arg("build")
            .arg(dockerfile)
            .arg("-t")
            .arg(tag)
            .output()
            .map_err(|err| {
                                
            });

        

        Ok(())
    }

    fn build_script(&self, script_path: String) -> Result<(), TorbBuilderErrors> {
        Ok(())
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
