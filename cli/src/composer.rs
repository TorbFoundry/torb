use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr, TorbInput, TorbNumeric};
use crate::resolver::inputs::{InputResolver, NO_INPUTS_FN, NO_VALUES_FN, NO_INITS_FN};
use crate::utils::{buildstate_path_or_create, for_each_artifact_repository, torb_path, kebab_to_snake_case, snake_case_to_kebab};

use hcl::{Block, Body, Expression, Object, ObjectKey, RawExpression, Number};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use thiserror::Error;
use indexmap::IndexSet;

#[derive(Error, Debug)]
pub enum TorbComposerErrors {}

fn reserved_outputs() -> HashMap<&'static str, &'static str> {
    let reserved = vec![("host", "")];

    let mut reserved_hash = HashMap::new();

    for (k, v) in reserved {
        reserved_hash.insert(k, v);
    }

    reserved_hash
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAddress {
    pub locality: String,
    pub node_type: String,
    pub node_name: String,
    pub node_property: String,
    pub property_specifier: String,
}

impl<'a> InputAddress {
    fn new(
        locality: String,
        node_type: String,
        node_name: String,
        node_property: String,
        property_specifier: String,
    ) -> InputAddress {
        InputAddress {
            locality,
            node_type,
            node_name,
            node_property,
            property_specifier,
        }
    }

    fn is_init_address(vals: &Vec<&str>) -> Option<InputAddress> {
        if vals.len() == 3 && vals[0] == "TORB" {
            let locality = vals[0].to_string();
            let node_type = "".to_string();
            let node_name = "".to_string();
            let node_property = vals[1].to_string();
            let property_specifier = vals[2].to_string();

            return Some(InputAddress::new(
                locality,
                node_type,
                node_name,
                node_property,
                property_specifier
            ))
        }

        None
    }

    fn is_input_address(vals: &Vec<&str>) -> Option<InputAddress> {
        if vals.len() == 5 && vals[0] == "self" {
            let locality = vals[0].to_string();
            let node_type = vals[1].to_string();
            let node_name = vals[2].to_string();
            let node_property = vals[3].to_string();
            let property_specifier = vals[4].to_string();

            return Some(InputAddress::new(
                locality,
                node_type,
                node_name,
                node_property,
                property_specifier,
            ))
        } 

        None
    }

    fn supported_localities() -> HashSet<&'a str> {
        let set = vec!["self", "TORB"];

        set.into_iter().collect::<HashSet<&'a str>>()
    }

}

impl TryFrom<&str> for InputAddress {
    type Error = TorbInput;

    fn try_from(input: &str) -> Result<Self, TorbInput> {
        let vals = input.split(".").collect::<Vec<&str>>();

        if !InputAddress::supported_localities().contains(vals[0]) {
            return Err(TorbInput::String(input.to_string()))
        }

        let init_addr_opt = InputAddress::is_init_address(&vals);

        if init_addr_opt.is_some() {
            return Ok(init_addr_opt.unwrap())
        }

        let input_addr_opt = InputAddress::is_input_address(&vals);

        if input_addr_opt.is_some() {
            return Ok(input_addr_opt.unwrap())
        }

        Err(TorbInput::String(input.to_string()))
    }
}

impl TryFrom<&TorbInput> for InputAddress {
    type Error = TorbInput;

    fn try_from(input: &TorbInput) -> Result<Self, TorbInput> {
        if let TorbInput::String(str_input) = input {
            let vals = str_input.split(".").collect::<Vec<&str>>();

            if !InputAddress::supported_localities().contains(vals[0]) {
                return Err(TorbInput::String(str_input.to_string()))
            }

            let init_addr_opt = InputAddress::is_init_address(&vals);

            if init_addr_opt.is_some() {
                return Ok(init_addr_opt.unwrap())
            }

            let input_addr_opt = InputAddress::is_input_address(&vals);

            if input_addr_opt.is_some() {
                return Ok(input_addr_opt.unwrap())
            }

            Err(TorbInput::String(str_input.to_string()))
        } else {
            Err(input.clone())
        }
    }
}

pub struct Composer<'a> {
    hash: String,
    build_files_seen: IndexSet<String>,
    fqn_seen: IndexSet<String>,
    release_name: String,
    main_struct: hcl::BodyBuilder,
    artifact_repr: &'a ArtifactRepr,
}

impl<'a> Composer<'a> {
    pub fn new(hash: String, artifact_repr: &ArtifactRepr) -> Composer {
        Composer {
            hash: hash,
            build_files_seen: IndexSet::new(),
            fqn_seen: IndexSet::new(),
            release_name: artifact_repr.release(),
            main_struct: Body::builder(),
            artifact_repr: artifact_repr,
        }
    }

    fn get_node_for_output_value(&self, torb_input_address: &InputAddress) -> &ArtifactNodeRepr {
        let stack_name = &self.artifact_repr.stack_name;
        let output_node_fqn = format!(
            "{}.{}.{}",
            stack_name, &torb_input_address.node_type, &torb_input_address.node_name
        );

        self.artifact_repr
            .nodes
            .get(&output_node_fqn)
            .expect("Unable to map input address to node, make sure your mapping is correct.")
    }

    fn interpolate_inputs_into_helm_values(
        &self,
        torb_input_address: Result<InputAddress, TorbInput>,
    ) -> String {
        let output_value = self.input_values_from_input_address(torb_input_address.clone());
        let string_value = hcl::format::to_string(&output_value).unwrap();
        match torb_input_address {
            Ok(input_address) => {

                if reserved_outputs().contains_key(input_address.property_specifier.as_str()) {
                    string_value.replace("\"", "")
                } else {
                    format!("${{{}}}", string_value.replace("\"", ""))
                }
            }
            Err(_s) => string_value,
        }
    }

    fn k8s_value_from_reserved_input(&self, torb_input_address: InputAddress) -> Expression {
        let output_node = self.get_node_for_output_value(&torb_input_address);

        match torb_input_address.property_specifier.as_str() {
            "host" => {
                let name = format!("{}-{}", self.release_name, output_node.display_name());

                let namespace = self.artifact_repr.namespace(output_node);

                Expression::String(format!("{}.{}.svc.cluster.local", name, namespace))
            }
            _ => {
                panic!("Unable to map reserved value.")
            }
        }
    }

    fn k8s_status_values_path_from_torb_input(&self, torb_input_address: InputAddress) -> String {
        let output_node = self.get_node_for_output_value(&torb_input_address);

        let kube_value = if torb_input_address.node_property == "output" || torb_input_address.node_property == "inputs" {
            let (kube_val, _) = output_node
                .mapped_inputs
                .get(&torb_input_address.property_specifier)
                .expect("Unable to map input from output node. Key does not exist.");

            kube_val
        } else {
            panic!("Unable to map node property to output attribute please check your inputs, ex: 'a.b.output.c or a.b.input.c");
        };

        let formatted_name = kebab_to_snake_case(&self.release_name);
        let block_name = format!("{}_{}", formatted_name, &output_node.display_name());

        format!(
            "jsondecode(data.torb_helm_release.{}.values)[\"{}\"]",
            block_name, kube_value
        )
    }

    pub fn compose(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Composing build environment...");
        let buildstate_path = buildstate_path_or_create();
        let environment_path = buildstate_path.join("iac_environment");

        if !environment_path.exists() {
            std::fs::create_dir(environment_path)?;
        }

        self.add_required_providers_to_main_struct();

        for node in self.artifact_repr.deploys.iter() {
            self.walk_artifact(node)?;
        }

        self.copy_supporting_build_files()
            .expect("Failed to write supporting buildfiles to new environment.");

        self.write_main_buildfile()
            .expect("Failed to write main buildfile to new environment.");

        Ok(())
    }

    fn copy_supporting_build_files(&self) -> Result<(), Box<dyn std::error::Error>> {
        for_each_artifact_repository(Box::new(|repos_path, repo| {
            let repo_path = repos_path.join(repo.file_name());
            let source_path = repo_path.join("common");
            let buildstate_path = buildstate_path_or_create();

            let new_environment_path = buildstate_path.join("iac_environment");

            let repo_name = repo.file_name().into_string().unwrap();
            let namespace_dir = kebab_to_snake_case(&repo_name);
            let dest = new_environment_path
                .join(namespace_dir)
                .join(source_path.as_path().file_name().unwrap());

            if !dest.exists() {
                fs::create_dir_all(dest.clone()).expect("Unable to create supporting buildfile directory at destination, please check torb has been initialized properly.");
            }

            self._copy_files_recursively(source_path, dest);

            let provider_path = repo_path.join("common/providers");
            let dest = new_environment_path.clone();

            self._copy_files_recursively(provider_path, dest);
        }))?;

        Ok(())
    }

    fn _copy_files_recursively(&self, path: std::path::PathBuf, dest: std::path::PathBuf) -> () {
        let error_string = format!("Failed reading dir: {}. Please check that torb is correctly initialized and that any additional artifact repos have been pulled with `torb artifacts refresh`.", path.to_str().unwrap());
        for entry in path.read_dir().expect(&error_string) {
            let error_string = format!("Failed reading entry in dir: {}. Please check that torb is correctly initialized and that any additional artifacts repos have been pulled with `torb artifacts refresh`.", path.to_str().unwrap());
            let entry = entry.expect(&error_string);
            if entry.path().is_dir() {
                let new_dest = dest.join(entry.path().file_name().unwrap());
                if !new_dest.exists() {
                    fs::create_dir(new_dest.clone()).expect("Unable to create supporting buildfile directory at destination, please check torb has been initialized properly.");
                }

                self._copy_files_recursively(entry.path(), new_dest.clone())
            } else {
                let path = entry.path();
                let new_path = dest.join(path.file_name().unwrap());
                println!("Copying {} to {}", path.display(), new_path.display());
                fs::copy(path, new_path).expect("Failed to copy supporting build file.");
            }
        }
    }

    fn write_main_buildfile(&mut self) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let builder = std::mem::take(&mut self.main_struct);
        let buildstate_path = buildstate_path_or_create();
        let environment_path = buildstate_path.join("iac_environment");
        let main_tf_path = environment_path.join("main.tf");

        let built_content = builder.build();

        let main_tf_content_hcl_string = hcl::to_string(&built_content)?;

        println!("{}", main_tf_content_hcl_string);

        fs::write(&main_tf_path, main_tf_content_hcl_string).expect("Failed to write main.tf");

        Ok(main_tf_path)
    }

    fn walk_artifact(&mut self, node: &ArtifactNodeRepr) -> Result<(), Box<dyn std::error::Error>> {
        // We want to walk to the end of the dependencies before we build.
        // This is because duplicate dependencies can exist, and we want to avoid building the same thing twice.
        // By walking to the end we ensure that whichever copy is built first will be in the set of seen nodes.
        // This let me avoid worrying about how to handle duplicate dependencies in the dependency tree data structure.
        // -Ian
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
            self.add_stack_node_to_main_struct(node).and_then(|_out| {
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

    fn create_output_data_block(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<Block, Box<dyn std::error::Error>> {
        let snake_case_release_name = self.release_name.clone().replace("-", "_");
        let namespace = self.artifact_repr.namespace(node);

        let name = node.fqn.clone().replace(".", "_");

        let data_block = Block::builder("data")
            .add_label("torb_helm_release")
            .add_label(format!("{}_{}", &snake_case_release_name, &node.display_name()))
            .add_attribute((
                "release_name",
                format!("{}-{}", self.release_name.clone(), snake_case_to_kebab(&node.name)),
            ))
            .add_attribute(("namespace", namespace))
            .add_attribute((
                "depends_on",
                Expression::from(vec![RawExpression::from(format!("module.{}", name))]),
            ))
            .build();

        Ok(data_block)
    }

    fn copy_build_files_for_node(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let buildstate_path = buildstate_path_or_create();
        let environment_path = buildstate_path.join("iac_environment");
        let node_source = node.source.clone().unwrap();
        let namespace_dir = kebab_to_snake_case(&node_source);
        let repo_path = environment_path.join(namespace_dir);

        if !repo_path.exists() {
            let error = format!(
                "Failed to create new repository namespace directory in environment for revision {}.",
                &self.hash
            );
            fs::create_dir(&repo_path).expect(&error);
        }

        let env_node_path = repo_path.join(format!("{}_module", &node.display_name()));

        if !env_node_path.exists() {
            let error = format!(
                "Failed to create new module directory in environment for revision {}.",
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

    fn create_input_values(&self, node: &ArtifactNodeRepr) -> Vec<Object<ObjectKey, Expression>> {
        let mut input_vals = Vec::<Object<ObjectKey, Expression>>::new();

        let resolver_fn = |spec: &String, input_address_result| {
            let mut input: Object<ObjectKey, Expression> = Object::new();

            input.insert(
                ObjectKey::Expression(Expression::String("name".to_string())),
                Expression::String(spec.clone()),
            );

            let mapped_expression = self.input_values_from_input_address(input_address_result);

            input.insert(
                ObjectKey::Expression(Expression::String("value".to_string())),
                mapped_expression.clone(),
            );

            if spec != "" {
                input_vals.push(input);
            }


            mapped_expression.clone().to_string()
        };

        let (_, _, _) = InputResolver::resolve(node, NO_VALUES_FN, Some(resolver_fn), NO_INITS_FN)
            .expect("Unable to resolve listed inputs.");

        input_vals
    }

    fn input_values_from_input_address(
        &self,
        input_address: Result<InputAddress, TorbInput>,
    ) -> Expression {
        match input_address {
            Ok(input_address) => {
                if reserved_outputs().contains_key(input_address.property_specifier.as_str()) {
                    let val = self.k8s_value_from_reserved_input(input_address);
                    val.clone()
                } else {
                    let val = self.k8s_status_values_path_from_torb_input(input_address);

                    Expression::Raw(RawExpression::new(val.clone()))
                }
            }
            Err(input_result) => {
                match input_result {
                    TorbInput::String(val) => Expression::String(val),
                    TorbInput::Bool(val) => Expression::String(val.to_string()),
                    TorbInput::Numeric(val) => {
                        match val {
                            TorbNumeric::Float(val) => Expression::String(Number::from_f64(val).unwrap().to_string()),
                            TorbNumeric::Int(val) => Expression::String(Number::from(val).to_string()),
                            TorbNumeric::NegInt(val) => Expression::String(Number::from(val).to_string())
                        }
                    }
                    TorbInput::Array(val) => {
                        Expression::String(self.torb_array_to_hcl_helm_array(val))
                    }
                }
                
            }
        }
    }

    fn torb_array_to_hcl_helm_array(&self, arr: Vec<TorbInput>) -> String {
        let mut new = Vec::<String>::new();
        for input in arr.iter().cloned() {
            let expr = match input {
                TorbInput::String(val) => Expression::String(val).to_string(),
                TorbInput::Bool(val) => Expression::Bool(val).to_string(),
                TorbInput::Numeric(val) => {
                    match val {
                        TorbNumeric::Float(val) => Expression::Number(Number::from_f64(val).unwrap()).to_string(),
                        TorbNumeric::Int(val) => Expression::Number(Number::from(val)).to_string(),
                        TorbNumeric::NegInt(val) => Expression::Number(Number::from(val)).to_string()
                    }
                }
                TorbInput::Array(_val) => {
                    panic!("Nested array types are not supported.")
                }
            };

            new.push(expr)
        }

        "{".to_owned() + &new.join(",") + "}"
    }

    fn add_required_providers_to_main_struct(&mut self) {
        let required_providers = Block::builder("terraform")
            .add_block(
                Block::builder("required_providers")
                    .add_attribute((
                        "torb",
                        Expression::from_iter(vec![
                            ("source", "TorbFoundry/torb"),
                            ("version", "0.1.2"),
                        ]),
                    ))
                    .build(),
            )
            .build();

        let torb_provider = Block::builder("provider").add_label("torb").build();

        let mut builder = std::mem::take(&mut self.main_struct);

        builder = builder.add_block(required_providers);
        builder = builder.add_block(torb_provider);

        self.main_struct = builder;
    }

    fn add_stack_node_to_main_struct(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let node_source = node.source.clone().unwrap();
        let namespace_dir = kebab_to_snake_case(&node_source);

        let source = format!("./{namespace_dir}/{}_module", node.display_name());
        let name = node.fqn.clone().replace(".", "_");

        let namespace = self.artifact_repr.namespace(node);

        let mut values = vec![];
        let mut attributes = vec![
            ("source", source),
            (
                "release_name",
                format!("{}-{}", self.release_name.clone(), snake_case_to_kebab(&node.name)),
            ),
            ("namespace", namespace),
        ];

        if node.build_step.is_some() {
            let build_step = node.build_step.clone().unwrap();
            let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
            let mut image_key_map: HashMap<String, String> = HashMap::new();

            if build_step.tag != "" {
                image_key_map.insert("tag".to_string(), build_step.tag);
            } else {
                image_key_map.insert("tag".to_string(), "latest".to_string());
            }

            if build_step.registry != "local" {
                image_key_map.insert("repository".to_string(), build_step.registry);
            } else {
                image_key_map.insert("repository".to_string(), node.display_name().clone());
            }

            map.insert("image".to_string(), image_key_map);

            values.push(serde_yaml::to_string(&map)?)
        }

        if node.deploy_steps["helm"].clone().unwrap()["repository"].clone() != "" {
            attributes.push((
                "repository",
                node.deploy_steps["helm"].clone().unwrap()["repository"].clone(),
            ));
            attributes.push((
                "chart_name",
                node.deploy_steps["helm"].clone().unwrap()["chart"].clone(),
            ));
        } else {
            // If repository is not specified, we assume that the chart is local.
            let local_path =
                torb_path().join(node.deploy_steps["helm"].clone().unwrap()["chart"].clone());
            attributes.push(("chart_name", local_path.to_str().unwrap().to_string()));
        }

        let mut depends_on_exprs = vec![];

        for dep in node.dependencies.iter() {
            let dep_fqn = &dep.fqn;

            if node.implicit_dependency_fqns.get(dep_fqn).is_none() {
                let dep_fqn_name = dep_fqn.clone().replace(".", "_");
                depends_on_exprs.push(RawExpression::from(format!("module.{dep_fqn_name}")))
            }
        }

        let module_version = node.deploy_steps["helm"]
            .clone()
            .unwrap()
            .get("version")
            .unwrap_or(&"".to_string())
            .clone();

        if module_version != "" {
            attributes.push(("version", module_version));
        }

        let output_block = self.create_output_data_block(node)?;

        let inputs = self.create_input_values(node);

        let resolver_fn = &mut |address: Result<InputAddress, TorbInput>| -> String {
            self.interpolate_inputs_into_helm_values(address)
        };

        let (mapped_values, _, _) = InputResolver::resolve(node, Some(resolver_fn), NO_INPUTS_FN, NO_INITS_FN)?;


        if mapped_values.clone().unwrap() != "---\n~\n" {
            values.push(mapped_values.expect("Unable to resolve values field."));
        }

        let mut builder = std::mem::take(&mut self.main_struct);

        let mut block = Block::builder("module")
                .add_label(&name)
                .add_attributes(attributes)
                .add_attribute(("inputs", inputs));

        if !values.is_empty() {
            block = block.add_attribute(("values", values));
        }

        if !depends_on_exprs.is_empty() {
            let depends_on = Expression::from(depends_on_exprs);

            block = block.add_attribute(("depends_on", depends_on));
        }

        builder = builder.add_block(
            block.build()
        );

        builder = builder.add_block(output_block);

        self.main_struct = builder;

        Ok(())
    }
}
