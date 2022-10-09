use utils::{torb_path};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    githubToken: String,
    githubUser: String
}

impl Config {
    fn new() -> Config {
        let torb_path = torb_path();
        let config_path = torb_path.join("config.yaml");

        let conf_str = fs::read_to_string(config_path);

        serde::from(conf_str);
    }
}