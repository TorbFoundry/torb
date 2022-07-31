use crate::resolver::{StackGraph, DependencyNode, BuildStep};
use serde_yaml::{self};
use std::{collections::HashMap};
use serde::{Deserialize, Serialize};
use std::fs;

const TORB_PATH: &str = ".torb";

fn torb_path() -> std::path::PathBuf {
    let home_dir = dirs::home_dir().unwrap();
    home_dir.join(TORB_PATH)
}

#[derive(Serialize, Deserialize)]
struct ArtifactNodeRepr {
    fqn: String,
    name: String,
    version: String,
    kind: String,
    build_step: Option<BuildStep>,
    deploy_steps: HashMap<String, Option<HashMap<String, String>>>,
    dependencies: Vec<ArtifactNodeRepr>,
}

impl ArtifactNodeRepr {
    fn new(fqn: String, name: String, version: String, kind: String, build_step: Option<BuildStep>, deploy_steps: HashMap<String, Option<HashMap<String, String>>>) -> ArtifactNodeRepr {
        ArtifactNodeRepr {
            fqn: fqn,
            name: name.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
            build_step: build_step,
            deploy_steps: deploy_steps,
            dependencies: Vec::new()
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ArtifactRepr {
    torb_version: String,
    helm_version: String,
    terraform_version: String,
    git_commit: String,
    stack_name: String,
    ingress: bool,
    meta: String,
    deploys: Vec<ArtifactNodeRepr>,
}

impl ArtifactRepr {
    fn new(
        torb_version: String,
        helm_version: String,
        terraform_version: String,
        git_commit: String,
        stack_name: String,
        ingress: bool,
        meta: String,
    ) -> ArtifactRepr {
        ArtifactRepr {
            torb_version,
            helm_version,
            terraform_version,
            git_commit,
            stack_name,
            ingress,
            meta,
            deploys: Vec::new()
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
            _ => panic!("Build artifact generation, unknown kind: {}", kind)
        };

        if list.len() == 0 {
            start_nodes.push(node);
        }
    }

    start_nodes
}

fn walk_graph(graph: &StackGraph) -> ArtifactRepr {
    let start_nodes = get_start_nodes(graph);
    let mut artifact = ArtifactRepr::new(
        graph.version.clone(),
        graph.helm_version.clone(),
        graph.tf_version.clone(),
        graph.commit.clone(),
        graph.name.clone(),
        graph.stack_config.ingress,
        graph.stack_config.meta.clone(),
    );
    for node in start_nodes {
        let artifact_repr = walk_nodes(node, graph);
        artifact.deploys.push(artifact_repr);
    }

    artifact
}

fn walk_nodes(node: &DependencyNode, graph: &StackGraph) -> ArtifactNodeRepr {
    let mut artifact_node = ArtifactNodeRepr::new(
        node.fqn.to_string(),
        node.name.to_string(),
        node.version.to_string(),
        node.kind.to_string(),
        node.build_step.clone(),
        node.deploy_steps.clone(),
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

pub fn write_build_file(graph: StackGraph) {
    let artifact = walk_graph(&graph);
    let string_rep = serde_yaml::to_string(&artifact).unwrap();
    let torb_path = torb_path();
    let outfile_dir_path = torb_path.join("buildfiles");
    let outfile_path = outfile_dir_path.join(format!("{}.yaml", "outfile"));

    if !outfile_dir_path.is_dir() {
        fs::create_dir(&outfile_dir_path).unwrap();
    };

    fs::write(&outfile_path, string_rep).unwrap();
}