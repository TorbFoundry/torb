// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.7-03.23
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

mod artifacts;
mod builder;
mod cli;
mod composer;
mod config;
mod deployer;
mod initializer;
mod resolver;
mod utils;
mod vcs;
mod watcher;
mod animation;

use indexmap::IndexMap;
use rayon::prelude::*;
use std::fs;
use std::fs::File;
use std::io::{self};
use std::process::Command;
use thiserror::Error;
use ureq;
use utils::{buildstate_path_or_create, torb_path, PrettyExit};
use animation::{BuilderAnimation, Animation};

use crate::artifacts::{
    deserialize_stack_yaml_into_artifact, get_build_file_info, load_build_file, write_build_file,
    ArtifactRepr,
};
use crate::builder::StackBuilder;
use crate::cli::cli;
use crate::composer::Composer;
use crate::config::TORB_CONFIG;
use crate::deployer::StackDeployer;
use crate::initializer::StackInitializer;
use crate::utils::{CommandConfig, CommandPipeline, PrettyContext};
use crate::vcs::{GitVersionControl, GithubVCS};
use crate::watcher::Watcher;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Error, Debug)]
pub enum TorbCliErrors {
    #[error("Stack manifest missing or invalid. Please run `torb init`")]
    ManifestInvalid,
    #[error("Stack meta template missing or invalid. Please run `torb init`")]
    StackMetaNotFound,
    #[error("The stack name was found in multiple repository manifests please prefix the stack name with the repository you wish to use. i.e. torb-artifacts:flask-app-with-react-frontend")]
    StackAmbiguous,
}

fn init() {
    println!("Initializing...");
    let torb_path_buf = torb_path();
    let torb_path = torb_path_buf.as_path();
    let artifacts_path = &torb_path.join("repositories");
    if !torb_path.is_dir() {
        println!("Creating {}...", torb_path.display());

        fs::create_dir(&torb_path).unwrap();
    }

    if !artifacts_path.is_dir() {
        println!("Cloning build artifacts...");
        fs::create_dir(artifacts_path).unwrap();
        let _clone_cmd_out = Command::new("git")
            .arg("clone")
            .arg("git@github.com:TorbFoundry/torb-artifacts.git")
            .current_dir(&artifacts_path)
            .output()
            .expect("Failed to clone torb-artifacts");
    };

    let torb_config_path = torb_path.join("config.yaml");
    let torb_config_template = torb_path.join("repositories/torb-artifacts/config.template.yaml");

    if !torb_config_path.exists() {
        let err_msg = format!("Unable to copy config template file from {}. Please check that Torb has been initialized properly.", torb_config_template.to_str().unwrap());
        fs::copy(torb_config_template, torb_config_path).expect(&err_msg);
    }

    let tf_path = torb_path.join("terraform.zip");
    let tf_bin_path = torb_path.join("terraform");
    if !tf_bin_path.is_file() {
        println!("Downloading terraform...");
        let tf_url = match std::env::consts::OS {
            "linux" => {
                "https://releases.hashicorp.com/terraform/1.2.5/terraform_1.2.5_linux_amd64.zip"
            }
            "macos" => {
                "https://releases.hashicorp.com/terraform/1.2.5/terraform_1.2.5_darwin_amd64.zip"
            }
            _ => panic!("Unsupported OS"),
        };
        let resp = ureq::get(tf_url).call().unwrap();

        let mut out = File::create(&tf_path).unwrap();
        io::copy(&mut resp.into_reader(), &mut out).expect("Failed to write terraform zip file.");

        let mut unzip_cmd = Command::new("unzip");

        unzip_cmd.arg(&tf_path).current_dir(&torb_path);

        let _unzip_cmd_out = unzip_cmd.output().expect("Failed to unzip terraform.");
    }

    let buildx_cmd_conf = CommandConfig::new(
        "docker",
        vec![
            "buildx",
            "create",
            "--name",
            "torb_builder",
            "--driver-opt",
            "network=host",
        ],
        None,
    );

    let res = CommandPipeline::execute_single(buildx_cmd_conf);

    match res {
        Ok(_) => println!("Created docker build kit builder, torb_builder."),
        Err(err) => panic!("{}", err),
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

        vcs.create_repo(local_only).expect("Failed to create repo.");
    } else {
        println!("Repo already exists locally. Skipping creation.");
    }
}

fn checkout_stack(name: Option<&str>) {
    match name {
        Some(name) => {
            let stack_yaml: String =
                pull_stack(name, false).expect("Failed to pull stack from any repository. Check that the source is configured correctly and that the stack exists.");

            fs::write("./stack.yaml", stack_yaml).expect("Failed to write stack.yaml.");
        }
        None => {
            fs::write("./stack.yaml", "").expect("Failed to write stack.yaml");
        }
    }
}

fn new_stack() {
    let torb_path = torb_path();
    let repositories_path = torb_path.join("repositories");
    let torb_artifacts = repositories_path.join("torb-artifacts");
    let template_path = torb_artifacts.join("stack.template.yaml");

    let dest = std::env::current_dir().unwrap().join("stack.template.yaml");

    let source_string = template_path.to_str().unwrap();
    let err_msg = format!("Unable to copy config template file from {source_string}. Please check that Torb has been initialized properly.");

    fs::copy(template_path, dest).expect(&err_msg);
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
        .run_node_init_steps().use_or_pretty_exit(
            PrettyContext::default()
            .error("Oh no, we failed to initialize the stack!")
            .context("Failures here are typically because of missing dependencies for parts of the stack you're looking to initialize.")
            .suggestions(vec![
                "Check that all dependencies are installed.",
                "Check to make sure you're on a compatible operating system."
            ])
            .success("Success! Stack initialized!")
            .pretty()
        )
}

fn compose_build_environment(build_hash: String, build_artifact: &ArtifactRepr) {
    let mut composer = Composer::new(build_hash, build_artifact, false);
    composer.compose().use_or_pretty_exit(
        PrettyContext::default()
        .error("Oh no, we failed to generate the IaC build environment!")
        .success("Success! IaC build environment generated!")
        .context("This typically happens due to failures parsing the stack into HCL for Terraform.")
        .suggestions(vec![
            "Check that your inputs are escaped correctly.",
            "Check that Torb has been initialized correctly, at ~/.torb you should see a Terraform binary appropriate to your system."
        ])
        .pretty()
    );
}

fn run_dependency_build_steps(
    _build_hash: String,
    build_artifact: &ArtifactRepr,
    build_platform_string: String,
    dryrun: bool,
    separate_local_registry: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = StackBuilder::new(
        build_artifact,
        build_platform_string,
        dryrun,
        separate_local_registry,
    );

    builder.build()
}

fn run_deploy_steps(
    _build_hash: String,
    build_artifact: &ArtifactRepr,
    dryrun: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut deployer = StackDeployer::new(false);

    deployer.deploy(build_artifact, dryrun)
}

fn watch(fp_opt: Option<&str>, local_registry: bool) {
    let watcher = Watcher::configure(fp_opt.unwrap_or("stack.yaml").to_string(), local_registry);

    watcher.start();
}

fn clone_artifacts() {
    if TORB_CONFIG.repositories.is_some() {
        let repos_to_aliases = TORB_CONFIG.repositories.clone().unwrap();
        let torb_path = torb_path();
        let artifacts_path = torb_path.join("repositories");
        repos_to_aliases
            .iter()
            .par_bridge()
            .for_each(|(repo, alias)| {
                if alias == "" {
                    let err_msg = format!("Failed to clone {}.", &repo);

                    let _clone_cmd_out = Command::new("git")
                        .arg("clone")
                        .arg(repo)
                        .current_dir(&artifacts_path)
                        .output()
                        .expect(&err_msg);
                } else {
                    let alias_path = artifacts_path.join(&alias);
                    std::fs::create_dir_all(&alias_path)
                        .expect("Unable to create aliased dir for artifact repo.");

                    let err_msg = format!("Failed to clone {} into {}.", &repo, &alias);

                    let _clone_cmd_out = Command::new("git")
                        .arg("clone")
                        .arg(repo)
                        .arg(".")
                        .current_dir(&alias_path)
                        .output()
                        .expect(&err_msg);
                }
            })
    }
}

fn update_artifacts(name: Option<&str>) {
    let filter_name = name.unwrap();
    let torb_path = torb_path();
    let repo_path = torb_path.join("repositories");

    let repos = fs::read_dir(&repo_path).unwrap().par_bridge();

    repos.for_each(|repo_result| {
        let repo = repo_result.unwrap();

        if filter_name == "" || repo.file_name() == filter_name {
            let repo_name = repo.file_name()
                    .into_string()
                    .expect("Failed to convert OsString to String.");

            println!(
                "Refreshing '{}' artifact repository...",
                repo_name
            );

            let err_msg = format!("Failed to pull {:?}", repo.file_name());
            let artifacts_path = repo_path.join(repo.file_name());
            let pull_cmd_out = Command::new("git")
                .arg("pull")
                .arg("--rebase")
                .current_dir(&artifacts_path)
                .output();

            let success_msg = format!("{repo_name} done refreshing!");
            pull_cmd_out.use_or_pretty_exit(
                PrettyContext::default()
                .error(&err_msg)
                .context("This type of error is usually an access or connection issue.")
                .suggestions(vec![
                    "Check that you have the ability to access the artifact repo you're refreshing.",
                    "Check that you have an active internet connection."
                ])
                .success(&success_msg)
                .pretty()
            );

        }
    })
}

fn load_stack_manifests() -> IndexMap<String, serde_yaml::Value> {
    let torb_path = torb_path();
    let artifacts_path = torb_path.join("repositories");

    let repository_paths = fs::read_dir(&artifacts_path)
        .expect("Unable to read list of repositories. Please re-initialize Torb.");

    let mut manifests = IndexMap::<String, serde_yaml::Value>::new();

    for artifact_path_result in repository_paths {
        let artifact_path =
            artifact_path_result.expect("Unable to read entry in repositories, try again.");
        let stack_manifest_path = artifact_path.path().join("stacks").join("manifest.yaml");
        let stack_manifest_contents = fs::read_to_string(&stack_manifest_path).unwrap();
        let stack_manifest_yaml: serde_yaml::Value =
            serde_yaml::from_str(&stack_manifest_contents).unwrap();

        let manifest_name = artifact_path.file_name().to_str().unwrap().to_string();

        manifests.insert(
            manifest_name,
            stack_manifest_yaml.get("stacks").unwrap().clone(),
        );
    }

    manifests
}

fn pull_stack(
    stack_name: &str,
    fail_not_found: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut repo = "";
    let mut stack = stack_name;

    if stack_name.find(":").is_some() {
        let stack_parts: Vec<&str> = stack_name.split(":").collect();
        repo = stack_parts[0];
        stack = stack_parts[1];
    }

    let manifests = load_stack_manifests();

    let mut count = 0;

    for (_name, manifest) in manifests.iter() {
        let stack_entry = manifest.get(stack);
        if stack_entry.is_some() {
            count += 1;
        }
    }

    if count > 1 && repo == "" {
        return Err(Box::new(TorbCliErrors::StackAmbiguous));
    } else if repo == "" {
        repo = "torb-artifacts"
    }

    let err_msg = format!("Unable to find manifest for {repo}. Make sure it was added in config.yaml and pulled with `torb artifacts refresh`");
    let repo_manifest = manifests.get(repo).expect(&err_msg);

    let stack_entry = repo_manifest.get(stack);

    if stack_entry.is_none() {
        if fail_not_found {
            return Err(Box::new(TorbCliErrors::ManifestInvalid));
        }

        update_artifacts(None);
        return pull_stack(stack_name, true);
    } else {
        let torb_path = torb_path();
        let repo_path = torb_path.join("repositories");
        let artifacts_path = repo_path.join(repo);
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
        Some("artifacts") => {
            let mut subcommand = cli_matches.subcommand_matches("artifacts").unwrap();
            match subcommand.subcommand_name() {
                Some("refresh") => {
                    subcommand = subcommand.subcommand_matches("refresh").unwrap();
                    let name_option = subcommand.value_of("name");
                    update_artifacts(name_option);
                }
                Some("clone") => {
                    clone_artifacts();
                }
                _ => {}
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
                Some("new") => new_stack(),
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
                    let dryrun = subcommand.is_present("--dryrun");
                    let local_registry = subcommand.is_present("--local-hosted-registry");

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

                        let (build_hash, build_filename, _) = write_build_file(contents, None);

                        let (_, _, build_artifact) =
                            load_build_file(build_filename).expect("Unable to load build file.");


                        let animator = BuilderAnimation::new();

                        let build_hash_clone = build_hash.clone();
                        let build_artifact_clone = build_artifact.clone();

                        animator.do_with_animation(Box::new(
                            move || {
                            run_dependency_build_steps(
                                build_hash_clone.clone(),
                                &build_artifact_clone,
                            build_platforms_string.clone(),
                                dryrun,
                                local_registry
                            )
                            }
                        )).use_or_pretty_exit(
                                PrettyContext::default()
                                .error("Oh no, we were unable to build the stack!")
                                .success("Success! Stack has been built!")
                                .context("Errors here are typically because of a failed docker build, syntax issue in the dockerfile or a connectivity issue with the docker registry.")
                                .suggestions(vec![
                                    "Check that your dockerfile has no syntax errors and is otherwise correct.",
                                    "If you're building with an image registry that is hosted on the same machine, but as a separate service and not the default docker registry, try passing --local-hosted-registry as a flag."
                                ])
                                .pretty()
                            );

                        compose_build_environment(build_hash.clone(), &build_artifact);
                    }
                }
                Some("deploy") => {
                    subcommand = subcommand.subcommand_matches("deploy").unwrap();
                    let file_path_option = subcommand.value_of("file");
                    let dryrun = subcommand.is_present("--dryrun");

                    if let Some(file_path) = file_path_option {
                        println!("Attempting to read and deploy stack: {}", file_path);
                        let contents = fs::read_to_string(file_path)
                            .expect("Something went wrong reading the stack file.");

                        let artifact = deserialize_stack_yaml_into_artifact(&contents)
                            .expect("Unable to read stack file into internal representation.");

                        let (build_hash, build_filename, _) = get_build_file_info(&artifact)
                            .expect("Unable to get build file info for stack.");
                        println!("build_filename: {}", build_filename);
                        let (_, _, build_artifact) =
                            load_build_file(build_filename).expect("Unable to load build file.");

                        run_deploy_steps(build_hash.clone(), &build_artifact, dryrun)
                        .use_or_pretty_exit(
                            PrettyContext::default()
                            .error("Oh no, we were unable to deploy the stack!")
                            .success("Success! Stack has been deployed!")
                            .context("Errors here are typically because of failed Terraform deployments or Helm failures.")
                            .suggestions(vec![
                                "Check that your Terraform IaC environment was generated correctly. \nThis can be found in your project folder at, .torb_buildstate/iac_environment, or .torb_buildstate/watcher_iac_environment if you're using the watcher.",
                                "To see if your Helm deployment failed you can do `helm ls --namespace <namespace>` where the namespace is the one you're deploying to.",
                                "After seeing if the deployment has failed in Helm, you can use kubectl to debug further. Take a look at https://kubernetes.io/docs/reference/kubectl/cheatsheet/ if you're less familiar with kubectl."
                            ])
                            .pretty()
                        )
                    }
                }
                Some("watch") => {
                    subcommand = subcommand.subcommand_matches("watch").unwrap();
                    let file_path_option = subcommand.value_of("file");
                    let has_local_registry = subcommand.is_present("--local-hosted-registry");
                    watch(file_path_option, has_local_registry);
                }
                Some("list") => {
                    println!("\nTorb Stacks:\n");
                    let stack_manifests = load_stack_manifests();

                    for (repo, manifest) in stack_manifests.iter() {
                        println!("{repo}:");

                        for (key, _) in manifest.as_mapping().unwrap().iter() {
                            println!("- {}", key.as_str().unwrap());
                        }
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
