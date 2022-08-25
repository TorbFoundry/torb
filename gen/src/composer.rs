use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use crate::utils::torb_path;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbComposerErrors {
    #[error("Environments folder not found. Please make sure Torb is correctly initialized.")]
    EnvironmentsNotFound,
    #[error("Attempted to add module to TFMain before TFMain was created.")]
    TFMainNotInitialized,
}

struct TFModule {
    name: String,
    source: String,
    version: Option<String>,
    release_name: String,
    chart_name: String,
    repository: String,
    namespace: String,
    values: String,
}

struct TFMain {
    modules: Vec<TFModule>,
}

pub struct Composer {
    hash: String,
    build_files_seen: HashSet<String>,
    fqn_seen: HashSet<String>,
    main_struct: Option<TFMain>,
}

impl Composer {
    pub fn new(hash: String) -> Composer {
        Composer {
            hash: hash,
            build_files_seen: HashSet::new(),
            fqn_seen: HashSet::new(),
            main_struct: None,
        }
    }

    pub fn compose(&mut self, artifact: &ArtifactRepr) -> Result<(), Box<dyn std::error::Error>> {
        println!("Composing build environment...");
        let path = torb_path();
        let environments_path = path.join("environments");

        if !environments_path.exists() {
            return Err(Box::new(TorbComposerErrors::EnvironmentsNotFound));
        }
        let new_environment_path = environments_path.join(&self.hash);

        fs::create_dir(new_environment_path).expect("Failed to create new environment directory.");

        for node in artifact.deploys.iter() {
            self.walk_artifact(node)?;
        }

        self.write_main_buildfile()?;

        Ok(())
    }

    fn hashmap_from_tf_module(&self, module: &TFModule) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("name".to_string(), module.name.clone());
        map.insert("source".to_string(), module.source.clone());
        map.insert(
            "version".to_string(),
            module.version.clone().unwrap_or("".to_string()),
        );
        map.insert("release_name".to_string(), module.release_name.clone());
        map.insert("chart_name".to_string(), module.chart_name.clone());
        map.insert("repository".to_string(), module.repository.clone());
        map.insert("namespace".to_string(), module.namespace.clone());
        map.insert("values".to_string(), module.values.clone());
        return map;
    }

    fn write_main_buildfile(&mut self) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let environment_path = torb_path().join("environments").join(&self.hash);
        let main_tf_path = environment_path.join("main.tf");
        let mut main_tf_content =
            HashMap::<String, &HashMap<String, HashMap<String, String>>>::new();
        let mut modules = HashMap::<String, HashMap<String, String>>::new();
        for module_struct in self.main_struct.as_ref().unwrap().modules.iter() {
            let module = self.hashmap_from_tf_module(module_struct.clone());
            modules.insert(module_struct.name.clone(), module);
        }
        main_tf_content.insert("module".to_string(), &modules);

        let main_tf_content_json = serde_json::to_string(&main_tf_content).unwrap();
        let mut tempfile = NamedTempFile::new()?;

        tempfile.write_all(main_tf_content_json.as_bytes()).unwrap();

        let temp_path = tempfile.into_temp_path();

        let torb_path = torb_path();

        let mut command = Command::new("json2hcl");
        command
            .current_dir(torb_path)
            .arg(temp_path.to_str().unwrap());

        let output = command.output()?;
        let stdout = String::from_utf8(output.stdout).unwrap();

        fs::write(&main_tf_path, stdout).unwrap();

        Ok(main_tf_path)
    }

    fn walk_artifact(&mut self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we compose the terraform environment.
        for child in node.dependencies.iter() {
            self.walk_artifact(child)?
        }

        if !self.build_files_seen.contains(&node.name) {
            self.copy_build_files(&node).and_then(|_out| {
                if self.build_files_seen.insert(node.name.clone()) {
                    Ok(())
                } else {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Node build files already seen.",
                    )))
                }
            })?;
        }

        if !self.fqn_seen.contains(&node.fqn) {
            self.add_to_main_struct(node).and_then(|_out| {
                if self.fqn_seen.insert(node.fqn.clone()) {
                    Ok(())
                } else {
                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Node already seen.",
                    )))
                }
            })?;
        }

        Ok(())
    }

    fn copy_build_files(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let environment_path = torb_path().join("environments").join(&self.hash);
        let env_node_path = environment_path.join(&node.name);

        if !env_node_path.exists() {
            let error = format!(
                "Failed to create new module directory in environment {}.",
                &self.hash
            );
            fs::create_dir(&env_node_path).expect(&error);
        }

        let tf_path = Path::new(&node.file_path)
            .parent()
            .unwrap()
            .join("terraform/");

        if tf_path.exists() && tf_path.is_dir() {
            for f in fs::read_dir(tf_path)? {
                let f = f?;
                let path = f.path();
                let file_name = path.file_name().unwrap().to_str().unwrap();
                let new_path = env_node_path.join(file_name);
                fs::copy(path, new_path)?;
            }
        }

        Ok(true)
    }

    fn add_to_main_struct(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let torb_path = torb_path();
        let source = torb_path
            .join("environments")
            .join(&self.hash)
            .join(&node.name);
        let name = node.fqn.clone().replace(".", "_");
        let module = TFModule {
            name: name.clone(),
            source: source.to_str().unwrap().to_string(),
            version: None,
            release_name: node.deploy_steps["helm"].clone().unwrap()["release_name"].clone(),
            chart_name: node.deploy_steps["helm"].clone().unwrap()["chart_name"].clone(),
            repository: node.deploy_steps["helm"].clone().unwrap()["repository"].clone(),
            namespace: node.deploy_steps["helm"].clone().unwrap()["namespace"].clone(),
            values: "".to_string(),
        };

        if self.main_struct.is_none() {
            Err(Box::new(TorbComposerErrors::TFMainNotInitialized))
        } else {
            let main_struct = self.main_struct.as_mut().unwrap();
            main_struct.modules.push(module);
            Ok(())
        }
    }
}
