use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use crate::utils::torb_path;
use hcl::{Block, Body, Expression};
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
}

pub struct Composer {
    hash: String,
    build_files_seen: HashSet<String>,
    fqn_seen: HashSet<String>,
    main_struct: hcl::BodyBuilder,
}

impl Composer {
    pub fn new(hash: String) -> Composer {
        Composer {
            hash: hash,
            build_files_seen: HashSet::new(),
            fqn_seen: HashSet::new(),
            main_struct: Body::builder(),
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

        self.copy_supporting_build_files()
            .expect("Failed to write supporting buildfiles to new environment.");

        self.write_main_buildfile()
            .expect("Failed to write main buildfile to new environment.");

        Ok(())
    }

    fn copy_supporting_build_files(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = torb_path();
        let supporting_build_files_path = path.join("torb-artifacts/common");
        let new_environment_path = torb_path().join("environments").join(&self.hash);
        let dest = new_environment_path.join(supporting_build_files_path.as_path().file_name().unwrap());

        fs::create_dir(dest.clone()).expect("Unable to create supporting buildfile directory at destination, please check torb has been initialized properly.");

        self._copy_files_recursively(supporting_build_files_path, dest);

        Ok(())
    }

    fn _copy_files_recursively(&self, path: std::path::PathBuf, dest: std::path::PathBuf) -> () {
        let error_string = format!("Failed reading torb-artifacts dir: {}. Please check that torb is correctly initialized.", path.to_str().unwrap());
        for entry in path.read_dir().expect(&error_string) {
            let error_string = format!("Failed reading entry in torb-artifacts dir: {}. Please check that torb is correctly initialized.", path.to_str().unwrap());
            let entry = entry.expect(&error_string);
            if entry.path().is_dir() {
                let new_dest = dest.join(entry.path().file_name().unwrap());
                fs::create_dir(new_dest.clone()).expect("Unable to create supporting buildfile directory at destination, please check torb has been initialized properly.");
                self._copy_files_recursively(entry.path(), new_dest.clone())
            } else {
                let path = entry.path();
                println!("Copying {} to {}", path.display(), dest.display());
                let new_path = dest.join(path.file_name().unwrap());
                fs::copy(path, new_path).expect("Failed to copy supporting build file.");
            }
        }
    }

    fn write_main_buildfile(&mut self) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let builder = std::mem::take(&mut self.main_struct);
        let environment_path = torb_path().join("environments").join(&self.hash);
        let main_tf_path = environment_path.join("main.tf");

        let built_content = builder.build();

        let main_tf_content_hcl_string = hcl::to_string(&built_content)?;

        println!("{}", main_tf_content_hcl_string);

        fs::write(&main_tf_path, main_tf_content_hcl_string).expect("Failed to write main.tf");

        Ok(main_tf_path)
    }

    fn walk_artifact(&mut self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we compose the terraform environment.
        for child in node.dependencies.iter() {
            self.walk_artifact(child)?
        }

        if !self.build_files_seen.contains(&node.name) {
            self.copy_build_files_for_node(&node).and_then(|_out| {
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

        println!("Build file copying done.");

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

    // fn generate_output_data_blocks(&self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
    //     let mut output_data_blocks = Body::builder();

    //     for output in node.outputs.iter() {
    //         let output_data_block = Body::builder()
    //             .add("value", output.value.clone())
    //             .build();

    //         output_data_blocks.add(output.name.clone(), output_data_block);
    //     }

    //     Ok(())
    // }

    fn copy_build_files_for_node(
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
        let source = format!("./{}", node.name);
        let name = node.fqn.clone().replace(".", "_");
        let namespace = node.fqn.split(".").next().unwrap().to_string().replace("_", "-");

        let mut attributes = vec![
            ("source", source),
            (
                "release_name",
                node.deploy_steps["helm"]
                    .clone()
                    .unwrap()
                    .get("release_name")
                    .unwrap_or(&"".to_string())
                    .clone(),
            ),
            (
                "chart_name",
                node.deploy_steps["helm"].clone().unwrap()["chart"].clone(),
            ),
            (
                "repository",
                node.deploy_steps["helm"].clone().unwrap()["repository"].clone(),
            ),
            ("namespace", namespace),
        ];


        let module_version = node.deploy_steps["helm"]
                .clone()
                .unwrap()
                .get("version")
                .unwrap_or(&"".to_string())
                .clone();

        if module_version != "" {
            attributes.push(("version", module_version));
        }

        let builder = std::mem::take(&mut self.main_struct);

        self.main_struct = builder.add_block(
            Block::builder("module")
                .add_label(&name)
                .add_attributes(attributes)
                .add_attribute(("values", vec![""]))
                .build(),
        );

        Ok(())
    }
}
