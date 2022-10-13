use crate::artifacts::{ArtifactRepr, ArtifactNodeRepr};
use std::collections::HashSet;

struct StackBuilder<'a> {
    artifact: &'a ArtifactRepr,
    built: HashSet<String>,
    dryrun: bool
}

impl<'a> StackBuilder<'a> {
    fn new(artifact: &'a ArtifactRepr, dryrun: bool) -> StackBuilder<'a> {
        StackBuilder {
            artifact: artifact,
            built: HashSet::new(),
            dryrun: dryrun
        }
    }

    fn run_node_build_steps(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        for node in self.artifact.deploys.iter() {
            self.walk_artifact(node)?;
        }

        Ok("".to_string())
    }

    fn build_node(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
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

        if !self.built.contains(&node.fqn) {
            self.build_node(&node).and_then(|_out| {
                if self.built.insert(node.fqn.clone()) {
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
