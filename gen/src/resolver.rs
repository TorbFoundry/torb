use serde::{Deserialize, Serialize};
use serde_yaml::{self, Value};
use std::{
    collections::HashMap,
    error::Error,
    path::{PathBuf},
};
use thiserror::Error;
use std::process::Command;

const TORB_PATH: &str = ".torb";

fn torb_path() -> std::path::PathBuf {
    let home_dir = dirs::home_dir().unwrap();
    home_dir.join(TORB_PATH)
}

#[derive(Error, Debug)]
pub enum TorbResolverErrors {
    #[error(
        "Unable to parse stack manifest, please check that it is a valid Torb stack manifest."
    )]
    CannotParseStackManifest,
}

#[derive(Clone)]
pub struct ResolverConfig {
    autoaccept: bool,
    stack_name: String,
    stack_description: String,
    stack_contents: serde_yaml::Value,
    torb_version: String,
}

impl ResolverConfig {
    pub fn new(
        autoaccept: bool,
        stack_name: String,
        stack_description: String,
        stack_contents: serde_yaml::Value,
        torb_version: String,
    ) -> ResolverConfig {
        ResolverConfig {
            autoaccept,
            stack_name,
            stack_description,
            stack_contents,
            torb_version,
        }
    }
}

// #[derive(Serialize, Deserialize, Clone)]
// pub struct DeployStep {
//     name: String,
//     tool_version: String,
//     tool_name: String,
//     tool_config: HashMap<String, String>,
// }
#[derive(Serialize, Deserialize, Clone)]
pub struct BuildStep {
    dockerfile: String,
    registry: String,
}

#[derive(Clone)]
pub struct StackConfig {
    meta: String,
    ingress: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DependencyNode {
    version: String,
    kind: String,
    name: String,
    #[serde(rename(deserialize = "deploy"))]
    deploy_steps: HashMap<String, Option<HashMap<String, String>>>,
    #[serde(rename(deserialize = "build"))]
    build_step: Option<BuildStep>,
    #[serde(skip)]
    stack_graph: Option<StackGraph>,
    #[serde(skip)]
    dependencies: DependencyNodeDependencies,
    #[serde(skip)]
    fqn: String
}

impl DependencyNode {
    pub fn new(
        name: String,
        deploy_steps: HashMap<String, Option<HashMap<String, String>>>,
        build_step: Option<BuildStep>,
        version: String,
        kind: String,
        stack_graph: Option<StackGraph>,
        dependencies: DependencyNodeDependencies,
    ) -> DependencyNode {
        let fqn = "".to_string();
        DependencyNode {
            name,
            deploy_steps: deploy_steps,
            build_step: build_step,
            version,
            kind,
            stack_graph,
            dependencies,
            fqn
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct DependencyNodeDependencies {
    services: Option<Vec<String>>,
    projects: Option<Vec<String>>,
    stacks: Option<Vec<String>>,
}

impl DependencyNodeDependencies {
    pub fn new() -> DependencyNodeDependencies {
        DependencyNodeDependencies {
            services: None,
            projects: None,
            stacks: None,
        }
    }
}

#[derive(Clone)]
pub struct StackGraph {
    stack_config: StackConfig,
    services: HashMap<String, DependencyNode>,
    projects: HashMap<String, DependencyNode>,
    stacks: HashMap<String, DependencyNode>,
    name: String,
    version: String,
    kind: String,
    commit: String,
    tf_version: String,
    helm_version: String,
}

impl StackGraph {
    pub fn new(
        stack_config: StackConfig,
        name: String,
        kind: String,
        version: String,
        commit: String,
        tf_version: String,
        helm_version: String,
    ) -> StackGraph {
        StackGraph {
            services: HashMap::<String, DependencyNode>::new(),
            projects: HashMap::<String, DependencyNode>::new(),
            stacks: HashMap::<String, DependencyNode>::new(),
            stack_config,
            name,
            version,
            kind,
            tf_version,
            helm_version,
            commit,
        }
    }

    pub fn add_service(&mut self, node: &DependencyNode) {
        self.services.insert(node.name.clone(), node.clone());
    }
    pub fn add_project(&mut self, node: &DependencyNode) {
        self.projects.insert(node.name.clone(), node.clone());
    }
    pub fn add_stack(&mut self, node: &DependencyNode) {
        self.stacks.insert(node.name.clone(), node.clone());
    }
}

pub struct Resolver {
    config: ResolverConfig,
    stack: Value,
}

impl Resolver {
    pub fn new(config: &ResolverConfig) -> Resolver {
        Resolver {
            config: config.clone(),
            stack: config.stack_contents.clone(),
        }
    }

    pub fn resolve(&self) -> Result<StackGraph, Box<dyn Error>> {
        let yaml = self.stack.clone();
        let graph = self.build_graph(yaml)?;

        Ok(graph)
    }

    fn build_graph(
        &self,
        yaml: serde_yaml::Value,
    ) -> Result<StackGraph, Box<dyn std::error::Error>> {
        let meta = serde_yaml::to_string(&yaml["config"]["meta"]).unwrap();
        let name = yaml["name"].as_str().unwrap().to_string();
        let version = yaml["version"].as_str().unwrap().to_string();
        let kind = yaml["kind"].as_str().unwrap().to_string();
        let ingress = yaml["config"]["ingress"].as_bool().unwrap();
        let tf_version = self.get_tf_version();
        let helm_version = self.get_helm_version();
        let git_sha = self.get_commit_sha();
        let mut graph = StackGraph::new(
            StackConfig {
                meta: meta,
                ingress: ingress,
            },
            name,
            kind,
            version,
            tf_version,
            helm_version,
            git_sha
        );

        self.walk_yaml(&mut graph, &yaml);

        Ok(graph)
    }

    fn get_helm_version(&self) -> String {
        let cmd_out = Command::new("helm")
            .arg("version")
            .output()
            .expect("Failed to get helm version, please make sure helm3 is installed and that the helm alias is in your path.");

        String::from_utf8(cmd_out.stdout).unwrap()
    }

    fn get_tf_version(&self) -> String {
        let torb_path = torb_path();
        let cmd_out = Command::new("./terraform")
            .arg("version")
            .arg("-json")
            .current_dir(torb_path)
            .output()
            .expect("Failed to get terraform version, please make sure Torb has been initialized properly.");

        String::from_utf8(cmd_out.stdout).unwrap()
    }

    fn get_commit_sha(&self) -> String {
        let torb_path = torb_path();
        let artifacts_path = torb_path.join("torb-artifacts");
        let cmd_out = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(artifacts_path)
            .output()
            .expect("Failed to get current commit SHA for torb-artifacts, please make sure git is installed and that Torb has been initialized.");

        String::from_utf8(cmd_out.stdout).unwrap()
    }

    fn resolve_service(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        name: &str,
        artifact_path: PathBuf,
    ) -> Result<DependencyNode, Box<dyn Error>> {
        let services_path = artifact_path.join("services");
        let service_path = services_path.join(name);
        let torb_yaml_path = service_path.join("torb.yaml");
        let torb_yaml = std::fs::read_to_string(torb_yaml_path)?;
        let mut node: DependencyNode = serde_yaml::from_str(torb_yaml.as_str())?;
        node.fqn = format!("{}-{}-{}", stack_name, stack_kind_name, name);

        Ok(node)
    }

    fn resolve_project(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        name: &str,
        artifact_path: PathBuf,
    ) -> Result<DependencyNode, Box<dyn Error>> {
        let services_path = artifact_path.join("projects");
        let service_path = services_path.join(name);
        let torb_yaml_path = service_path.join("torb.yaml");
        let torb_yaml = std::fs::read_to_string(torb_yaml_path)?;
        let mut node: DependencyNode = serde_yaml::from_str(torb_yaml.as_str())?;
        node.fqn = format!("{}-{}-{}", stack_name, stack_kind_name, name);

        Ok(node)
    }

    fn resolve_stack(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        name: &str,
        artifact_path: PathBuf,
    ) -> Result<DependencyNode, Box<dyn Error>> {
        let stack_path = artifact_path.join("stacks");
        let stack_yaml_path = stack_path.join(format!("{}.yaml", name));
        let torb_yaml = std::fs::read_to_string(stack_yaml_path)?;

        let graph = self.build_graph(serde_yaml::from_str(torb_yaml.as_str())?)?;
        let mut node = DependencyNode::new(
            graph.name.clone(),
            HashMap::<String, Option<HashMap<String, String>>>::new(),
            None,
            graph.version.clone(),
            "stack".to_string(),
            Some(graph),
            DependencyNodeDependencies::new(),
        );
        node.fqn = format!("{}-{}-{}", stack_name, stack_kind_name, name);

        Ok(node)
    }

    fn resolve_node(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        node_type: &str,
        yaml: serde_yaml::Value,
    ) -> Result<DependencyNode, Box<dyn Error>> {
        let err = TorbResolverErrors::CannotParseStackManifest;
        let home_dir = dirs::home_dir().unwrap();
        let torb_path = home_dir.join(".torb");
        let artifacts_path = torb_path.join("torb-artifacts");
        let mut node = match node_type {
            "service" => {
                let service_name = yaml.get("service").ok_or(err)?.as_str().unwrap();
                self.resolve_service(stack_name, stack_kind_name, service_name, artifacts_path)
            }
            "project" => {
                let project_name = yaml.get("project").ok_or(err)?.as_str().unwrap();
                self.resolve_project(stack_name, stack_kind_name, project_name, artifacts_path)
            }
            "stack" => {
                let local_stack_name = yaml.get("project").ok_or(err)?.as_str().unwrap();
                self.resolve_stack(stack_name, stack_kind_name, local_stack_name, artifacts_path)
            }
            _ => {
                return Err(Box::new(err))
            }
        }?;
        let dep_values = yaml.get("deps");
        match dep_values {
            Some(deps) => {
                let yaml_str = serde_yaml::to_string(deps)?;
                let deps: DependencyNodeDependencies = serde_yaml::from_str(yaml_str.as_str()).unwrap();
                node.dependencies = deps;

                Ok(node)
            }
            None => {
                return Ok(node)
            }
        }
    }

    fn walk_yaml(&self, graph: &mut StackGraph, yaml: &serde_yaml::Value) {
        // walk yaml and add to graph
        for (key, value) in yaml.as_mapping().unwrap().iter() {
            let key_string = key.as_str().unwrap();
            match key_string {
                "services" => {
                    for (service_name, service_value) in value.as_mapping().unwrap().iter() {
                        let stack_service_name = service_name.as_str().unwrap();
                        let stack_name = self.config.stack_name.clone();
                        let service_value = service_value.clone();
                        let service_node = self
                            .resolve_node(stack_name.as_str(), stack_service_name, "service", service_value)
                            .unwrap();
                        graph.add_service(&service_node);
                    }
                }
                "projects" => {
                    for (project_name, project_value) in value.as_mapping().unwrap().iter() {
                        let project_name = project_name.as_str().unwrap();
                        let stack_name = self.config.stack_name.clone();
                        let project_value = project_value.clone();
                        let project_node = self
                            .resolve_node(stack_name.as_str(), project_name, "project", project_value)
                            .unwrap();
                        graph.add_project(&project_node);
                    }
                }
                "stacks" => {
                    for (stack_name, stack_value) in value.as_mapping().unwrap().iter() {
                        let global_stack_name = self.config.stack_name.clone();
                        let local_stack_name = stack_name.as_str().unwrap();
                        let stack_value = stack_value.clone();
                        let stack_node =
                            self.resolve_node(global_stack_name.as_str(), local_stack_name, "stack", stack_value).unwrap();
                        graph.add_stack(&stack_node);
                    }
                }
                _ => {}
            }
        }
    }
}
