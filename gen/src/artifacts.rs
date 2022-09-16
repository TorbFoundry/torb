use crate::resolver::{BuildStep, DependencyNode, StackGraph};
use crate::utils::torb_path;
use base64ct::{Base64UrlUnpadded, Encoding};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::{self};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;

#[derive(Serialize, Deserialize, Debug)]
pub struct ArtifactNodeRepr {
    pub fqn: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub build_step: Option<BuildStep>,
    pub deploy_steps: IndexMap<String, Option<IndexMap<String, String>>>,
    pub inputs: IndexMap<String, (String, String)>,
    pub outputs: IndexMap<String, String>,
    pub dependencies: Vec<ArtifactNodeRepr>,
    pub file_path: String,
}

impl ArtifactNodeRepr {
    fn new(
        fqn: String,
        name: String,
        version: String,
        kind: String,
        build_step: Option<BuildStep>,
        deploy_steps: IndexMap<String, Option<IndexMap<String, String>>>,
        inputs: IndexMap<String, (String, String)>,
        outputs: IndexMap<String, String>,
        file_path: String,
    ) -> ArtifactNodeRepr {
        ArtifactNodeRepr {
            fqn: fqn,
            name: name.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
            build_step: build_step,
            deploy_steps: deploy_steps,
            inputs: inputs,
            outputs: outputs,
            dependencies: Vec::new(),
            file_path,
        }
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
        }
    }
}

fn get_start_nodes(graph: &StackGraph) -> Vec<&DependencyNode> {
    let mut start_nodes = Vec::<&DependencyNode>::new();

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

    let meta = meta_into_artifact(&graph.meta)?;

    let mut artifact = ArtifactRepr::new(
        graph.version.clone(),
        graph.helm_version.clone(),
        graph.tf_version.clone(),
        graph.commit.clone(),
        graph.name.clone(),
        graph.stack_config.ingress,
        meta,
    );
    for node in start_nodes {
        let artifact_repr = walk_nodes(node, graph);
        artifact.deploys.push(artifact_repr);
    }

    Ok(artifact)
}

fn meta_into_artifact(
    meta: &Box<Option<DependencyNode>>,
) -> Result<Box<Option<ArtifactRepr>>, Box<dyn std::error::Error>> {
    let unboxed_meta = meta.as_ref();
    match unboxed_meta {
        Some(meta) => {
            let artifact = walk_graph(&meta.stack_graph.clone().unwrap())?;
            Ok(Box::new(Some(artifact)))
        }
        None => Ok(Box::new(None)),
    }
}

fn validate_and_map_inputs(node: &DependencyNode) -> IndexMap<String, (String, String)> {
    if node.input_spec.is_some() {
        let input_spec = &node.input_spec.clone().unwrap();

        match validate_inputs(&node.inputs, &input_spec) {
            Ok(_) => map_inputs(&node.inputs, &input_spec),
            Err(e) => panic!(
                "Input validation failed: {} is not a valid key. Valid Keys: {}",
                e,
                input_spec
                    .keys()
                    .into_iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<&str>>()
                    .join(", ")
            )
        }
    } else {
        if !node.inputs.is_empty() {
            println!("Warning: {} has inputs but no input spec, passing empty values.", node.fqn);
        }

        IndexMap::new()
    }
}

fn walk_nodes(node: &DependencyNode, graph: &StackGraph) -> ArtifactNodeRepr {    
    let inputs = validate_and_map_inputs(node);

    let mut artifact_node = ArtifactNodeRepr::new(
        node.fqn.to_string(),
        node.name.to_string(),
        node.version.to_string(),
        node.kind.to_string(),
        node.build_step.clone(),
        node.deploy_steps.clone(),
        inputs,
        node.outputs.clone(),
        node.file_path.clone(),
    );

    node.dependencies.projects.as_ref().map_or((), |projects| {
        for project in projects {
            let p_fqn = format!("{}.project.{}", graph.name.clone(), project.clone());
            let p_node = graph.projects.get(&p_fqn).unwrap();
            let p_node_repr = walk_nodes(p_node, graph);

            artifact_node.dependencies.push(p_node_repr);
        }
    });

    node.dependencies.services.as_ref().map_or((), |services| {
        for service in services {
            let s_fqn = format!("{}.service.{}", graph.name.clone(), service.clone());
            let s_node = graph.services.get(&s_fqn).unwrap();
            let s_node_repr = walk_nodes(s_node, graph);

            artifact_node.dependencies.push(s_node_repr);
        }
    });

    return artifact_node;
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

pub fn write_build_file(graph: StackGraph) -> (String, String, ArtifactRepr) {
    println!("Creating build file...");
    let artifact = walk_graph(&graph).unwrap();
    let string_rep = serde_yaml::to_string(&artifact).unwrap();
    let torb_path = torb_path();
    let outfile_dir_path = torb_path.join("buildfiles");
    let hash = Sha256::digest(string_rep.as_bytes());
    let hash_base64 = Base64UrlUnpadded::encode_string(&hash);
    let filename = format!("{}_{}.yaml", hash_base64, "outfile");
    let outfile_path = outfile_dir_path.join(&filename);

    if !outfile_dir_path.is_dir() {
        fs::create_dir(&outfile_dir_path).expect("Failed to create buildfile directory.");
    };

    println!("Writing buildfile to {}", outfile_path.display());
    fs::File::create(outfile_path)
        .and_then(|mut f| f.write(&string_rep.as_bytes()))
        .expect("Failed to create buildfile.");

    (hash_base64, filename, artifact)
}
