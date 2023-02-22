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

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;
use ureq::{AgentBuilder};

#[derive(Error, Debug)]
pub enum TorbVCSErrors {
    #[error("Cannot create repo directory at: {path:?}, reason: {response:?}")]
    UnableToCreateLocalRepoDir { path: PathBuf, response: String },
    #[error("Unable to init local git repo, reason: {response:?}")]
    UnableToInitLocalGitRepo { response: String },
    #[error("Unable to sync remote repo, reason: {response:?}")]
    UnableToSyncRemoteRepo { response: String },
    #[error("Unable to push to remote repo, reason: {response:?}")]
    UnableToPushToRemoteRepo { response: String },
    #[error("Unable to push to init readme, reason: {response:?}")]
    UnableToInitReadme { response: String },
}
trait Or: Sized {
    fn or(self, other: Self) -> Self;
}

impl<'a> Or for &'a str {
    fn or(self, other: &'a str) -> &'a str {
        if self.is_empty() { other } else { self }
    }
}
mod private {
    use super::GithubVCS;

    pub trait Sealed {}
    impl Sealed for GithubVCS {}
}

pub trait GitVersionControlHelpers: private::Sealed {
    fn init_readme(&self) -> Result<(), TorbVCSErrors> {
        let repo_name = self.get_repo_name().unwrap().to_string();
        let error_msg_ga_readme = "Failed to git add README.md";
        let error_msg_commit_readme = "Failed to git commit README.md";
        let cwd = self.get_cwd();
        let readme_path = cwd.join("README.md");
        let contents = format!("# {}", repo_name);

        fs::File::create(&readme_path).unwrap();
        fs::write(&readme_path, contents).unwrap();

        let git_add_readme = Command::new("git")
            .arg("add")
            .arg("./README.md")
            .current_dir(self.get_cwd())
            .output()
            .expect(error_msg_ga_readme);

        Ok(git_add_readme).map(|output| {
            if !output.status.success() {
                Err(output)
            } else {
                Ok(())
            }
        }).and_then(|_output| {
            let git_commit_readme = Command::new("git")
                .arg("commit")
                .arg("-m")
                .arg("Add README.md")
                .current_dir(self.get_cwd())
                .output()
                .expect(error_msg_commit_readme);

            if !git_commit_readme.status.success() {
                Err(git_commit_readme.stderr)
            } else {
                Ok(())
            }
        }).map_err(|err| {
            TorbVCSErrors::UnableToInitReadme {
                response: String::from_utf8(err).unwrap()
            }
        })
    }

    fn add_remote_origin(&self) -> Result<(), TorbVCSErrors> {
        let repo_name = self.get_repo_name().unwrap().to_string();
        let error_msg_remote = format!("Failed to add remote: {:?}", repo_name);
        let remote_repo = format!("{}:{}/{}", self.get_address(), self.get_user(), repo_name);
        println!("remote: {:?}", remote_repo.clone());

        let git_remote_command = Command::new("git")
            .arg("remote")
            .arg("add")
            .arg("origin")
            .arg(remote_repo)
            .current_dir(self.get_cwd())
            .output()
            .expect(&error_msg_remote);

        if !git_remote_command.status.success() {
            Err(TorbVCSErrors::UnableToInitLocalGitRepo {
                response: String::from_utf8(git_remote_command.stderr).unwrap(),
            })
        } else {
            Ok(())
        }
    }

    fn create_main_branch(&self) -> Result<(), TorbVCSErrors> {
        let error_msg_main = "Failed to sync main branch.".to_string();
        let git_main_branch = Command::new("git")
            .arg("branch")
            .arg("-M")
            .arg("main")
            .current_dir(self.get_cwd())
            .output()
            .expect(&error_msg_main);

        if !git_main_branch.status.success() {
            Err(TorbVCSErrors::UnableToSyncRemoteRepo {
                response: String::from_utf8(git_main_branch.stderr).unwrap(),
            })
        } else {
            Ok(())
        }
    }

    fn push_new_main(&self) -> Result<(), TorbVCSErrors> {
        let error_msg_push = "Failed to push to remote.".to_string();
        let mut git_push_main = Command::new("git");

        git_push_main
            .arg("push")
            .arg("-u")
            .arg("origin")
            .arg("main")
            .current_dir(self.get_cwd());

        let res = git_push_main
            .output()
            .expect(&error_msg_push);

        if !res.status.success() {
            Err(TorbVCSErrors::UnableToPushToRemoteRepo {
                response: String::from_utf8(res.stderr).unwrap(),
            })
        } else {
            Ok(())
        }
    }

    fn get_cwd(&self) -> PathBuf;
    fn get_address(&self) -> String;
    fn get_user(&self) -> String;

    fn get_repo_name(&self) -> Option<String> {
        let cwd = self.get_cwd();

        let repo_name = cwd.file_name().unwrap().to_str();

        match repo_name {
            Some(repo_name) => {
                Some(repo_name.to_string())
            }
            None => {
                None
            }
        }
    }
}

pub trait GitVersionControl: GitVersionControlHelpers {
    fn create_remote_repo(&self) -> Result<String, Box<dyn std::error::Error>>;

    fn create_local_repo(
        &self
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mkdir = Command::new("mkdir")
            .arg(self.get_cwd())
            .output()
            .expect("Failed to create directory.");

        if mkdir.status.success() {
            let error_msg = format!("Failed to init git repo at path: {:?}", self.get_cwd());
            let git_command = Command::new("git")
                .arg("init")
                .current_dir(self.get_cwd())
                .output()
                .expect(&error_msg);

            if git_command.status.success() {
                if let Some(_remote) = self.get_repo_name() {
                    self.init_readme()
                        .and_then(|_arg| {
                            self.add_remote_origin()
                        })
                        .and_then(|_arg| { self.create_main_branch() })
                        .and_then(|_arg| { self.push_new_main() } )?;

                    Ok(self.get_cwd().clone())
                } else {
                    Ok(self.get_cwd().clone())
                }
            } else {
                Err(Box::new(TorbVCSErrors::UnableToCreateLocalRepoDir {
                    path: self.get_cwd(),
                    response: String::from_utf8(git_command.stderr).unwrap(),
                }))
            }
        } else {
            let err = TorbVCSErrors::UnableToInitLocalGitRepo {
                response: std::str::from_utf8(&mkdir.stderr)?.to_string(),
            };

            Err(Box::new(err))
        }
    }

    fn create_repo(
        &self,
        local_only: bool,
    ) -> Result<(PathBuf, String), Box<dyn Error>> {
        if local_only {
            Ok((self.create_local_repo()?, "".to_string()))
        } else {
            let remote = self.create_remote_repo()?;

            Ok((
                self.create_local_repo()?,
                remote,
            ))
        }
    }

    /*
     Ian: Generally setters and getters in Rust are non idiomatic and a bit of a smell,
     however traits don't allow us to enforce struct members, or reference them directly.

     The hack for this is to create methods that enforce the members you want.
    */
    fn _get_api_token(&self) -> String;
    fn get_api_token(&self) -> String {
        self._get_api_token()
    }

    fn _get_user(&self) -> String;

    fn _get_address(&self) -> String;

    fn _get_cwd(&self) -> PathBuf;

    fn _set_cwd(&mut self, directory: PathBuf) -> PathBuf;
    fn set_cwd(&mut self, directory: PathBuf) -> PathBuf {
        self._set_cwd(directory)
    }
}

pub struct GithubVCS {
    api_token: String,
    user: String,
    agent: ureq::Agent,
    remote_address: String,
    cwd: PathBuf,
}

impl GitVersionControlHelpers for GithubVCS {
    fn get_user(&self) -> String {
        self._get_user()
    }

    fn get_address(&self) -> String {
        self._get_address()
    }

    fn get_cwd(&self) -> PathBuf {
        self._get_cwd()
    }
}

impl GitVersionControl for GithubVCS {
    fn create_remote_repo(&self) -> Result<String, Box<dyn std::error::Error>> {
        let name = self.get_repo_name().unwrap();

        let token = self.get_api_token();
        /*
        The amount of HTTP requests at the cli level should be fairly low and not take much time.
        With that consideration taking on the overhead of an async runtime which is a heavy dependency,
        and an async client with the changes to a rust project needed to typically support async does not
        seem like the right move to me. - Ian
        */
        let req_string = format!("https://api.github.com/user/repos");
        let req = self
            .agent
            .post(&req_string)
            .set("Authorization", &format!("Bearer {}", token));
        println!("{:?}", req);
        let resp = req
            .send_json(ureq::json!({
                "name": name,
                "private": true,
                "auto_init": false
            }))?
            .into_string()?;

        Ok(resp)
    }

    fn _get_api_token(&self) -> String {
        self.api_token.clone()
    }

    fn _get_user(&self) -> String {
        self.user.clone()
    }

    fn _get_address(&self) -> String {
        self.remote_address.clone()
    }

    fn _get_cwd(&self) -> PathBuf {
        self.cwd.clone()
    }

    fn _set_cwd(&mut self, directory: PathBuf) -> PathBuf {
        self.cwd = directory;

        self.cwd.clone()
    }
}

impl GithubVCS {
    pub fn new(api_token: String, user: String) -> GithubVCS {
        let agent = AgentBuilder::new().build();

        GithubVCS {
            api_token: api_token,
            user: user,
            agent: agent,
            remote_address: "git@github.com".to_string(),
            cwd: PathBuf::new(),
        }
    }
}
