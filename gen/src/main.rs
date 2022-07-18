use std::process::Command;
use std::fs::File;
use std::io::Write;
use std::io;
use std::fs;
use std::path;
use std::env;
use dirs;
use ureq;
use clap::{App, Arg, SubCommand};

fn init() {
    let home_dir = dirs::home_dir().unwrap();
    let torb_path = home_dir.join(".torb");

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
        let resp = ureq::get("https://releases.hashicorp.com/terraform/1.2.5/terraform_1.2.5_linux_amd64.zip")
            .call()
            .unwrap();
        
        let mut out = File::create(tf_path).unwrap();
        io::copy(&mut resp.into_reader(), &mut out)
            .expect("Failed to write terraform zip file.");
        

        let _unzip_cmd_out = Command::new("unzip")
            .arg(&torb_path.join("terraform.zip"))
            .current_dir(&torb_path)
            .output()
            .expect("Failed to unzip terraform.");
    }
}

fn pull_stack(stack_name: &str) {
    let home_dir = dirs::home_dir().unwrap();
    let torb_path = home_dir.join(".torb");
    let artifacts_path = torb_path.join("torb-artifacts");
    let stack_manifest_path = artifacts_path.join("stacks").join("manifest.yaml");
    let stack_manifest_contents = fs::read_to_string(&stack_path).unwrap();
    let stack_manifest_yaml: serde_yaml::Value = serde_yaml::from_str(&stack_manifest_contents).unwrap();

    if !stack_path.is_dir() {
        let _clone_cmd_out = Command::new("git")
            .arg("clone")
            .arg("")
    }
}

fn main() {
    let cli = App::new("torb")
        .version("1.0.0")
        .author("Torb Foundry")
        .subcommand(
            SubCommand::with_name("init")
            .about("Initialize Torb, download artifacts and tools.")
        )
        .subcommand(
            SubCommand::with_name("build-stack")
            .about("Build a stack from a stack definition file.")
            .arg(
                Arg::new("--stack-name")
                .short('s')
                .takes_value(true)
                .help("Name of the stack to build.")
            )
            .arg(
                Arg::new("--file-path")
                .short('f')
                .takes_value(true)
                .help("Path to local file of the stack to build.")
            )
        )
        .subcommand(
            SubCommand::with_name("list-stacks")
            .about("List all available stacks.")
        );

    let cli_matches = cli.get_matches();

    match cli_matches.subcommand_name() {
        Some("init") => {
            init();
        }
        Some("build-stack") => {
            let stack_name = cli_matches.subcommand_matches("build-stack").unwrap().value_of("stack-name").unwrap();
            let file_path = cli_matches.subcommand_matches("build-stack").unwrap().value_of("file-path").unwrap();

            if stack_name {
                let mut stack_yaml: str = pull_stack(stack_name).expect("Failed to pull stack from torb-artifacts.");
                build_from_yaml(stack_yaml);
            }

            if file_path {
                let mut stack_yaml: str = read_stack_from_file(file_path);
                let stack_path = path::Path::new(&build_stack_matches);
                if !stack_path.is_file() {
                    println!("Stack definition file not found.");
                    return;
                }
                let stack_def = fs::read_to_string(&stack_path).unwrap();
                build_from_yaml(stack_yaml);
            }
            println!("Building stack {} from {}", stack_name, file_path);
        }
        Some("list-stacks") => {
            println!("Listing stacks");
        }
        _ => {
            println!("No subcommand specified.");
        }
    }

    if let Some(_init_matches) = cli_matches.get_one::<String>("init") {
        init();
        return;
    };

    if let Some(build_stack_matches) = cli_matches.get_one::<String>("build-stack") {
        let stack_def_yaml: serde_yaml::Value = serde_yaml::from_str(&stack_def).unwrap();
        let stack_name = stack_def_yaml.get("name").unwrap().as_str().unwrap();
        let stack_description = stack_def_yaml.get("description").unwrap().as_str().unwrap();
        let stack_template = stack_def_yaml.get("template").unwrap().as_str().unwrap();
        let stack_template_path = path::Path::new(&stack_template);
        if !stack_template_path.is_file() {
            println!("Stack template file not found.");
            return;
        }
    }
}
