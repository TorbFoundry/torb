use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbVSCErrors {
    #[error("Cannot create repo directory at: {path:?}, reason: {response:?}")]
    UnableToCreateLocalRepoDir { path: PathBuf, response: String },

    #[error("Unable to init local git repo, reason: {response:?}")]
    UnableToInitLocalGitRepo { response: String },

    #[error("Unable to create remote repo: {name:?}, reason: {response:?}")]
    UnableToCreateRemoteRepo { name: String, response: String },
}

trait GitVersionControl {
    fn create_remote_repo(
        &self,
        name: &str,
    ) -> Result<String, Box<dyn std::error::Error>>;
    fn create_local_repo(
        &self,
        path: &PathBuf,
        remote: Option<&str>,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mkdir = Command::new("mkdir")
            .arg(path)
            .output()
            .expect("Failed to create directory.");

        if mkdir.status.success() {
            let error_msg = format!("Failed to init git repo at path: {:?}", path);
            let git_command = Command::new("git")
                .arg("init")
                .current_dir(path)
                .output()
                .expect(&error_msg);

            if git_command.status.success() {
                if let Some(remote) = remote {
                    let error_msg = format!("Failed to add remote: {:?}", remote);
                    let git_remote_command = Command::new("git")
                        .arg("remote")
                        .arg("add")
                        .arg("origin")
                        .arg(remote)
                        .current_dir(path)
                        .output()
                        .expect(&error_msg);

                    if git_remote_command.status.success() {
                        Ok(path.clone())
                    } else {
                        Err(Box::new(TorbVSCErrors::UnableToInitLocalGitRepo {
                            response: String::from_utf8(git_remote_command.stderr).unwrap(),
                        }))
                    }
                } else {
                    Ok(path.clone())
                }
            } else {
                Err(Box::new(TorbVSCErrors::UnableToCreateLocalRepoDir {
                    path: path.clone(),
                    response: String::from_utf8(git_command.stderr).unwrap(),
                }))
            }
        } else {
            let err = TorbVSCErrors::UnableToInitLocalGitRepo {
                response: std::str::from_utf8(&mkdir.stderr)?.to_string(),
            };

            Err(Box::new(err))
        }
    }

    fn create_repo(
        &self,
        name: &str,
        local_path: &str,
        local_only: bool,
    ) -> Result<(PathBuf, String), Box<dyn Error>> {
        let mut path_buf = std::path::PathBuf::new();
        path_buf.push(local_path);

        if local_only {
            Ok((self.create_local_repo(&path_buf, None)?, "".to_string()))
        } else {
            let remote = self.create_remote_repo(name)?;

            Ok((
                self.create_local_repo(&path_buf, Some(name))?,
                remote,
            ))
        }
    }

    fn _get_api_token(&self) -> String;
    fn get_api_token(&self) -> String {
        self._get_api_token()
    }

    fn _get_user(&self) -> String;
    fn get_user(&self) -> String {
        self._get_user()
    }
}

struct GithubVSC {
    api_token: String,
    user: String
}

impl GitVersionControl for GithubVSC {
    fn create_remote_repo(
        &self,
        name: &str
    ) -> Result<String, Box<dyn std::error::Error>> {
        let token = self.get_api_token();
        /*
        The amount of HTTP requests at the cli level should be fairly low and not take much time.
        With that consideration taking on the overhead of an async runtime which is a heavy dependency, 
        and an async client with the changes to a rust project needed to typically support async does not 
        seem like the right move to me. - Ian
        */
        let user = self.get_user();
        let req_string = format!("https://api.github.com/user/{}/repos?access_token={}", user, token);
        let resp: String = ureq::post(&req_string)
        .send_json(ureq::json!({
            "name": name,
            "private": true,
            "auto_init": true
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
}

impl GithubVSC {
    fn new(api_token: String, user: String) -> Self {
        GithubVSC {
            api_token: api_token,
            user: user
        }
    }
}
