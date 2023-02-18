use crate::{artifacts::{ArtifactRepr, ArtifactNodeRepr}, resolver::inputs::{InputResolver, NO_INPUTS_FN, NO_VALUES_FN}};
use std::{env::current_dir};
use crate::utils::{run_command_in_user_shell, buildstate_path_or_create};
use indexmap::IndexSet;

pub struct StackInitializer<'a> {
    artifact: &'a ArtifactRepr,
    initialized: IndexSet<String>,
}

impl<'a> StackInitializer<'a> {
    pub fn new(artifact: &'a ArtifactRepr) -> StackInitializer {
        StackInitializer {
            artifact: artifact,
            initialized: IndexSet::new(),
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

    fn copy_required_files(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        let node_file_path = std::path::Path::new(&node.file_path);
        let node_dir = node_file_path.parent().unwrap();

        let files = node.files.clone().unwrap_or_default();

        for file in files {
            let file_path = node_dir.join(file);

            if current_dir()?.join(file_path.clone()).exists() {
                let file_name = file_path.file_name().unwrap();
                let dest_path = current_dir()?.join(file_name);
                
                std::fs::copy(file_path, dest_path)?;
            }
        }

        println!("{}", node.file_path);
        Ok(())
    }

    fn initalize_node(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        self.copy_required_files(node)?;

        if node.init_step.is_some() {
            let (_, _, resolved_steps) = InputResolver::resolve(node, NO_VALUES_FN, NO_INPUTS_FN, Some(true))?;

            let script = resolved_steps.unwrap().join(";");

            run_command_in_user_shell(script, Some("/bin/bash".to_string()))?;
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