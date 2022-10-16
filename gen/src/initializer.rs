use crate::artifacts::{ArtifactRepr, ArtifactNodeRepr};
use std::collections::HashSet;
use crate::utils::{run_command_in_user_shell, torb_path};

pub struct StackInitializer<'a> {
    artifact: &'a ArtifactRepr,
    initialized: HashSet<String>,
}

impl<'a> StackInitializer<'a> {
    pub fn new(artifact: &'a ArtifactRepr) -> StackInitializer {
        StackInitializer {
            artifact: artifact,
            initialized: HashSet::new(),
        }
    }

    pub fn run_node_init_steps(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !std::path::Path::new("./.stack_initilized").exists() {
            for node in self.artifact.deploys.iter() {
                self.walk_artifact(node)?;
            }

            std::fs::write("./.stack_initialized", "")?;
        }

        Ok(())
    }

    fn initalize_node(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(step) = node.init_step.clone() {
            let script_contents = std::fs::read_to_string(step.script)?;

            run_command_in_user_shell(script_contents)?;
        };

        Ok(())
    }

    fn walk_artifact(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we build. 
        // This is because duplicate dependencies can exist, and we want to avoid building the same thing twice.
        // By walking to the end we ensure that whichever copy is built first will be in the set of seen nodes.
        // This let me avoid worrying about how to handle duplicate dependencies in the dependency tree data structure.
        // -Ian
        for child in node.dependencies.iter() {
            self.walk_artifact(child)?
        }

        if !self.initialized.contains(&node.fqn) {
            self.initalize_node(&node).and_then(|_out| {
                if self.initialized.insert(node.fqn.clone()) {
                    Ok(())
                } else {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Step already initialized.",
                    )))
                }
            })?;
        }

        Ok(())
    }
}