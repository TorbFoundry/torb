// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.6-03.19
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

pub mod inputs;

use crate::artifacts::{ArtifactNodeRepr, BuildStep, TorbInput};
use crate::utils::{for_each_artifact_repository, normalize_name, torb_path};
use crate::watcher::{WatcherConfig};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::{self, Value};
use std::collections::HashMap;
use std::process::Command;
use std::{error::Error, path::PathBuf};
use thiserror::Error;

// const VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub fn resolve_stack(stack_yaml: &String) -> Result<StackGraph, Box<dyn std::error::Error>> {
    let stack_def_yaml: serde_yaml::Value = serde_yaml::from_str(stack_yaml).unwrap();
    let stack_name = stack_def_yaml.get("name").unwrap().as_str().unwrap();
    // let stack_description = stack_def_yaml.get("description").unwrap().as_str().unwrap();
    let resolver_conf = ResolverConfig::new(
        // false,
        normalize_name(stack_name),
        // stack_description.to_string(),
        stack_def_yaml.clone(),
        // VERSION.to_string(),
    );

    let resolver = Resolver::new(&resolver_conf);

    resolver.resolve()
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
    // autoaccept: bool,
    stack_name: String,
    // stack_description: String,
    stack_contents: serde_yaml::Value,
    // torb_version: String,
}

impl ResolverConfig {
    pub fn new(
        // autoaccept: bool,
        stack_name: String,
        // stack_description: String,
        stack_contents: serde_yaml::Value,
        // torb_version: String,
    ) -> ResolverConfig {
        ResolverConfig {
            // autoaccept,
            stack_name,
            // stack_description,
            stack_contents,
            // torb_version,
        }
    }
}

// #[derive(Serialize, Deserialize, Clone)]
// pub struct DeployStep {
//     name: String,
//     tool_version: String,
//     tool_name: String,
//     tool_config: IndexMap<String, String>,
// }

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct NodeDependencies {
    pub services: Option<Vec<String>>,
    pub projects: Option<Vec<String>>,
    pub stacks: Option<Vec<String>>,
}

impl NodeDependencies {}

#[derive(Clone, Debug)]
pub struct StackGraph {
    pub services: HashMap<String, ArtifactNodeRepr>,
    pub projects: HashMap<String, ArtifactNodeRepr>,
    pub stacks: HashMap<String, ArtifactNodeRepr>,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub commits: IndexMap<String, String>,
    pub tf_version: String,
    pub helm_version: String,
    pub meta: Box<Option<ArtifactNodeRepr>>,
    pub incoming_edges: HashMap<String, Vec<String>>,
    pub namespace: Option<String>,
    pub release: Option<String>,
    pub repositories: Option<Vec<String>>,
    pub watcher: WatcherConfig
}

impl StackGraph {
    pub fn new(
        name: String,
        kind: String,
        version: String,
        commits: IndexMap<String, String>,
        tf_version: String,
        helm_version: String,
        meta: Box<Option<ArtifactNodeRepr>>,
        namespace: Option<String>,
        release: Option<String>,
        repositories: Option<Vec<String>>,
        watcher: WatcherConfig
    ) -> StackGraph {
        StackGraph {
            services: HashMap::<String, ArtifactNodeRepr>::new(),
            projects: HashMap::<String, ArtifactNodeRepr>::new(),
            stacks: HashMap::<String, ArtifactNodeRepr>::new(),
            name,
            version,
            kind,
            tf_version,
            helm_version,
            commits,
            meta,
            incoming_edges: HashMap::<String, Vec<String>>::new(),
            namespace,
            release,
            repositories,
            watcher: watcher
        }
    }

    pub fn add_service(&mut self, node: &ArtifactNodeRepr) {
        self.services.insert(node.fqn.clone(), node.clone());
    }
    pub fn add_project(&mut self, node: &ArtifactNodeRepr) {
        self.projects.insert(node.fqn.clone(), node.clone());
    }
    // pub fn add_stack(&mut self, node: &ArtifactNodeRepr) {
    //     self.stacks.insert(node.fqn.clone(), node.clone());
    // }
    pub fn add_all_incoming_edges_downstream(
        &mut self,
        stack_name: String,
        node: &ArtifactNodeRepr,
    ) {
        self.incoming_edges
            .entry(node.fqn.clone())
            .or_insert(Vec::<String>::new());

        node.dependency_names
            .projects
            .as_ref()
            .map_or((), |projects| {
                projects.iter().for_each(|project| {
                    let p_fqn = format!("{}.{}.{}", stack_name, "project".to_string(), project);
                    match self.incoming_edges.get_mut(p_fqn.as_str()) {
                        Some(edges) => {
                            edges.push(node.fqn.clone());
                        }
                        None => {
                            let mut edges = Vec::new();
                            edges.push(node.fqn.clone());
                            self.incoming_edges.insert(p_fqn.clone(), edges);
                        }
                    }
                });
            });

        node.dependency_names
            .services
            .as_ref()
            .map_or((), |projects| {
                projects.iter().for_each(|project| {
                    let s_fqn = format!("{}.{}.{}", stack_name, "service".to_string(), project);
                    match self.incoming_edges.get_mut(project) {
                        Some(edges) => {
                            edges.push(node.fqn.clone());
                        }
                        None => {
                            let mut edges = Vec::new();
                            edges.push(node.fqn.clone());
                            self.incoming_edges.insert(s_fqn.clone(), edges);
                        }
                    }
                });
            });

        node.dependency_names
            .stacks
            .as_ref()
            .map_or((), |projects| {
                projects.iter().for_each(|project| {
                    let s_fqn = format!("{}.{}.{}", stack_name, "stack".to_string(), project);
                    match self.incoming_edges.get_mut(project) {
                        Some(edges) => {
                            edges.push(node.fqn.clone());
                        }
                        None => {
                            let mut edges = Vec::new();
                            edges.push(node.fqn.clone());
                            self.incoming_edges.insert(s_fqn.clone(), edges);
                        }
                    }
                });
            });
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
        println!("Resolving stack graph...");
        let yaml = self.stack.clone();
        let graph = self.build_graph(yaml)?;

        Ok(graph)
    }

    fn build_graph(
        &self,
        yaml: serde_yaml::Value,
    ) -> Result<StackGraph, Box<dyn std::error::Error>> {
        let meta = Box::new(None);
        let mut name = yaml["name"].as_str().unwrap().to_string();
        name = normalize_name(&name);

        let version = yaml["version"].as_str().unwrap().to_string();
        let kind = yaml["kind"].as_str().unwrap().to_string();
        let tf_version = self.get_tf_version();
        let helm_version = self.get_helm_version();
        let mut commits = IndexMap::new();

        for_each_artifact_repository(Box::new(|_repo_path, repo| {
            let repo_string = &repo.file_name().into_string().unwrap();
            let sha = self.get_commit_sha(repo_string);

            commits.insert(repo_string.clone(), sha);
        }))?;

        let namespace = yaml["namespace"].as_str().map(|ns| ns.to_string());
        let release = yaml["release"].as_str().map(|ns| ns.to_string());
        let repositories: Option<Vec<String>> =
            serde_yaml::from_value(yaml["repositories"].clone())?;


        let watcher: WatcherConfig = match yaml["watcher"] {
            Value::Null => WatcherConfig::default(),
            _ => serde_yaml::from_value(yaml["watcher"].clone())?
        };

        let mut graph = StackGraph::new(
            name,
            kind,
            version,
            commits,
            tf_version,
            helm_version,
            meta,
            namespace,
            release,
            repositories,
            watcher
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

    fn get_commit_sha(&self, repo: &String) -> String {
        let torb_path = torb_path();
        let artifacts_path = torb_path.join("repositories").join(repo);
        let cmd_out = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(artifacts_path)
            .output()
            .expect("Failed to get current commit SHA for an artifact repo, please make sure git is installed and that Torb has been initialized.");

        let mut sha = String::from_utf8(cmd_out.stdout).unwrap();

        // Removes newline
        sha.pop();

        sha
    }

    fn resolve_service(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        node_name: &str,
        service_name: &str,
        artifact_path: PathBuf,
        inputs: IndexMap<String, TorbInput>,
        values: serde_yaml::Value,
        source: &str,
        namespace: Option<String>
    ) -> Result<ArtifactNodeRepr, Box<dyn Error>> {
        let services_path = artifact_path.join("services");
        let service_path = services_path.join(service_name);
        let torb_yaml_path = service_path.join("torb.yaml");
        let torb_yaml = std::fs::read_to_string(&torb_yaml_path)?;
        let mut node: ArtifactNodeRepr = serde_yaml::from_str(torb_yaml.as_str())?;
        node.fqn = format!("{}.{}.{}", stack_name, stack_kind_name, node_name);
        let node_fp = torb_yaml_path
            .to_str()
            .ok_or("Could not convert path to string.")?
            .to_string();
        node.file_path = node_fp;

        node.source = Some(source.to_string());
        node.namespace = namespace;

        node.values =
            serde_yaml::to_string(&values).expect("Unable to convert values yaml to string.");
        node.validate_map_and_set_inputs(inputs);
        node.discover_and_set_implicit_dependencies(&stack_name.to_string())?;

        Ok(node)
    }

    fn reconcile_build_step(&self, build_step: BuildStep, new_build_step: BuildStep) -> BuildStep {
        let registry = if new_build_step.registry != "" {
            new_build_step.registry
        } else {
            build_step.registry
        };

        let dockerfile = if new_build_step.dockerfile != "" {
            new_build_step.dockerfile
        } else {
            build_step.dockerfile
        };

        let script_path = if new_build_step.script_path != "" {
            new_build_step.script_path
        } else {
            build_step.script_path
        };

        let tag = if new_build_step.tag != "" {
            new_build_step.tag
        } else {
            build_step.tag
        };

        BuildStep {
            registry,
            tag,
            dockerfile,
            script_path,
        }
    }

    fn resolve_project(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        node_name: &str,
        project_name: &str,
        artifact_path: PathBuf,
        inputs: IndexMap<String, TorbInput>,
        build_config: Option<&Value>,
        values: serde_yaml::Value,
        source: &str,
        namespace: Option<String>
    ) -> Result<ArtifactNodeRepr, Box<dyn Error>> {
        let projects_path = artifact_path.join("projects");
        let project_path = projects_path.join(project_name);
        let torb_yaml_path = project_path.join("torb.yaml");
        let torb_yaml = std::fs::read_to_string(&torb_yaml_path)?;
        let mut node: ArtifactNodeRepr = serde_yaml::from_str(torb_yaml.as_str())?;
        let node_fp = torb_yaml_path
            .to_str()
            .ok_or("Could not convert path to string.")?
            .to_string();

        node.source = Some(source.to_string());
        node.namespace = namespace;

        let build_step = node.build_step.or(Some(BuildStep::default())).unwrap();
        let new_build_step: BuildStep = match build_config {
            Some(build) => {
                let temp = serde_yaml::from_value(build.clone())?;
                self.reconcile_build_step(build_step, temp)
            }
            None => {
                let temp = BuildStep {
                    registry: "".to_string(),
                    dockerfile: "".to_string(),
                    script_path: "".to_string(),
                    tag: "".to_string(),
                };

                self.reconcile_build_step(build_step, temp)
            }
        };

        node.build_step = Some(new_build_step);
        node.fqn = format!("{}.{}.{}", stack_name, stack_kind_name, node_name);
        node.file_path = node_fp;
        node.validate_map_and_set_inputs(inputs);
        node.values =
            serde_yaml::to_string(&values).expect("Unable to convert values yaml to string.");
        node.discover_and_set_implicit_dependencies(&stack_name.to_string())?;

        Ok(node)
    }

    fn deserialize_params(
        params: Option<&serde_yaml::Value>,
    ) -> Result<IndexMap<String, TorbInput>, Box<dyn Error>> {
        match params {
            Some(params) => {
                let deserialized_params: IndexMap<String, TorbInput> =
                    serde_yaml::from_value(params.clone())?;

                Ok(deserialized_params)
            }
            None => Ok(IndexMap::new()),
        }
    }

    fn resolve_node(
        &self,
        stack_name: &str,
        stack_kind_name: &str,
        node_name: &str,
        yaml: serde_yaml::Value,
    ) -> Result<ArtifactNodeRepr, Box<dyn Error>> {
        println!("Resolving node: {}", node_name);
        let err = TorbResolverErrors::CannotParseStackManifest;
        let home_dir = dirs::home_dir().unwrap();
        let torb_path = home_dir.join(".torb");
        let repository_path = torb_path.join("repositories");

        let repo = match yaml.get("source") {
            Some(source) => source.as_str().unwrap(),
            None => "torb-artifacts",
        };

        let artifacts_path = repository_path.join(repo);

        let inputs = Resolver::deserialize_params(yaml.get("inputs"))
            .expect("Unable to deserialize inputs.");

        let config_values = yaml.get("values").unwrap_or(&serde_yaml::Value::Null);

        let mut node = match stack_kind_name {
            "service" => {
                let service_name = yaml
                    .get("service")
                    .ok_or(err)?
                    .as_str()
                    .expect("Unable to parse service name.");

                let service_namespace = yaml.get("namespace").map(|x| {
                    x.as_str().unwrap().to_string()
                });

                self.resolve_service(
                    stack_name,
                    stack_kind_name,
                    node_name,
                    service_name,
                    artifacts_path,
                    inputs,
                    config_values.clone(),
                    repo,
                    service_namespace
                )
            }
            "project" => {
                let project_name = yaml
                    .get("project")
                    .ok_or(err)?
                    .as_str()
                    .expect("Unable to parse project name.");
                let build_config = yaml.get("build");

                let project_namespace = yaml.get("namespace").map(|x| {
                    x.as_str().unwrap().to_string()
                });

                self.resolve_project(
                    stack_name,
                    stack_kind_name,
                    node_name,
                    project_name,
                    artifacts_path,
                    inputs,
                    build_config,
                    config_values.clone(),
                    repo,
                    project_namespace
                )
            }

            _ => return Err(Box::new(err)),
        }?;

        let dep_values = yaml.get("deps");
        match dep_values {
            Some(deps) => {
                let yaml_str = serde_yaml::to_string(deps)?;
                let deps: NodeDependencies = serde_yaml::from_str(yaml_str.as_str()).unwrap();
                node.dependency_names = deps;

                Ok(node)
            }
            None => return Ok(node),
        }
    }

    fn walk_yaml(&self, graph: &mut StackGraph, yaml: &serde_yaml::Value) {
        // Walk yaml and add nodes to graph
        for (key, value) in yaml.as_mapping().unwrap().iter() {
            let key_string = key.as_str().unwrap();
            match key_string {
                "services" => {
                    value.as_mapping().and_then(|mapping| {
                        for (service_name, service_value) in mapping.iter() {
                            let stack_service_name = service_name.as_str().unwrap();
                            let stack_name = self.config.stack_name.clone();
                            let service_value = service_value.clone();
                            let service_node = self
                                .resolve_node(
                                    stack_name.as_str(),
                                    "service",
                                    stack_service_name,
                                    service_value,
                                )
                                .unwrap();

                            graph.add_service(&service_node);
                            graph.add_all_incoming_edges_downstream(
                                stack_name.clone(),
                                &service_node,
                            );
                        }

                        Some(())
                    });
                }
                "projects" => {
                    value.as_mapping().and_then(|mapping| {
                        for (project_name, project_value) in mapping.iter() {
                            let project_name = project_name.as_str().unwrap();
                            let stack_name = self.config.stack_name.clone();
                            let project_value = project_value.clone();
                            let project_node = self
                                .resolve_node(
                                    stack_name.as_str(),
                                    "project",
                                    project_name,
                                    project_value,
                                )
                                .expect("Failed to resolve project node.");
                            graph.add_project(&project_node);
                            graph.add_all_incoming_edges_downstream(
                                stack_name.clone(),
                                &project_node,
                            );
                        }

                        Some(())
                    });
                }
                _ => (),
            }
        }
    }
}
