use crate::artifacts::{ArtifactRepr, ArtifactNodeRepr};
use std::{collections::HashSet, env::current_dir};
use crate::utils::{run_command_in_user_shell, buildstate_path_or_create, torb_path};

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

        if let Some(steps) = node.init_step.clone() {
            let resolved_steps = steps.iter().map(|step| {
                self.resolve_torb_value_interpolation(step, node)
            }).collect::<Vec<String>>();
            
            let script = resolved_steps.join(";");

            run_command_in_user_shell(script)?;
        };

        Ok(())
    }

    /*
        Case 1: Token at start
            Remaining = anything after token
        Case 2: Token in middle
            Remaining = anything before or after token
        Case 3: Token at end
            Remaining = anything before token
     */
    fn resolve_torb_value_interpolation(&self, script_step: &String, node: &ArtifactNodeRepr) -> String {
        let start_option: Option<usize> = script_step.find(TOKEN);
        match start_option {
            Some(start) => {
                let end = script_step.split_at(start).1.find(" ").unwrap_or(script_step.len());

                println!("Script Step: {}", script_step);
                println!("Default End: {}", script_step.len() - start);
                println!("Start: {}, End: {}", start, end);
                println!("script_step_len: {}", script_step.len());

                let remaining = if start == 0 && end == script_step.len() {
                    let resolved_token = self.resolve_input_token(script_step.to_string(), node);

                    resolved_token
                } else if end == script_step.len() {
                    let parts = script_step.split_at(start);
                    let resolved_token = self.resolve_input_token(parts.1.to_string(), node);
                    format!("{}{}", parts.0.to_string(), resolved_token)
                } else if start == 0 {
                    let parts = script_step.split_at(end);
                    let resolved_token = self.resolve_input_token(parts.0.to_string(), node);
                    format!("{}{}", resolved_token, parts.1.to_string())
                } else {
                    let parts = script_step.split_at(start);
                    let remaining_1 = parts.0.to_string();
                    let parts = parts.1.split_at(end);
                    let token = parts.0.to_string();
                    let remaining_2 = parts.1.to_string();

                    let resolved_token = self.resolve_input_token(token, node);

                    format!("{}{}{}", remaining_1, resolved_token, remaining_2)
                };

                println!("remaining: {}", remaining);
                self.resolve_torb_value_interpolation(&remaining.to_string(), node)
            },
            None => {
                script_step.clone()
            }
        }
    }

    fn resolve_input_token(&self, token: String, node: &ArtifactNodeRepr) -> String {
        let input = token.split("TORB.inputs.").collect::<Vec<&str>>()[1];
        println!("input: {}", input);
        let (_, val) = node.mapped_inputs.get(input).unwrap();

        val.clone()
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