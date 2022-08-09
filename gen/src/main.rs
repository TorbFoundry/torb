use clap::{App, Arg, SubCommand};
use dirs;
use std::fs;
use std::fs::File;
use std::io;
use std::process::Command;
use thiserror::Error;
use ureq;
mod resolver;
use resolver::{Resolver, ResolverConfig, StackGraph};
mod artifacts;
use artifacts::{write_build_file, ArtifactRepr};
mod builder;
use builder::{StackBuilder};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Error, Debug)]
pub enum TorbCliErrors {
    #[error("Stack manifest missing or invalid. Please run `torb init`")]
    ManifestInvalid,
    #[error("Stack meta template missing or invalid. Please run `torb init`")]
    StackMetaNotFound,
}

const TORB_PATH: &str = ".torb";

fn torb_path() -> std::path::PathBuf {
    let home_dir = dirs::home_dir().unwrap();
    home_dir.join(TORB_PATH)
}

fn init() {
    let torb_path_buf = torb_path();
    let torb_path = torb_path_buf.as_path();
    if !torb_path.is_dir() {
        fs::create_dir(&torb_path).unwrap();
        let _clone_cmd_out = Command::new("git")
            .arg("clone")
            .arg("git@github.com:TorbFoundry/torb-artifacts.git")
            .current_dir(&torb_path)
            .output()
            .expect("Failed to clone torb-artifacts");
    };

    let tf_path = torb_path.join("terraform.zip");
    if !tf_path.is_file() {
        let resp = ureq::get(
            "https://releases.hashicorp.com/terraform/1.2.5/terraform_1.2.5_linux_amd64.zip",
        )
        .call()
        .unwrap();

        let mut out = File::create(tf_path).unwrap();
        io::copy(&mut resp.into_reader(), &mut out).expect("Failed to write terraform zip file.");

        let _unzip_cmd_out = Command::new("unzip")
            .arg(&torb_path.join("terraform.zip"))
            .current_dir(&torb_path)
            .output()
            .expect("Failed to unzip terraform.");
    }
}

fn resolve_stack(stack_yaml: &String) -> Result<StackGraph, Box<dyn std::error::Error>> {
    let stack_def_yaml: serde_yaml::Value = serde_yaml::from_str(stack_yaml).unwrap();
    let stack_name = stack_def_yaml.get("name").unwrap().as_str().unwrap();
    let stack_description = stack_def_yaml.get("description").unwrap().as_str().unwrap();
    let resolver_conf = ResolverConfig::new(
        false,
        stack_name.to_string(),
        stack_description.to_string(),
        stack_def_yaml.clone(),
        VERSION.to_string(),
    );

    let resolver = Resolver::new(&resolver_conf);

    resolver.resolve()
}

fn build_stack(build_artifact: ArtifactRepr, dryrun: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut stack_builder = StackBuilder::new();
    stack_builder.build_stack(&build_artifact, dryrun)
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

fn pull_stack(
    stack_name: &str,
    fail_not_found: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let home_dir = dirs::home_dir().unwrap();
    let torb_path = home_dir.join(".torb");
    let artifacts_path = torb_path.join("torb-artifacts");
    let stack_manifest_path = artifacts_path.join("stacks").join("manifest.yaml");
    let stack_manifest_contents = fs::read_to_string(&stack_manifest_path).unwrap();
    let stack_manifest_yaml: serde_yaml::Value =
        serde_yaml::from_str(&stack_manifest_contents).unwrap();
    let stacks = stack_manifest_yaml.get("stacks").unwrap();
    let stack_entry = stacks.get(stack_name);

    if stack_entry.is_none() {
        if fail_not_found {
            return Err(Box::new(TorbCliErrors::ManifestInvalid));
        }

        update_artifacts();
        return pull_stack(stack_name, true);
    } else {
        let stack_entry_str = stack_entry.unwrap().as_str().unwrap();
        let stack_contents = fs::read(artifacts_path.join("stacks").join(stack_entry_str))
            .map(|s| String::from_utf8(s).unwrap())?;

        return Ok(stack_contents);
    }
}

fn main() {
    let cli = App::new("torb")
        .version("1.0.0")
        .author("Torb Foundry")
        .subcommand(SubCommand::with_name("version").about("Get the version of this torb."))
        .subcommand(
            SubCommand::with_name("init").about("Initialize Torb, download artifacts and tools."),
        )
        .subcommand(
            SubCommand::with_name("build-stack")
                .about("Build a stack from a stack definition file.")
                .arg(
                    Arg::new("--stack-name")
                        .short('s')
                        .takes_value(true)
                        .help("Name of the stack to build."),
                )
                .arg(
                    Arg::new("--dryrun")
                        .short('d')
                        .takes_value(false)
                        .help("Dry run. Don't actually build the stack."),
                ),
        )
        .subcommand(SubCommand::with_name("list-stacks").about("List all available stacks."));

    let cli_matches = cli.get_matches();

    match cli_matches.subcommand_name() {
        Some("init") => {
            init();
        }
        Some("build-stack") => {
            let stack_name_option = cli_matches
                .subcommand_matches("build-stack")
                .unwrap()
                .value_of("--stack-name");
            
            let dryrun_option = cli_matches
                .subcommand_matches("build-stack")
                .unwrap()
                .value_of("--dryrun");

            if let Some(stack_name) = stack_name_option {
                println!("Attempting to pull and build stack: {}", stack_name);
                let stack_yaml: String = pull_stack(stack_name, false)
                    .expect("Failed to pull stack from torb-artifacts.");
                let graph = resolve_stack(&stack_yaml).unwrap();
                let (build_filename, build_artifact) = write_build_file(graph);

                build_stack(build_artifact, dryrun_option.is_some()).unwrap()
            }
        }
        Some("list-stacks") => {
            println!("Listing stacks");
        }
        Some("version") => {
            println!("Torb Version: {}", VERSION);
        }
        _ => {
            println!("No subcommand specified.");
        }
    }
}
