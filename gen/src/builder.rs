use crate::artifacts::{ArtifactRepr, ArtifactNodeRepr};
use std::collections::HashSet;

struct StackBuilder<'a> {
    artifact: &'a ArtifactRepr,
    built: HashSet<String>,
    dryrun: bool
}

impl<'a> StackBuilder<'a> {
    fn build_node(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn walk_stack_root(
        stack: &ArtifactRepr
    ) {

    }

    fn walk_build_path(
        &self,
        node: &ArtifactNodeRepr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we build.
        for child in node.dependencies.iter() {
            self.walk_build_path(child)?
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
