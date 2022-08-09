use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use serde::{Deserialize, Serialize};
use serde_yaml::{self};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::process::Command;
use thiserror::Error;

const TORB_PATH: &str = ".torb";

fn torb_path() -> std::path::PathBuf {
    let home_dir = dirs::home_dir().unwrap();
    home_dir.join(TORB_PATH)
}

pub struct StackBuilder {
    built: HashSet<String>,
}

impl StackBuilder {
    pub fn new() -> StackBuilder {
        StackBuilder {
            built: HashSet::new(),
        }
    }

    pub fn build_stack(
        &mut self,
        artifact: &ArtifactRepr,
        dryrun: bool
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Building {} stack...", artifact.stack_name.as_str());

        if artifact.meta.as_ref().is_some() {
            self.build_meta(&artifact.meta, dryrun)?;
        }

        for node in artifact.deploys.iter() {
            self.walk_build_path(node, dryrun)?
        }
        Ok(())
    }

    fn build_meta(
        &mut self,
        meta_stack: &Box<Option<ArtifactRepr>>,
        dryrun: bool
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(meta) = meta_stack.as_ref() {
            self.build_stack(meta, dryrun)?;
        }

        Ok(())
    }

    fn walk_build_path(
        &mut self,
        node: &ArtifactNodeRepr,
        dryrun: bool
    ) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we build.
        for child in node.dependencies.iter() {
            self.walk_build_path(child, dryrun)?
        }

        if !self.built.contains(&node.fqn) {
            self.build_node(&node, dryrun).and_then(|_out| {
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

    fn build_node(&self, node: &ArtifactNodeRepr, dryrun: bool) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        if node.build_step.is_some() {
            println!("Building {}", node.fqn);
        }
        let tf_path = Path::new(&node.file_path)
            .parent()
            .unwrap()
            .join("terraform/");
        println!("Deploying {}", node.fqn);
        let helm_config = node.deploy_steps
            .get(&"helm".to_string()).unwrap();

        match helm_config {
            Some(conf) => {
                self.deploy_tf(&tf_path, conf, dryrun)
            },
            None => {
                println!("No helm configuration found for {}... trying to deploy...", node.fqn);
                self.deploy_tf(&tf_path, &HashMap::<String, String>::new(), dryrun)
            }
        }
    }

    fn deploy_tf(
        &self,
        path: &std::path::PathBuf,
        config: &HashMap<String, String>,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let mut cmd = Command::new("./terraform");
        cmd.arg("plan").arg("-out=tfplan").arg("-no-color").arg("-detailed-exitcode");

        for (key, value) in config.iter() {
            cmd.arg(format!("-var={}={}", key, value));
        }
        cmd.current_dir(path);
        let out = cmd.output()?;

        if dryrun {
            return Ok(out)
        } else {
            let mut cmd = Command::new("./terraform");
            cmd.arg("apply").arg("tfplan");
            cmd.current_dir(path);
            let out = cmd.output()?;
        }

        Ok(out)
    }
}
