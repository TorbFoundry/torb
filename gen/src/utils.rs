use std::process::{Command, Output};

use thiserror::Error;
use std::error::Error;
use sha2::{Digest, Sha256};
use base64ct::{Base64UrlUnpadded, Encoding};

#[derive(Error, Debug)]
pub enum TorbUtilityErrors {
    #[error("Unable to run this command: {command:?}, in shell: {shell:?}, because of this reason: {reason:?}")]
    UnableToRunCommandInShell { command: String, shell: String , reason: String },

    #[error("Unable to run this command: {command:?}, because of this reason: {reason:?}")]
    UnableToRunCommand { command: String, reason: String }
}

const TORB_PATH: &str = ".torb";

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

pub fn run_command_in_user_shell(command_str: String) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let shell = std::env::var("SHELL").unwrap();
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
    let hash_base64 = Base64UrlUnpadded::encode_string(&hash);

    hash_base64 == original_hash
}

pub struct CommandPipeline {
    commands: Vec<Command>
}

impl CommandPipeline {
    pub fn new(commands: Option<Vec<Vec<&str>>>) -> Self {
        let new_commands = commands.unwrap_or(Vec::new()).iter().map(|command_parts| {
            let head = &command_parts[..1];
            let tail = &command_parts[1..];
            let command_string = head.first().unwrap();
            let mut command = Command::new(command_string);

            tail.iter().for_each(|arg| {
                command.arg(arg);
            });

            command
        }).collect();

        CommandPipeline {
            commands: new_commands
        }
    }

    pub fn execute(&mut self) -> Result<Vec<std::process::Output>, Box<dyn Error>> {
        let outputs: Result<Vec<Output>, Box<dyn std::error::Error>> = self.commands.iter_mut().map(CommandPipeline::run_command).collect();

        outputs
    }

    fn run_command(command: &mut Command) -> Result<std::process::Output, Box<dyn Error>> {
        let output = command.output()?;

        if output.status.success() {
            Ok(output)
        } else {
            Err(Box::new(TorbUtilityErrors::UnableToRunCommand {
                command: format!("{:?}", command),
                reason: String::from_utf8(output.stderr).unwrap(),
            }))
        }
    }
}