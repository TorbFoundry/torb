use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use std::collections::{HashSet};
use indexmap::{IndexMap};
use std::path::Path;
use std::process::Command;
use thiserror::Error;
use crate::utils::{torb_path, buildstate_path_or_create};

#[derive(Error, Debug)]
pub enum TorbDeployerErrors {
    #[error("Command `{command:?}` failed with response: {response}")]
    FailedToPlan {
        command: std::process::Command,
        response: String,
    },
}

pub struct StackDeployer {
    built: HashSet<String>,
}

impl StackDeployer {
    pub fn new() -> StackDeployer {
        StackDeployer {
            built: HashSet::new(),
        }
    }

    pub fn deploy(
        &mut self,
        artifact: &ArtifactRepr,
        dryrun: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Deploying {} stack...", artifact.stack_name.as_str());

        let out = self.init_tf().expect("Failed to initialize terraform.");
        println!("{}", std::str::from_utf8(&out.stdout).unwrap());

        if artifact.meta.as_ref().is_some() {
            self.deploy_meta(&artifact.meta, dryrun)?;
        }

        for node in artifact.deploys.iter() {
            self.walk_deploy_path(node, dryrun)?
        }
        Ok(())
    }

    fn deploy_meta(
        &mut self,
        meta_stack: &Box<Option<ArtifactRepr>>,
        dryrun: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(meta) = meta_stack.as_ref() {
            self.deploy(meta, dryrun)?;
        }

        Ok(())
    }

    fn walk_deploy_path(
        &mut self,
        node: &ArtifactNodeRepr,
        dryrun: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we deploy.
        for child in node.dependencies.iter() {
            self.walk_deploy_path(child, dryrun)?
        }

        if !self.built.contains(&node.fqn) {
            self.deploy_node(&node, dryrun).and_then(|_out| {
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

    fn deploy_node(
        &self,
        node: &ArtifactNodeRepr,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
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
                self.deploy_tf(&tf_path, &IndexMap::<String, String>::new(), dryrun)
            }
        }
    }

    fn init_tf(&self) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        println!("Initalizing terraform...");
        let torb_path = torb_path();
        let buildstate_path = buildstate_path_or_create();
        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", buildstate_path.to_str().unwrap()));
        cmd.arg("init");
        cmd.current_dir(torb_path);

        Ok(cmd.output()?)
    }

    fn deploy_tf(
        &self,
        path: &std::path::PathBuf,
        config: &IndexMap<String, String>,
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
            let err = TorbDeployerErrors::FailedToPlan {
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
