use crate::composer::InputAddress;
use crate::resolver::inputs::{InputResolver, NO_INITS_FN};
use crate::resolver::{resolve_stack, NodeDependencies, StackGraph};
use crate::utils::{buildstate_path_or_create, checksum, kebab_to_snake_case};

use data_encoding::BASE32;
use indexmap::{IndexMap, IndexSet};
use memorable_wordlist;
use once_cell::sync::Lazy;
use serde::{de, de::SeqAccess, de::Visitor, Deserialize, Deserializer, Serialize};
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

#[derive(Error, Debug)]
pub enum TorbInputErrors {
    #[error("TorbInput value requested is not a(n) {requested:?} type is a(n) {actual:?}")]
    IncorrectTorbInputType { requested: String, actual: String },
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

fn get_types() -> IndexSet<&'static str> {
    IndexSet::from(["bool", "array", "string", "numeric"])
}

pub static TYPES: Lazy<IndexSet<&str>> = Lazy::new(get_types);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TorbNumeric {
    Int(u64),
    Float(f64),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TorbInput {
    Bool(bool),
    Array(Vec<TorbInput>),
    String(String),
    Numeric(TorbNumeric),
    Address(InputAddress)
}

impl From<bool> for TorbInput {
    fn from(value: bool) -> Self {
        TorbInput::Bool(value)
    }
}

impl From<u64> for TorbInput {
    fn from(value: u64) -> Self {
        let wrapper = TorbNumeric::Int(value);

        TorbInput::Numeric(wrapper)
    }
}

impl From<f64> for TorbInput {
    fn from(value: f64) -> Self {
        let wrapper = TorbNumeric::Float(value);

        TorbInput::Numeric(wrapper)
    }
}

impl From<String> for TorbInput {
    fn from(value: String) -> Self {
        TorbInput::String(value)
    }
}

impl<T> From<Vec<T>> for TorbInput 
where TorbInput: From<T>,
    T: Clone
{
    fn from(value: Vec<T>) -> Self {
        let mut new_vec = Vec::<TorbInput>::new();

        for item in value.iter().cloned() {
            new_vec.push(Into::<TorbInput>::into(item));
        }

        TorbInput::Array(new_vec)
    }
}


impl TorbInput {
    fn input_type(&self) -> String {
        match self {
            TorbInput::Bool(_) => "bool".to_string(),
            TorbInput::Array(_) => "array".to_string(),
            TorbInput::String(_) => "string".to_string(),
            TorbInput::Numeric(_) => "numeric".to_string(),
            TorbInput::Address(_) => "address".to_string()
        }
    }

    pub fn serialize_for_init(&self, expected_type: String) -> String {
        let serde_val = serde_json::json!(self);

        let serialized_type = match serde_val {
            serde_json::Value::Array(_) => {
                "array"
            },
            serde_json::Value::Bool(_) => {
                "bool"
            },
            serde_json::Value::Number(_) => {
                "numeric"
            },
            serde_json::Value::String(_) => {
                "string"
            },
            serde_json::Value::Null => {
                "invalid"
            },
            serde_json::Value::Object(_) => {
                "invalid"
            }
        };

        if expected_type != serialized_type {
            panic!("Serialzed type doesn't match expected type.")
        }

        serde_json::to_string(&serde_val).expect("Unable to serialize TorbInput to JSON, this is a bug and should be reported to the project maintainer(s).")
    }

    fn bool_value(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if self.input_type() == "bool" {
            let TorbInput::Bool(val) = self;

            Ok(val.clone())
        } else {
            Err(Box::new(TorbInputErrors::IncorrectTorbInputType { requested: "bool".to_string(), actual: self.input_type() }))
        }
    }

    fn array_value(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if self.input_type() == "array" {
            let TorbInput::Bool(val) = self;

            Ok(val.clone())
        } else {
            Err(Box::new(TorbInputErrors::IncorrectTorbInputType { requested: "array".to_string(), actual: self.input_type() }))
        }
    }

    fn string_value(&self) -> Result<String, Box<dyn std::error::Error>> {
        if self.input_type() == "string" {
            let TorbInput::String(val) = self;

            Ok(val.clone())
        } else {
            Err(Box::new(TorbInputErrors::IncorrectTorbInputType { requested: "string".to_string(), actual: self.input_type() }))
        }
    }

    fn numeric_value(&self) -> Result<TorbNumeric, Box<dyn std::error::Error>> {
        if self.input_type() == "string" {
            let TorbInput::Numeric(val) = self;

            Ok(val.clone())
        } else {
            Err(Box::new(TorbInputErrors::IncorrectTorbInputType { requested: "numeric".to_string(), actual: self.input_type() }))
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct TorbInputSpec {
    typing: String,
    default: TorbInput,
    mapping: String,
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
    pub mapped_inputs: IndexMap<String, (String, TorbInput)>,
    #[serde(alias = "inputs", default = "IndexMap::new")]
    pub input_spec: IndexMap<String, TorbInputSpec>,
    #[serde(default = "Vec::new")]
    pub outputs: Vec<String>,
    #[serde(default = "Vec::new")]
    pub dependencies: Vec<ArtifactNodeRepr>,
    #[serde(default = "IndexSet::new")]
    pub implicit_dependency_fqns: IndexSet<String>,
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
    pub source: Option<String>,
}

struct TorbInputSpecDeserializer;
impl<'de> Visitor<'de> for TorbInputSpecDeserializer {
    type Value = TorbInputSpec;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a list.")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let default = TorbInput::String(String::new());
        let mapping = v.to_string();
        let typing = "string".to_string();

        Ok(TorbInputSpec {
            typing,
            default,
            mapping,
        })
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        println!("HERE SEQ");
        let mut count = 0;
        let mut typing = String::new();
        let mut mapping = String::new();
        let mut default = TorbInput::String(String::new());

        if seq.size_hint().is_some() && seq.size_hint() != Some(3) {
            return Err(de::Error::custom(format!(
                "Didn't find the right sequence of values to create a TorbInputSpec."
            )));
        }

        while count < 3 {
            match count {
                0 => {
                    let value_opt = seq.next_element::<String>()?;

                    let value = if !value_opt.is_some() {
                        return Err(de::Error::custom(format!(
                            "Didn't find the right sequence of values to create a TorbInputSpec."
                        )));
                    } else {
                        value_opt.unwrap()
                    };

                    if !TYPES.contains(value.as_str()) {
                        return Err(de::Error::custom(format!(
                            "Please set a valid type for your input spec. Valid types are {:#?}. \n If you see this as a regular user, a unit author has included a broken spec.",
                            TYPES
                        )));
                    }

                    typing = value;
                    count += 1;
                }
                1 => {
                    match typing.as_str() {
                        "bool" => {
                            let value_opt = seq.next_element::<bool>()?;

                            let value = if !value_opt.is_some() {
                                return Err(de::Error::custom(format!(
                                    "Didn't find the right sequence of values to create a TorbInputSpec."
                                )));
                            } else {
                                value_opt.unwrap()
                            };

                            default = TorbInput::Bool(value);
                        }
                        "string" => {
                            let value_opt = seq.next_element::<String>()?;

                            let value = if !value_opt.is_some() {
                                return Err(de::Error::custom(format!(
                                    "Didn't find the right sequence of values to create a TorbInputSpec."
                                )));
                            } else {
                                value_opt.unwrap()
                            };

                            default = TorbInput::String(value);
                        }
                        "array" => {
                            default = TorbInput::String(String::new());
                        }
                        "numeric" => {
                            default = TorbInput::String(String::new());
                        }
                        _ => {
                            default = TorbInput::String(String::new());
                        }
                    }
                    count += 1;
                }
                2 => {
                    let value_opt = seq.next_element::<String>()?;

                    let value = if !value_opt.is_some() {
                        return Err(de::Error::custom(format!(
                            "Didn't find the right sequence of values to create a TorbInputSpec."
                        )));
                    } else {
                        value_opt.unwrap()
                    };

                    mapping = value;
                    count += 1;
                }
                _ => {
                    return Err(de::Error::custom(format!(
                        "Didn't find the right sequence of values to create a TorbInputSpec."
                    )));
                }
            }
        }

        let new_obj = TorbInputSpec {
            typing,
            mapping,
            default,
        };

        Ok(new_obj)
    }
}

impl<'de> Deserialize<'de> for TorbInputSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TorbInputSpecDeserializer)
    }
}

impl ArtifactNodeRepr {
    pub fn display_name(&self) -> String {
        kebab_to_snake_case(&self.name)
    }

    #[allow(dead_code)]
    pub fn new(
        fqn: String,
        name: String,
        version: String,
        kind: String,
        lang: Option<String>,
        init_step: Option<Vec<String>>,
        build_step: Option<BuildStep>,
        deploy_steps: IndexMap<String, Option<IndexMap<String, String>>>,
        inputs: IndexMap<String, (String, TorbInput)>,
        input_spec: IndexMap<String, TorbInputSpec>,
        outputs: Vec<String>,
        file_path: String,
        stack_graph: Option<StackGraph>,
        files: Option<Vec<String>>,
        values: String,
        namespace: Option<String>,
        source: Option<String>,
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
            implicit_dependency_fqns: IndexSet::new(),
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
            namespace,
            source,
        }
    }

    fn address_to_fqn(
        graph_name: &String,
        addr_result: Result<InputAddress, String>,
    ) -> Option<String> {
        match addr_result {
            Ok(addr) => {
                let fqn = format!(
                    "{}.{}.{}",
                    graph_name,
                    addr.node_type.clone(),
                    addr.node_name.clone()
                );

                Some(fqn)
            }
            Err(_s) => None,
        }
    }

    pub fn discover_and_set_implicit_dependencies(
        &mut self,
        graph_name: &String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut implicit_deps_inputs = IndexSet::new();

        let inputs_fn = |_spec: &String, val: Result<InputAddress, String>| -> String {
            let fqn_option = ArtifactNodeRepr::address_to_fqn(graph_name, val);

            if fqn_option.is_some() {
                let fqn = fqn_option.unwrap();
                implicit_deps_inputs.insert(fqn);
            };

            "".to_string()
        };

        let mut implicit_deps_values = IndexSet::new();

        let values_fn = |addr: Result<InputAddress, String>| -> String {
            let fqn_option = ArtifactNodeRepr::address_to_fqn(graph_name, addr);

            if fqn_option.is_some() {
                let fqn = fqn_option.unwrap();
                implicit_deps_values.insert(fqn);
            };

            "".to_string()
        };

        let (_, _, _) = InputResolver::resolve(&self, Some(values_fn), Some(inputs_fn), NO_INITS_FN)?;

        let unioned_deps = implicit_deps_inputs.union(&mut implicit_deps_values);

        self.implicit_dependency_fqns = unioned_deps.cloned().collect();

        Ok(())
    }

    pub fn validate_map_and_set_inputs(&mut self, inputs: IndexMap<String, TorbInput>) {
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

            self.mapped_inputs = IndexMap::<String, (String, TorbInput)>::new();
        }
    }

    fn validate_inputs(
        inputs: &IndexMap<String, TorbInput>,
        spec: &IndexMap<String, TorbInputSpec>,
    ) -> Result<(), String> {
        for (key, val) in inputs.iter() {
            if !spec.contains_key(key) {
                return Err(key.clone());
            }

            let input_spec = spec.get(key).unwrap();

            let val_type = match val {
                TorbInput::String(val) => {
                    match InputAddress::try_from(val.as_str()) {
                        Ok(_) => {
                            "input_address"
                        }
                        _ => {
                            "string"
                        }
                    }
                },
                TorbInput::Bool(val) => {
                    "bool" 
                },
                TorbInput::Numeric(val) => {
                    "numeric"
                },
                TorbInput::Array(val) => {
                    "array"
                },
                TorbInput::Address(val) => {
                    "address"
                }
            };

            if val_type != "address" && input_spec.typing != val_type {
                return Err(format!("{key} is type {val_type} but is supposed to be {}", input_spec.typing))
            }
        }

        Ok(())
    }

    fn map_inputs(
        inputs: &IndexMap<String, TorbInput>,
        spec: &IndexMap<String, TorbInputSpec>,
    ) -> IndexMap<String, (String, TorbInput)> {
        let mut mapped_inputs = IndexMap::<String, (String, TorbInput)>::new();

        for (key, value) in spec.iter() {
            let input = inputs.get(key).unwrap_or(&value.default);
            mapped_inputs.insert(
                key.to_string(),
                (value.mapping.clone(), input.clone()),
            );
        }

        mapped_inputs
    }
}

#[derive(Serialize, Deserialize)]
pub struct ArtifactRepr {
    pub torb_version: String,
    pub helm_version: String,
    pub terraform_version: String,
    pub commits: IndexMap<String, String>,
    pub stack_name: String,
    pub meta: Box<Option<ArtifactRepr>>,
    pub deploys: Vec<ArtifactNodeRepr>,
    pub nodes: IndexMap<String, ArtifactNodeRepr>,
    pub namespace: Option<String>,
    pub release: Option<String>,
    pub repositories: Option<Vec<String>>,
}

impl ArtifactRepr {
    fn new(
        torb_version: String,
        helm_version: String,
        terraform_version: String,
        commits: IndexMap<String, String>,
        stack_name: String,
        meta: Box<Option<ArtifactRepr>>,
        namespace: Option<String>,
        release: Option<String>,
        repositories: Option<Vec<String>>,
    ) -> ArtifactRepr {
        ArtifactRepr {
            torb_version,
            helm_version,
            terraform_version,
            commits,
            stack_name,
            meta,
            deploys: Vec::new(),
            nodes: IndexMap::new(),
            namespace: namespace,
            release: release,
            repositories,
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
        graph.commits.clone(),
        graph.name.clone(),
        meta,
        graph.namespace.clone(),
        graph.release.clone(),
        graph.repositories.clone(),
    );

    let mut node_map: IndexMap<String, ArtifactNodeRepr> = IndexMap::new();

    for node in start_nodes {
        let artifact_node_repr = walk_nodes(node, graph, &mut node_map);
        artifact.deploys.push(artifact_node_repr);
    }

    artifact.nodes = node_map;

    Ok(artifact)
}

pub fn stack_into_artifact(
    meta: &Box<Option<ArtifactNodeRepr>>,
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

fn walk_nodes(
    node: &ArtifactNodeRepr,
    graph: &StackGraph,
    node_map: &mut IndexMap<String, ArtifactNodeRepr>,
) -> ArtifactNodeRepr {
    let mut new_node = node.clone();

    for fqn in new_node.implicit_dependency_fqns.iter() {
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

    new_node
        .dependency_names
        .projects
        .as_ref()
        .map_or((), |projects| {
            for project in projects {
                let p_fqn = format!("{}.project.{}", graph.name.clone(), project.clone());

                if !new_node.implicit_dependency_fqns.contains(&p_fqn) {
                    let p_node = graph.projects.get(&p_fqn).unwrap();
                    let p_node_repr = walk_nodes(p_node, graph, node_map);

                    new_node.dependencies.push(p_node_repr);
                }
            }
        });

    new_node
        .dependency_names
        .services
        .as_ref()
        .map_or((), |services| {
            for service in services {
                let s_fqn = format!("{}.service.{}", graph.name.clone(), service.clone());

                if !new_node.implicit_dependency_fqns.contains(&s_fqn) {
                    let s_node = graph.services.get(&s_fqn).unwrap();
                    let s_node_repr = walk_nodes(s_node, graph, node_map);

                    new_node.dependencies.push(s_node_repr);
                }
            }
        });

    node_map.insert(node.fqn.clone(), new_node.clone());

    return new_node;
}

pub fn load_build_file(
    filename: String,
) -> Result<(String, String, ArtifactRepr), Box<dyn std::error::Error>> {
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

pub fn deserialize_stack_yaml_into_artifact(
    stack_yaml: &String,
) -> Result<ArtifactRepr, Box<dyn std::error::Error>> {
    let graph: StackGraph = resolve_stack(stack_yaml)?;
    let artifact = walk_graph(&graph)?;
    Ok(artifact)
}

pub fn get_build_file_info(
    artifact: &ArtifactRepr,
) -> Result<(String, String, String), Box<dyn std::error::Error>> {
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
