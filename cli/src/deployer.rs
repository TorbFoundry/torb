// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.5-03.13
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

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
    watcher_patch: bool
}

impl StackDeployer {
    pub fn new(watcher_patch: bool) -> StackDeployer {
        StackDeployer {
            watcher_patch
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

        let out = self.deploy_tf(dryrun).expect("Failed to plan and deploy terraform.");
        println!("{}", std::str::from_utf8(&out.stdout).unwrap());
        println!("{}", std::str::from_utf8(&out.stderr).unwrap());

        Ok(())
    }

    fn init_tf(&self) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        println!("Initalizing terraform...");
        let torb_path = torb_path();
        let iac_env_path = self.iac_environment_path();
        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()));
        cmd.arg("init");
        cmd.arg("-upgrade");
        cmd.current_dir(torb_path);

        println!("Running command: {:?}", cmd);
        Ok(cmd.output()?)
    }

    fn iac_environment_path(&self) -> std::path::PathBuf {
        let buildstate_path = buildstate_path_or_create();
        if self.watcher_patch {
            buildstate_path.join("watcher_iac_environment")
        } else {
            buildstate_path.join("iac_environment")
        }
    }

    fn deploy_tf(
        &self,
        dryrun: bool,
    ) -> Result<std::process::Output, Box<dyn std::error::Error>> {
        let torb_path = torb_path();
        let iac_env_path = self.iac_environment_path();

        if self.watcher_patch {
            let buildstate_path = buildstate_path_or_create();
            let non_watcher_iac = buildstate_path.join("iac_environment");
            let tf_state_path = non_watcher_iac.join("terraform.tfstate");

            if tf_state_path.exists() {
                let new_path = iac_env_path.join("terraform.tfstate");
                std::fs::copy(tf_state_path, new_path).expect("Failed to copy supporting build file.");
            };
        };

        let mut cmd = Command::new("./terraform");
        cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()))
            .arg("plan")
            .arg("-out=./tfplan");

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
            cmd.arg(format!("-chdir={}", iac_env_path.to_str().unwrap()))
            .arg("apply")
            .arg("./tfplan")
            .current_dir(&torb_path);
            Ok(cmd.output()?)
        }
    }
}
