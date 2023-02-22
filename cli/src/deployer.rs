# Business Source License 1.1
# Licensor:  Torb Foundry
# Licensed Work:  Torb v0.3.0-02.22
# The Licensed Work is © 2023-Present Torb Foundry
#
# Change License: GNU Affero General Public License Version 3
# Additional Use Grant: None
# Change Date: Feb 22, 2023
#
# See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use crate::artifacts::{ArtifactRepr};
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
}

impl StackDeployer {
    pub fn new() -> StackDeployer {
        StackDeployer {
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
        println!("{}", std::str::from_utf8(&out.stderr).unwrap());

        // if artifact.meta.as_ref().is_some() {
        //     println!("Deploying meta...");
        //     self.deploy_meta(&artifact.meta, dryrun)?;
        // }

        let out = self.deploy_tf(dryrun).expect("Failed to plan and deploy terraform.");
        println!("{}", std::str::from_utf8(&out.stdout).unwrap());
        println!("{}", std::str::from_utf8(&out.stderr).unwrap());

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
        cmd.arg("-upgrade");
        cmd.current_dir(torb_path);

        println!("Running command: {:?}", cmd);
        Ok(cmd.output()?)
    }

    fn deploy_tf(
        &self,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let torb_path = torb_path();
        let buildstate_path = buildstate_path_or_create();
        let iac_env_path = buildstate_path.join("iac_environment");

        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()))
            .arg("plan")
            .arg("-out=./tfplan");
            // .arg("-detailed-exitcode");

        cmd.current_dir(&torb_path);

        println!("Running command: {:?}", cmd);
        let out = cmd.output()?;


        if !out.status.success() {
            let err_resp = std::str::from_utf8(&out.stderr).unwrap();
            println!("{}", std::str::from_utf8(&out.stderr).unwrap());
            println!("{}", std::str::from_utf8(&out.stderr).unwrap());
            println!("{}", std::str::from_utf8(&out.stderr).unwrap());
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
            cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()))
            .arg("apply")
            .arg("./tfplan")
            .current_dir(&torb_path);
            Ok(cmd.output()?)
        }
    }
}
