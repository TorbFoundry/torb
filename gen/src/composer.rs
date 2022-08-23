use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use thiserror::Error;
use std::collections::{HashSet, HashMap};
use std::fs;
use std::path::Path;
use crate::utils::{torb_path};


#[derive(Error, Debug)]
pub enum TorbComposerErrors {
    #[error("Environments folder not found. Please make sure Torb is correctly initialized.")]
    EnvironmentsNotFound
}

struct TFModule {
    name: String,
    source: String,
    version: Option<String>,
    variables: Option<HashMap<String, String>>,
}

struct TFMain {
    modules: Vec<TFModule>
}

pub struct Composer {
    hash: String,
    build_files_seen: HashSet<String>,
    fqn_seen: HashSet<String>,
    main_struct: TFMain
}

impl Composer {
    pub fn new(hash: String) -> Composer {
        Composer {
            hash: hash,
            build_files_seen: HashSet::new(),
            fqn_seen: HashSet::new()
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

        Ok(())
    }

    fn walk_artifact(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    fn copy_build_files(&mut self, node: &ArtifactNodeRepr) -> Result<bool, Box<dyn std::error::Error>> {
        let environment_path = torb_path().join("environments").join(&self.hash);
        let env_node_path = environment_path.join(&node.name);

        if !env_node_path.exists() {
            let error = format!("Failed to create new module directory in environment {}.", &self.hash);
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

    fn add_to_main_struct(&mut self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
