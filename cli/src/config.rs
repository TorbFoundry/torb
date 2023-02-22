// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.0-02.22
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use serde::{Serialize, Deserialize};
use serde_yaml::{self};
use once_cell::sync::Lazy;
use std::fs;
use indexmap::IndexMap;

use crate::utils::{torb_path};

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Config {
    pub githubToken: String,
    pub githubUser: String,
    pub repositories: Option<IndexMap<String, String>>
}

impl Config {
    fn new() -> Config {
        let torb_path = torb_path();
        let config_path = torb_path.join("config.yaml");

        let conf_str = fs::read_to_string(config_path).expect("Failed to read config.yaml");

        serde_yaml::from_str(conf_str.as_str()).expect("Failed to parse config.yaml")
    }
}

pub static TORB_CONFIG: Lazy<Config> = Lazy::new(Config::new);