use std::{process::{Command, Output}, fs::DirEntry};
use data_encoding::BASE32;
use sha2::{Digest, Sha256};
use std::error::Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbUtilityErrors {
    #[error("Unable to run this command: {command:?}, in shell: {shell:?}, because of this reason: {reason:?}")]
    UnableToRunCommandInShell {
        command: String,
        shell: String,
        reason: String,
    },

    #[error("Unable to run this command: {command:?}, because of this reason: {reason:?}")]
    UnableToRunCommand { command: String, reason: String },
}

const TORB_PATH: &str = ".torb";

pub fn kebab_to_snake_case(input: &str) -> String {
    input.replace("-", "_")
}

#[allow(dead_code)]
pub fn snake_case_to_kebab(input: &str) -> String {
    input.replace("_", "-")
}

pub fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .replace("-", "_")
        .replace("/", "")
        .replace(".", "_")
        .replace(" ", "_")
}

pub fn torb_path() -> std::path::PathBuf {
    let home_dir = dirs::home_dir().unwrap();
    home_dir.join(TORB_PATH)
}

pub fn buildstate_path_or_create() -> std::path::PathBuf {
    let current_dir = std::env::current_dir().unwrap();
    let current_dir_state_dir = current_dir.join(".torb_buildstate");

    if current_dir_state_dir.exists() {
        current_dir_state_dir
    } else {
        std::fs::create_dir_all(&current_dir_state_dir).unwrap();
        current_dir_state_dir
    }
}

pub fn for_each_artifact_repository(mut closure: Box<dyn FnMut(std::path::PathBuf, DirEntry) -> () + '_ >) -> Result<(), Box<dyn Error>> {
    let path = torb_path();
    let repo_path = path.join("repositories");

    let repos = std::fs::read_dir(&repo_path)?;

    for repo_res in repos {
        let repo = repo_res?;

        closure(repo_path.clone(), repo);
    }

    Ok(())
}

pub fn run_command_in_user_shell(
    command_str: String,
    shell_override: Option<String>
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let shell = match shell_override {
        Some(sh) => sh,
        None => std::env::var("SHELL").unwrap()
    };

    let shell_args = vec!["-c".to_string(), command_str.to_string()];

    let mut command = std::process::Command::new(shell.clone());
    command.args(shell_args);

    let output = command.output()?;

    if output.status.success() {
        Ok(output)
    } else {
        Err(Box::new(TorbUtilityErrors::UnableToRunCommandInShell {
            command: command_str.to_string(),
            shell: shell,
            reason: String::from_utf8(output.stderr).unwrap(),
        }))
    }
}

pub fn checksum(data: String, original_hash: String) -> bool {
    let hash = Sha256::digest(data.as_bytes());
    let hash_base32 = BASE32.encode(&hash);

    println!("hash: {}", hash_base32);
    println!("original_hash: {}", original_hash);

    hash_base32 == original_hash
}

pub struct CommandPipeline {
    commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub struct CommandConfig<'a> {
    command: &'a str,
    args: Vec<&'a str>,
    working_dir: Option<&'a str>,
}

impl<'a> CommandConfig<'a> {
    pub fn new(
        command: &'a str,
        args: Vec<&'a str>,
        working_dir: Option<&'a str>,
    ) -> CommandConfig<'a> {
        CommandConfig {
            command: command,
            args: args,
            working_dir: working_dir,
        }
    }
}

impl CommandPipeline {
    pub fn new(commands: Option<Vec<CommandConfig>>) -> Self {
        let new_commands = commands
            .unwrap_or(Vec::new())
            .iter()
            .map(|conf| {
                let mut command = Command::new(conf.command);

                conf.args.iter().for_each(|arg| {
                    command.arg(arg);
                });

                if conf.working_dir.is_some() {
                    command.current_dir(conf.working_dir.unwrap());
                };

                command
            })
            .collect();

        CommandPipeline {
            commands: new_commands,
        }
    }

    pub fn execute(&mut self) -> Result<Vec<std::process::Output>, Box<dyn Error>> {
        let outputs: Result<Vec<Output>, Box<dyn std::error::Error>> = self
            .commands
            .iter_mut()
            .map(CommandPipeline::run_command)
            .collect();

        outputs
    }

    fn run_command(command: &mut Command) -> Result<std::process::Output, Box<dyn Error>> {
        let output = command.output()?;

        if output.status.success() {
            println!("Command success: {:?}", output.stdout.clone());
            Ok(output)
        } else {
            println!("Command failed: {:?}", std::str::from_utf8(&output.stderr));
            Err(Box::new(TorbUtilityErrors::UnableToRunCommand {
                command: format!("{:?}", command),
                reason: String::from_utf8(output.stderr).unwrap(),
            }))
        }
    }
}
