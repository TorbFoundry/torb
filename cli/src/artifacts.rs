use crate::resolver::inputs::InputResolver;
use crate::resolver::{NodeDependencies, StackGraph, resolve_stack};
use crate::utils::{checksum, buildstate_path_or_create};
use crate::composer::{InputAddress};

use data_encoding::BASE32;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::{self};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use thiserror::Error;
use memorable_wordlist;

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
    #[serde(default = "IndexMap::new")]
    pub mapped_inputs: IndexMap<String, (String, String)>,
    #[serde(alias = "inputs", default = "IndexMap::new")]
    pub input_spec: IndexMap<String, String>,
    #[serde(default = "Vec::new")]
    pub outputs: Vec<String>,
    #[serde(default = "Vec::new")]
    pub dependencies: Vec<ArtifactNodeRepr>,
    #[serde(default = "HashSet::new")]
    pub implicit_dependency_names: HashSet<String>,
    #[serde(skip)]
    pub dependency_names: NodeDependencies,
    #[serde(default = "String::new")]
    pub file_path: String,
    #[serde(skip)]
    pub stack_graph: Option<StackGraph>,
    pub files: Option<Vec<String>>,
    #[serde(default = "String::new")]
    pub values: String,
    pub namespace: Option<String>,
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
        values: String,
        namespace: Option<String>,
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
            implicit_dependency_names: HashSet::new(),
            dependencies: Vec::new(),
            dependency_names: NodeDependencies {
                services: None,
                projects: None,
                stacks: None,
            },
            file_path,
            stack_graph,
            files,
            values,
            namespace
        }
    }

    fn address_to_fqn(graph_name: &String, addr_result: Result<InputAddress, String>) -> Option<String> {
        match addr_result {
            Ok(addr) => {
                let fqn = format!("{}.{}.{}", graph_name, addr.node_type.clone(), addr.node_name.clone());

                Some(fqn)
            },
            Err(_s) => {
                None
            }
        }
    }

    pub fn discover_and_set_implicit_dependencies(&mut self, graph_name: &String) -> Result<(), Box<dyn std::error::Error>> {
        let mut implicit_deps_inputs = HashSet::new();

        let inputs_fn = |_spec: &String, val: Result<InputAddress, String>| -> String {
            let fqn_option = ArtifactNodeRepr::address_to_fqn(graph_name, val);

            if fqn_option.is_some() {
                let fqn = fqn_option.unwrap();
                implicit_deps_inputs.insert(fqn);
            };

            "".to_string()
        };

        let mut implicit_deps_values = HashSet::new();

        let values_fn = |addr: Result<InputAddress, String>| -> String {
            let fqn_option = ArtifactNodeRepr::address_to_fqn(graph_name, addr);

            if fqn_option.is_some() {
                let fqn = fqn_option.unwrap();
                implicit_deps_values.insert(fqn);
            };

            "".to_string()
        };

        let (_, _) = InputResolver::resolve(&self, Some(values_fn), Some(inputs_fn))?;

        let unioned_deps = implicit_deps_inputs.union(&mut implicit_deps_values);

        self.implicit_dependency_names = unioned_deps.cloned().collect();

        Ok(())
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
    pub meta: Box<Option<ArtifactRepr>>,
    pub deploys: Vec<ArtifactNodeRepr>,
    pub nodes: IndexMap<String, ArtifactNodeRepr>,
    pub namespace: Option<String>,
    pub release: Option<String>
}

impl ArtifactRepr {
    fn new(
        torb_version: String,
        helm_version: String,
        terraform_version: String,
        git_commit: String,
        stack_name: String,
        meta: Box<Option<ArtifactRepr>>,
        namespace: Option<String>,
        release: Option<String>,
    ) -> ArtifactRepr {
        ArtifactRepr {
            torb_version,
            helm_version,
            terraform_version,
            git_commit,
            stack_name,
            meta,
            deploys: Vec::new(),
            nodes: IndexMap::new(),
            namespace: namespace,
            release: release,
        }
    }

    pub fn namespace(&self, node: &ArtifactNodeRepr) -> String {
        let mut namespace = node
            .fqn
            .split(".")
            .next()
            .unwrap()
            .to_string()
            .replace("_", "-");

        if self.namespace.is_some() {
            namespace = self.namespace.clone().unwrap();
        }

        if node.namespace.is_some() {
            namespace = node.namespace.clone().unwrap();
        }

        namespace
    }

    pub fn release(&self) -> String {
        if self.release.is_some() {
            self.release.clone().unwrap()
        } else {
            memorable_wordlist::kebab_case(16)
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

    start_nodes.sort_by(|a, b| b.fqn.cmp(&a.fqn));
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
        meta,
        graph.namespace.clone(),
        graph.release.clone()
    );

    let mut node_map: IndexMap<String, ArtifactNodeRepr> = IndexMap::new();

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

fn walk_nodes(node: &ArtifactNodeRepr, graph: &StackGraph, node_map: &mut IndexMap<String, ArtifactNodeRepr>) -> ArtifactNodeRepr {
    let mut new_node = node.clone();

    for fqn in new_node.implicit_dependency_names.iter() {
        let kind = fqn.split(".").collect::<Vec<&str>>()[1];
        let node = match kind {
            "project" => graph.projects.get(fqn).unwrap(),
            "service" => graph.services.get(fqn).unwrap(),
            "stack" => graph.stacks.get(fqn).unwrap(),
            _ => panic!("Build artifact generation, unknown kind: {}", kind),
        };

        let node_repr = walk_nodes(node, graph, node_map);

        new_node.dependencies.push(node_repr)
    }

    new_node.dependency_names.projects.as_ref().map_or((), |projects| {
        for project in projects {
            let p_fqn = format!("{}.project.{}", graph.name.clone(), project.clone());

            if !new_node.implicit_dependency_names.contains(&p_fqn) {
                let p_node = graph.projects.get(&p_fqn).unwrap();
                let p_node_repr = walk_nodes(p_node, graph, node_map);

                new_node.dependencies.push(p_node_repr);
            }
        }
    });

    new_node.dependency_names.services.as_ref().map_or((), |services| {
        for service in services {
            let s_fqn = format!("{}.service.{}", graph.name.clone(), service.clone());

            if !new_node.implicit_dependency_names.contains(&s_fqn) {
                let s_node = graph.services.get(&s_fqn).unwrap();
                let s_node_repr = walk_nodes(s_node, graph, node_map);

                new_node.dependencies.push(s_node_repr);
            }
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
    let hash_base32 = BASE32.encode(&hash);
    let filename = format!("{}_{}.yaml", hash_base32, "outfile");

    Ok((hash_base32, filename, string_rep))
}

pub fn write_build_file(stack_yaml: String) -> (String, String, ArtifactRepr) {
    let artifact = deserialize_stack_yaml_into_artifact(&stack_yaml).unwrap();
    let current_dir = std::env::current_dir().unwrap();
    let current_dir_state_dir = current_dir.join(".torb_buildstate");
    let outfile_dir_path = current_dir_state_dir.join("buildfiles");
    let (hash_base32, filename, artifact_as_string) = get_build_file_info(&artifact).unwrap();
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


    (hash_base32, filename, artifact)
}
