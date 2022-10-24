use crate::artifacts::{ArtifactRepr, ArtifactNodeRepr};
use std::collections::HashSet;
use crate::utils::{run_command_in_user_shell, buildstate_path_or_create};

const TOKEN: &str = "TORB";

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
        let buildstate_path = buildstate_path_or_create();
        let init_canary_path = buildstate_path.join(".stack_initialized");

        if !init_canary_path.exists() {
            for node in self.artifact.deploys.iter() {
                self.walk_artifact(node)?;
            }

            std::fs::write(init_canary_path, "")?;
        } else {
            println!("Stack has already been initialized, skipping.")
        }

        Ok(())
    }

    fn initalize_node(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(steps) = node.init_step.clone() {
            let resolved_steps = steps.iter().map(|step| {
                self.resolve_torb_value_interpolation(step)
            }).collect::<Vec<String>>();
            
            let script = resolved_steps.join(";");

            run_command_in_user_shell(script)?;
        };

        Ok(())
    }

    fn resolve_torb_value_interpolation(&self, script_step: &String) -> String {
        let start = script_step.find(TOKEN).unwrap_or(0);
        let end = script_step.split_at(start).1.find(" ").unwrap_or(script_step.len() - start);
        let token = script_step.split_at(start).1.split_at(end).0;
        let remaining = script_step.split_at(end).1;

        let remaining = self.resolve_torb_value_interpolation(&remaining.to_string());

        "".to_string()
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