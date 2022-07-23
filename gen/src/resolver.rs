use serde_yaml::{self, Value};
use std::{collections::HashMap, collections::VecDeque};

pub struct ResolverConfig {
    autoaccept: bool,
    stack_path: String,
    stack_name: String,
    stack_description: String,
    stack_contents: String,
    torb_version: String,
}

impl ResolverConfig {
    pub fn new(
        autoaccept: bool,
        stack_path: String,
        stack_name: String,
        stack_description: String,
        stack_contents: String,
        torb_version: String,
    ) -> ResolverConfig {
        ResolverConfig {
            autoaccept,
            stack_path,
            stack_name,
            stack_description,
            stack_contents,
            torb_version,
        }
    }
}

struct DeployStep {
    name: String,
    tool_version: String,
    tool_name: String,
    tool_config: HashMap<String, String>,
}
struct BuildStep {
    dockerfile: String,
    registry: String,
}

struct StackConfig {
    meta: String,
    ingress: bool,
}

struct DependencyNode {
    name: String,
    deploy_steps: HashMap<String, DeployStep>,
    build_step: Option<BuildStep>,
    dependencies: Vec<DependencyNode>,
    version: String,
    kind: String,
}

impl DependencyNode {
    pub fn new(
        name: String,
        deploy_steps: HashMap<String, DeployStep>,
        build_step: Option<BuildStep>,
        dependencies: Vec<DependencyNode>,
        version: String,
        kind: String,
    ) -> DependencyNode {
        DependencyNode {
            name,
            deploy_steps: deploy_steps,
            build_step: build_step,
            dependencies,
            version,
            kind,
        }
    }

    pub fn add_dependency(&mut self, dependency: DependencyNode) {
        self.dependencies.push(dependency);
    }
}

struct StackGraph {
    head: DependencyNode,
    nodes: Vec<DependencyNode>,
    stack_config: StackConfig,
}

impl StackGraph {
    pub fn new(head: DependencyNode, stack_config: StackConfig) -> StackGraph {
        let graph_nodes = Vec::new();
        graph_nodes.push(head);
        StackGraph {
            nodes: graph_nodes,
            head,
            stack_config,
        }
    }

    pub fn add_node(&mut self, node: DependencyNode) {
        self.nodes.push(node);
    }
}

pub struct Resolver {
    config: ResolverConfig,
    stack: Value,
}

impl Resolver {
    pub fn new(config: ResolverConfig) -> Resolver {
        Resolver {
            config: config,
            stack: serde_yaml::from_str(config.stack_contents.as_str()).unwrap(),
        }
    }

    pub fn resolve(&self, yaml: serde_yaml::Value) -> Result<(), String> {
        let mut graph = self.build_graph(yaml);
        // self.resolve_graph(&mut graph)

        Ok(())
    }

    fn build_graph(
        &self,
        yaml: serde_yaml::Value,
    ) -> Result<StackGraph, Box<dyn std::error::Error>> {
        let meta = serde_yaml::to_string(&yaml["config"]["meta"]).unwrap();
        let ingress = serde_yaml::to_string(&yaml["config"]["ingress"])
            .unwrap()
            .parse::<bool>()
            .unwrap();
        let mut graph = StackGraph::new(
            self.build_node(yaml),
            StackConfig {
                meta: meta,
                ingress: ingress,
            },
        );
        let mut nodes = Vec::new();
        let mut head = DependencyNode::new();
        nodes.push(head);
        StackGraph { head, nodes }
    }
}
