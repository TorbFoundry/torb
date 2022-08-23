use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use serde::{Deserialize, Serialize};
use serde_yaml::{self};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::process::Command;
use thiserror::Error;
use crate::utils::{torb_path};

#[derive(Error, Debug)]
pub enum TorbBuilderErrors {
    #[error("Command `{command:?}` failed with response: {response}")]
    FailedToPlan {
        command: std::process::Command,
        response: String,
    },
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
        dryrun: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Building {} stack...", artifact.stack_name.as_str());

        let out = self.init_tf().expect("Failed to initialize terraform.");
        println!("{}", std::str::from_utf8(&out.stdout).unwrap());

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
        dryrun: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(meta) = meta_stack.as_ref() {
            self.build_stack(meta, dryrun)?;
        }

        Ok(())
    }

    fn walk_build_path(
        &mut self,
        node: &ArtifactNodeRepr,
        dryrun: bool,
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

    fn build_node(
        &self,
        node: &ArtifactNodeRepr,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        if node.build_step.is_some() {
            println!("Building {}", node.fqn);
        }
        let tf_path = Path::new(&node.file_path)
            .parent()
            .unwrap()
            .join("terraform/");
        println!("Deploying {}", node.fqn);
        println!("deploy steps: {:?}", node.deploy_steps);
        println!("tf path: {:?}", &tf_path);
        let helm_config = node
            .deploy_steps
            .get(&"helm".to_string())
            .expect("No helm deploy config key found.");

        match helm_config {
            Some(conf) => self.deploy_tf(&tf_path, conf, dryrun),
            None => {
                println!(
                    "No helm configuration found for {}... trying to deploy...",
                    node.fqn
                );
                self.deploy_tf(&tf_path, &HashMap::<String, String>::new(), dryrun)
            }
        }
    }

    fn init_tf(&self) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        println!("Initalizing terraform...");
        let torb_path = torb_path();
        let artifact_path = torb_path.join("torb-artifacts/");
        println!("artifact path: {:?}", artifact_path);
        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", artifact_path.to_str().unwrap()));
        cmd.arg("init");
        cmd.current_dir(torb_path);

        Ok(cmd.output()?)
    }

    fn deploy_tf(
        &self,
        path: &std::path::PathBuf,
        config: &HashMap<String, String>,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let torb_path = torb_path();
        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", path.to_str().unwrap()));
        cmd.arg("plan")
            .arg("-out=tfplan")
            .arg("-no-color")
            .arg("-detailed-exitcode");

        for (key, value) in config.iter() {
            cmd.arg(format!("-var={}={}", key, value));
        }
        cmd.current_dir(torb_path);
        let out = cmd.output()?;

        if !out.status.success() {
            let err_resp = std::str::from_utf8(&out.stderr).unwrap();
            let err = TorbBuilderErrors::FailedToPlan {
                command: cmd,
                response: err_resp.to_string(),
            };

            return Err(Box::new(err));
        }

        if dryrun {
            Ok(out)
        } else {
            let mut cmd = Command::new("./terraform");
            cmd.arg("apply").arg("tfplan");
            cmd.current_dir(path);
            Ok(cmd.output()?)
        }
    }
}
