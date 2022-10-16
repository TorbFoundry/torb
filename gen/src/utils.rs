use thiserror::Error;
use sha2::{Digest, Sha256};
use base64ct::{Base64UrlUnpadded, Encoding};

#[derive(Error, Debug)]
pub enum TorbUtilityErrors {
    #[error("Unable to run this command: {command:?}, in shell: {shell:?}, because of this reason: {reason:?}")]
    UnableToRunCommandInShell { command: String, shell: String , reason: String },
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

pub fn run_command_in_user_shell(command_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let shell = std::env::var("SHELL").unwrap();
    let shell_args = vec!["-c".to_string(), command_str.to_string()];

    let mut command = std::process::Command::new(shell.clone());
    command.args(shell_args);

    let output = command.output()?;

    if output.status.success() {
        Ok(())
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