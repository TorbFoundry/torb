use crate::resolver::{DependencyNodeDependencies, StackGraph, resolve_stack};
use crate::utils::{checksum, buildstate_path_or_create};
use base64ct::{Base64UrlUnpadded, Encoding};
use indexmap::IndexMap;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_yaml::{self};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbArtifactErrors {
    #[error("Hash of loaded build file does not match hash of file on disk.")]
    LoadChecksumFailed,

}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InitStep {
    pub steps: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BuildStep {
    #[serde(default = "String::new")]
    pub script_path: String,
    #[serde(default = "String::new")]
    pub dockerfile: String,
    #[serde(default = "String::new")]
    pub tag: String,
    #[serde(default = "String::new")]
    pub registry: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArtifactNodeRepr {
    #[serde(default = "String::new")]
    pub fqn: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub lang: Option<String>,
    #[serde(alias = "init")]
    pub init_step: Option<Vec<String>>,
    #[serde(alias = "build")]
    pub build_step: Option<BuildStep>,
    #[serde(alias = "deploy")]
    pub deploy_steps: IndexMap<String, Option<IndexMap<String, String>>>,
    #[serde(default = "IndexMap::new", rename(serialize = "inputs"))]
    pub mapped_inputs: IndexMap<String, (String, String)>,
    #[serde(alias = "inputs", skip_serializing, skip_deserializing, default = "IndexMap::new")]
    pub input_spec: IndexMap<String, String>,
    #[serde(default = "Vec::new")]
    pub outputs: Vec<String>,
    #[serde(default = "Vec::new")]
    pub dependencies: Vec<ArtifactNodeRepr>,
    #[serde(skip)]
    pub dependency_names: DependencyNodeDependencies,
    #[serde(default = "String::new")]
    pub file_path: String,
    #[serde(skip)]
    pub stack_graph: Option<StackGraph>,
    pub files: Option<Vec<String>>,
}

impl ArtifactNodeRepr {
    pub fn new(
        fqn: String,
        name: String,
        version: String,
        kind: String,
        lang: Option<String>,
        init_step: Option<Vec<String>>,
        build_step: Option<BuildStep>,
        deploy_steps: IndexMap<String, Option<IndexMap<String, String>>>,
        inputs: IndexMap<String, (String, String)>,
        input_spec: IndexMap<String, String>,
        outputs: Vec<String>,
        file_path: String,
        stack_graph: Option<StackGraph>,
        files: Option<Vec<String>>,
    ) -> ArtifactNodeRepr {
        ArtifactNodeRepr {
            fqn: fqn,
            name: name,
            version: version,
            kind: kind,
            lang: lang,
            init_step: init_step,
            build_step: build_step,
            deploy_steps: deploy_steps,
            mapped_inputs: inputs,
            input_spec: input_spec,
            outputs: outputs,
            dependencies: Vec::new(),
            dependency_names: DependencyNodeDependencies {
                services: None,
                projects: None,
                stacks: None,
            },
            file_path,
            stack_graph,
            files
        }
    }

    pub fn validate_map_and_set_inputs(&mut self, inputs: IndexMap<String, String>) {
        if !self.input_spec.is_empty() {
            let input_spec = &self.input_spec.clone();

            match ArtifactNodeRepr::validate_inputs(&inputs, &input_spec) {
                Ok(_) => {
                    self.mapped_inputs = ArtifactNodeRepr::map_inputs(&inputs, &input_spec);
                }
                Err(e) => panic!(
                    "Input validation failed: {} is not a valid key. Valid Keys: {}",
                    e,
                    input_spec
                        .keys()
                        .into_iter()
                        .map(AsRef::as_ref)
                        .collect::<Vec<&str>>()
                        .join(", ")
                ),
            }
        } else {
            if !inputs.is_empty() {
                println!(
                    "Warning: {} has inputs but no input spec, passing empty values.",
                    &self.fqn
                );
            }

            self.mapped_inputs = IndexMap::<String, (String, String)>::new();
        }
    }

    fn validate_inputs(
        inputs: &IndexMap<String, String>,
        spec: &IndexMap<String, String>,
    ) -> Result<(), String> {
        for (key, _) in inputs.iter() {
            if !spec.contains_key(key) {
                return Err(key.clone());
            }
        }

        Ok(())
    }

    fn map_inputs(
        inputs: &IndexMap<String, String>,
        spec: &IndexMap<String, String>,
    ) -> IndexMap<String, (String, String)> {
        let mut mapped_inputs = IndexMap::<String, (String, String)>::new();

        for (key, value) in inputs.iter() {
            let spec_value = spec.get(key).unwrap();
            mapped_inputs.insert(key.to_string(), (spec_value.to_string(), value.to_string()));
        }

        mapped_inputs
    }
}

#[derive(Serialize, Deserialize)]
pub struct ArtifactRepr {
    pub torb_version: String,
    pub helm_version: String,
    pub terraform_version: String,
    pub git_commit: String,
    pub stack_name: String,
    pub ingress: bool,
    pub meta: Box<Option<ArtifactRepr>>,
    pub deploys: Vec<ArtifactNodeRepr>,
    #[serde(skip)]
    pub nodes: HashMap<String, ArtifactNodeRepr>
}

impl ArtifactRepr {
    fn new(
        torb_version: String,
        helm_version: String,
        terraform_version: String,
        git_commit: String,
        stack_name: String,
        ingress: bool,
        meta: Box<Option<ArtifactRepr>>,
    ) -> ArtifactRepr {
        ArtifactRepr {
            torb_version,
            helm_version,
            terraform_version,
            git_commit,
            stack_name,
            ingress,
            meta,
            deploys: Vec::new(),
            nodes: HashMap::new()
        }
    }
}

fn get_start_nodes(graph: &StackGraph) -> Vec<&ArtifactNodeRepr> {
    let mut start_nodes = Vec::<&ArtifactNodeRepr>::new();

    for (fqn, list) in graph.incoming_edges.iter() {
        let kind = fqn.split(".").collect::<Vec<&str>>()[1];
        let node = match kind {
            "project" => graph.projects.get(fqn).unwrap(),
            "service" => graph.services.get(fqn).unwrap(),
            "stack" => graph.stacks.get(fqn).unwrap(),
            _ => panic!("Build artifact generation, unknown kind: {}", kind),
        };

        if list.len() == 0 {
            start_nodes.push(node);
        }
    }

    start_nodes
}

fn walk_graph(graph: &StackGraph) -> Result<ArtifactRepr, Box<dyn std::error::Error>> {
    let start_nodes = get_start_nodes(graph);

    let meta = stack_into_artifact(&graph.meta)?;

    let mut artifact = ArtifactRepr::new(
        graph.version.clone(),
        graph.helm_version.clone(),
        graph.tf_version.clone(),
        graph.commit.clone(),
        graph.name.clone(),
        graph.stack_config.ingress,
        meta,
    );

    let mut node_map: HashMap<String, ArtifactNodeRepr> = HashMap::new();

    for node in start_nodes {
        let artifact_node_repr = walk_nodes(node, graph, &mut node_map);
        artifact.deploys.push(artifact_node_repr);
    }
    
    artifact.nodes = node_map;

    Ok(artifact)
}

pub fn stack_into_artifact(meta: &Box<Option<ArtifactNodeRepr>>) -> Result<Box<Option<ArtifactRepr>>, Box<dyn std::error::Error>> {
    let unboxed_meta = meta.as_ref();
    match unboxed_meta {
        Some(meta) => {
            let artifact = walk_graph(&meta.stack_graph.clone().unwrap())?;
            Ok(Box::new(Some(artifact)))
        },
        None => { Ok(Box::new(None)) }
    }
}

fn walk_nodes(node: &ArtifactNodeRepr, graph: &StackGraph, node_map: &mut HashMap<String, ArtifactNodeRepr>) -> ArtifactNodeRepr {
    let mut new_node = node.clone();
    new_node.dependency_names.projects.as_ref().map_or((), |projects| {
        for project in projects {
            let p_fqn = format!("{}.project.{}", graph.name.clone(), project.clone());
            let p_node = graph.projects.get(&p_fqn).unwrap();
            let p_node_repr = walk_nodes(p_node, graph, node_map);

            new_node.dependencies.push(p_node_repr);
        }
    });

    new_node.dependency_names.services.as_ref().map_or((), |services| {
        for service in services {
            let s_fqn = format!("{}.service.{}", graph.name.clone(), service.clone());
            let s_node = graph.services.get(&s_fqn).unwrap();
            let s_node_repr = walk_nodes(s_node, graph, node_map);

            new_node.dependencies.push(s_node_repr);
        }
    });

    node_map.insert(node.fqn.clone(), new_node.clone());

    return new_node;
}


pub fn load_build_file(filename: String) -> Result<(String, String, ArtifactRepr), Box<dyn std::error::Error>> {
    let buildstate_path = buildstate_path_or_create();
    let buildfiles_path = buildstate_path.join("buildfiles");
    let path = buildfiles_path.join(filename.clone());


    let file = std::fs::File::open(path)?;

    let hash = filename.clone().split("_").collect::<Vec<&str>>()[0].to_string();

    let reader = std::io::BufReader::new(file);

    let artifact: ArtifactRepr = serde_yaml::from_reader(reader)?;
    let string_rep = serde_yaml::to_string(&artifact).unwrap();

    if checksum(string_rep, hash.clone()) {
        Ok((hash, filename, artifact))
    } else {
        Err(Box::new(TorbArtifactErrors::LoadChecksumFailed))
    }
}

pub fn deserialize_stack_yaml_into_artifact(stack_yaml: &String) -> Result<ArtifactRepr, Box<dyn std::error::Error>> {
    let graph: StackGraph = resolve_stack(stack_yaml)?;
    let artifact = walk_graph(&graph)?;
    Ok(artifact)
}

pub fn get_build_file_info(artifact: &ArtifactRepr) -> Result<(String, String, String), Box<dyn std::error::Error>> {
    let string_rep = serde_yaml::to_string(&artifact).unwrap();
    let hash = Sha256::digest(string_rep.as_bytes());
    let hash_base64 = Base64UrlUnpadded::encode_string(&hash);
    let filename = format!("{}_{}.yaml", hash_base64, "outfile");

    Ok((hash_base64, filename, string_rep))
}

pub fn write_build_file(stack_yaml: String) -> (String, String, ArtifactRepr) {
    let artifact = deserialize_stack_yaml_into_artifact(&stack_yaml).unwrap();
    let current_dir = std::env::current_dir().unwrap();
    let current_dir_state_dir = current_dir.join(".torb_buildstate");
    let outfile_dir_path = current_dir_state_dir.join("buildfiles");
    let (hash_base64, filename, artifact_as_string) = get_build_file_info(&artifact).unwrap();
    let outfile_path = outfile_dir_path.join(&filename);

    if !outfile_dir_path.is_dir() {
        fs::create_dir(&outfile_dir_path).expect("Failed to create buildfile directory.");
    };

    if outfile_path.exists() {
        println!("Build file already exists with same hash, skipping write.");
    } else {
        println!("Writing buildfile to {}", outfile_path.display());
        fs::File::create(outfile_path)
            .and_then(|mut f| f.write(&artifact_as_string.as_bytes()))
            .expect("Failed to create buildfile.");
    }


    (hash_base64, filename, artifact)
}
