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
            println!("Deploying meta...");
            self.deploy_meta(&artifact.meta, dryrun)?;
        }

        self.deploy_tf(dryrun)?;

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

    fn init_tf(&self) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        println!("Initalizing terraform...");
        let torb_path = torb_path();
        let buildstate_path = buildstate_path_or_create();
        let iac_env_path = buildstate_path.join("iac_environment");
        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()));
        cmd.arg("init");
        cmd.current_dir(torb_path);

        println!("Running command: {:?}", cmd);
        Ok(cmd.output()?)
    }

    fn deploy_tf(
        &self,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let torb_path = torb_path();
        let mut cmd = Command::new("./terraform");
        let buildstate_path = buildstate_path_or_create();
        let iac_env_path = buildstate_path.join("iac_environment");
        cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()));
        cmd.arg("plan")
            .arg("-out=tfplan")
            .arg("-no-color")
            .arg("-detailed-exitcode");

        cmd.current_dir(&torb_path);

        println!("Running command: {:?}", cmd);
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
            cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()));
            cmd.current_dir(&torb_path);
            Ok(cmd.output()?)
        }
    }
}
