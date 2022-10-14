use crate::artifacts::{ArtifactRepr, ArtifactNodeRepr};
use std::collections::HashSet;

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

    pub fn run_node_init_steps(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        for node in self.artifact.deploys.iter() {
            self.walk_artifact(node)?;
        }

        Ok("".to_string())
    }

    fn initalize_node(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(step) = node.init_step.clone() {
            ()
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
                        "Step already built.",
                    )))
                }
            })?;
        }

        Ok(())
    }
}