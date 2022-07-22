use serde_yaml;
use std::collections::HashMap;

pub struct ResolverConfig {
    autoaccept: bool,
    stack_path: String,
    stack_name: String,
    stack_description: String,
    stack_contents: String,
    torb_version: String,
}

impl ResolverConfig {
    pub fn new(autoaccept: bool, stack_path: String, stack_name: String, stack_description: String, stack_contents: String, torb_version: String) -> ResolverConfig {
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

struct DependencyNode {
    name: String,
    steps: HashMap<String, String>,
    dependencies: Vec<DependencyNode>,
}

impl DependencyNode {
    pub fn new(name: String, steps: HashMap<String, String>, dependencies: Vec<DependencyNode>) -> DependencyNode {
        DependencyNode {
            name,
            steps,
            dependencies,
        }
    }

    pub fn add_dependency(&mut self, dependency: DependencyNode) {
        self.dependencies.push(dependency);
    }
}

struct DependencyGraph {
    head: DependencyNode,
    nodes: Vec<DependencyNode>
}

impl DependencyGraph {
    pub fn new(head: DependencyNode) -> DependencyGraph {
        let graph_nodes = Vec::new();
        graph_nodes.push(head);
        DependencyGraph {
            nodes: graph_nodes,
            head: head
        }
    }

    pub fn add_node(&mut self, node: DependencyNode) {
        self.nodes.push(node);
    }
}


pub struct Resolver {
    config: ResolverConfig,
}

impl Resolver {
    pub fn new(config: ResolverConfig) -> Resolver {
        Resolver {
            config: config,
        }
    }

    pub fn resolve(&self) -> Result<(), String> {
        let mut graph = self.build_graph();
        self.resolve_graph(&mut graph)
    }

    fn build_graph(&self) -> DependencyGraph {
        let mut nodes = Vec::new();
        let mut head = DependencyNode::new();
        nodes.push(head);
        DependencyGraph {
            head,
            nodes
        }
    }
}