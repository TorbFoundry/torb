mod artifacts;
mod builder;
mod composer;
mod config;
mod deployer;
mod initializer;
mod resolver;
mod utils;
mod vcs;
mod cli;

use std::fs;
use std::fs::File;
use std::io;
use std::process::Command;
use thiserror::Error;
use ureq;
use utils::{buildstate_path_or_create, torb_path};

use crate::artifacts::{load_build_file, get_build_file_info, deserialize_stack_yaml_into_artifact, write_build_file, ArtifactRepr};
use crate::composer::Composer;
use crate::config::TORB_CONFIG;
use crate::initializer::StackInitializer;
use crate::vcs::{GitVersionControl, GithubVCS};
use crate::builder::{StackBuilder};
use crate::deployer::{StackDeployer};
use crate::cli::cli;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Error, Debug)]
pub enum TorbCliErrors {
    #[error("Stack manifest missing or invalid. Please run `torb init`")]
    ManifestInvalid,
    #[error("Stack meta template missing or invalid. Please run `torb init`")]
    StackMetaNotFound,
}

fn init() {
    println!("Initializing...");
    let torb_path_buf = torb_path();
    let torb_path = torb_path_buf.as_path();
    if !torb_path.is_dir() {
        println!("Creating {}...", torb_path.display());
        fs::create_dir(&torb_path).unwrap();
        println!("Cloning build artifacts...");
        let _clone_cmd_out = Command::new("git")
            .arg("clone")
            .arg("git@github.com:TorbFoundry/torb-artifacts.git")
            .current_dir(&torb_path)
            .output()
            .expect("Failed to clone torb-artifacts");
    };

    let torb_config_path = torb_path.join("config.yaml");
    let torb_config_template = torb_path.join("torb-artifacts/config.template.yaml");

    if !torb_config_path.exists() {
        fs::copy(torb_config_template, torb_config_path).expect("Unable to copy config template file from ~/.torb/torb-artifacts/config.template.yaml. Please check that Torb has been initialized properly.");
    }

    let tf_path = torb_path.join("terraform.zip");
    let tf_bin_path = torb_path.join("terraform");
    if !tf_bin_path.is_file() {
        println!("Downloading terraform...");
        let tf_url = match std::env::consts::OS {
            "linux" => "https://releases.hashicorp.com/terraform/1.2.5/terraform_1.2.5_linux_amd64.zip",
            "macos" => "https://releases.hashicorp.com/terraform/1.2.5/terraform_1.2.5_darwin_amd64.zip",
            _ => panic!("Unsupported OS"),
        };
        let resp = ureq::get(
            tf_url
        )
        .call()
        .unwrap();

        let mut out = File::create(&tf_path).unwrap();
        io::copy(&mut resp.into_reader(), &mut out).expect("Failed to write terraform zip file.");

        let mut unzip_cmd = Command::new("unzip");

        unzip_cmd.arg(&tf_path).current_dir(&torb_path);

        let _unzip_cmd_out = unzip_cmd.output().expect("Failed to unzip terraform.");
    }

    println!("Finished!")
}

fn create_repo(path: String, local_only: bool) {
    if !std::path::Path::new(&path).exists() {
        let mut vcs = GithubVCS::new(
            TORB_CONFIG.githubToken.clone(),
            TORB_CONFIG.githubUser.clone(),
        );

        let mut buf = std::path::PathBuf::new();
        buf.push(path);

        vcs.set_cwd(buf);

        vcs.create_repo(local_only)
            .expect("Failed to create repo.");
    } else {
        println!("Repo already exists locally. Skipping creation.");
    }
}

fn checkout_stack(name: Option<&str>) {
    match name {
        Some(name) => {
            let stack_yaml: String =
                pull_stack(name, false).expect("Failed to pull stack from torb-artifacts.");

            fs::write("./stack.yaml", stack_yaml).expect("Failed to write stack.yaml.");
        }
        None => {
            fs::write("./stack.yaml", "").expect("Failed to write stack.yaml");
        }
    }
}

fn init_stack(file_path: String) {
    println!("Attempting to read or create buildstate folder...");
    buildstate_path_or_create();

    println!("Attempting to read stack file...");
    let stack_yaml = fs::read_to_string(&file_path).expect("Failed to read stack.yaml.");

    println!("Reading stack into internal representation...");
    let artifact = deserialize_stack_yaml_into_artifact(&stack_yaml)
        .expect("Failed to read stack into internal representation.");

    let mut stack_initializer = StackInitializer::new(&artifact);

    stack_initializer
        .run_node_init_steps()
        .expect("Failed to initialize stack.");
}

fn compose_build_environment(build_hash: String, build_artifact: &ArtifactRepr) {
    let mut composer = Composer::new(build_hash, build_artifact);
    composer.compose().unwrap();
}

fn run_dependency_build_steps(_build_hash: String, build_artifact: &ArtifactRepr, build_platform_string: String, dryrun: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = StackBuilder::new(build_artifact, build_platform_string, dryrun);

    builder.build()
}

fn run_deploy_steps(_build_hash: String, build_artifact: &ArtifactRepr, dryrun: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut deployer = StackDeployer::new(); 

    deployer.deploy(build_artifact, dryrun)
}

fn update_artifacts() {
    let torb_path_buf = torb_path();
    let torb_path = torb_path_buf.as_path();
    let artifacts_path = torb_path.join("torb-artifacts");
    let _clone_cmd_out = Command::new("git")
        .arg("pull")
        .arg("--rebase")
        .current_dir(&artifacts_path)
        .output()
        .expect("Failed to pull torb-artifacts");
}

fn load_stack_manifest() -> serde_yaml::Value {
    let torb_path = torb_path();
    let artifacts_path = torb_path.join("torb-artifacts");
    let stack_manifest_path = artifacts_path.join("stacks").join("manifest.yaml");
    let stack_manifest_contents = fs::read_to_string(&stack_manifest_path).unwrap();
    let stack_manifest_yaml: serde_yaml::Value =
        serde_yaml::from_str(&stack_manifest_contents).unwrap();
    
    stack_manifest_yaml.get("stacks").unwrap().clone()
}

fn pull_stack(
    stack_name: &str,
    fail_not_found: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let stacks = load_stack_manifest();
    let stack_entry = stacks.get(stack_name);

    if stack_entry.is_none() {
        if fail_not_found {
            return Err(Box::new(TorbCliErrors::ManifestInvalid));
        }

        update_artifacts();
        return pull_stack(stack_name, true);
    } else {
        let torb_path = torb_path();
        let artifacts_path = torb_path.join("torb-artifacts");
        let stack_entry_str = stack_entry.unwrap().as_str().unwrap();
        let stack_contents = fs::read(artifacts_path.join("stacks").join(stack_entry_str))
            .map(|s| String::from_utf8(s).unwrap())?;

        return Ok(stack_contents);
    }
}

fn main() {
    let cli_app = cli();

    let cli_matches = cli_app.get_matches();

    match cli_matches.subcommand_name() {
        Some("init") => {
            init();
        }
        Some("repo") => {
            let mut subcommand = cli_matches.subcommand_matches("repo").unwrap();
            match subcommand.subcommand_name() {
                Some("create") => {
                    subcommand = subcommand.subcommand_matches("create").unwrap();
                    let path_option = subcommand.value_of("path");
                    let local_option = subcommand.value_of("--local-only");

                    create_repo(path_option.unwrap().to_string(), local_option.is_some());
                }
                _ => {
                    println!("No subcommand specified.");
                }
            }
        }
        Some("stack") => {
            let mut subcommand = cli_matches.subcommand_matches("stack").unwrap();
            match subcommand.subcommand_name() {
                Some("checkout") => {
                    let name_option = subcommand
                        .subcommand_matches("checkout")
                        .unwrap()
                        .value_of("name");

                    checkout_stack(name_option);
                }
                Some("init") => {
                    let file_path_option = subcommand
                        .subcommand_matches("init")
                        .unwrap()
                        .value_of("file");

                    init_stack(file_path_option.unwrap().to_string())
                }
                Some("build") => {
                    subcommand = subcommand.subcommand_matches("build").unwrap();
                    let file_path_option = subcommand.value_of("file");
                    let dryrun_option = subcommand.value_of("--dryrun");
                    let build_platforms_string = subcommand
                        .values_of("--platforms")
                        .unwrap()
                        .collect::<Vec<&str>>()
                        .join(",");

                    if let Some(file_path) = file_path_option {
                        println!("Attempting to read or create buildstate folder...");
                        buildstate_path_or_create();
                        println!("Attempting to read and build stack: {}", file_path);
                        let contents = fs::read_to_string(file_path)
                            .expect("Something went wrong reading the stack file.");

                        let (build_hash, build_filename, _) = write_build_file(contents);

                        let (_, _, build_artifact) =
                            load_build_file(build_filename).expect("Unable to load build file.");

                        run_dependency_build_steps(
                            build_hash.clone(),
                            &build_artifact,
                            build_platforms_string,
                            dryrun_option.is_some(),
                        ).expect("Unable to build required images/artifacts for nodes.");

                        compose_build_environment(build_hash.clone(), &build_artifact);
                    }
                }
                Some("deploy") => {
                    subcommand = subcommand.subcommand_matches("deploy").unwrap();
                    let file_path_option = subcommand.value_of("file");
                    let dryrun_option = subcommand.value_of("--dryrun");

                    if let Some(file_path) = file_path_option {
                        println!("Attempting to read and deploy stack: {}", file_path);
                        let contents = fs::read_to_string(file_path)
                            .expect("Something went wrong reading the stack file.");

                        let artifact = deserialize_stack_yaml_into_artifact(&contents).expect("Unable to read stack file into internal representation.");
                        
                        let (build_hash, build_filename, _) = get_build_file_info(&artifact).expect("Unable to get build file info for stack.");
                        println!("build_filename: {}", build_filename);
                        let (_, _, build_artifact) =
                            load_build_file(build_filename).expect("Unable to load build file.");

                        run_deploy_steps(
                            build_hash.clone(),
                            &build_artifact,
                            dryrun_option.is_some(),
                        ).expect("Unable to deploy required images/artifacts for nodes.");
                    }
                }
                Some("list") => {
                    println!("\nTorb Stacks:\n");
                    let stack_manifest = load_stack_manifest();
                    for (key, _) in stack_manifest.as_mapping().unwrap().iter() {
                        println!("- {}", key.as_str().unwrap());
                    }
                }
                _ => {
                    println!("No subcommand specified.");
                }
            }
        }
        Some("version") => {
            println!("Torb Version: {}", VERSION);
        }
        _ => {
            println!("No subcommand specified.");
        }
    }
}
