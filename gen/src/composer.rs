use crate::artifacts::{ArtifactNodeRepr, ArtifactRepr};
use crate::utils::{torb_path, buildstate_path_or_create};
use hcl::{Block, Body, Expression, RawExpression, Object, ObjectKey};
use memorable_wordlist;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbComposerErrors {
}

fn reserved_outputs() -> HashMap<&'static str, &'static str> {
    let reserved = vec![
        ("host", ""),
    ];

    let mut reserved_hash = HashMap::new();

    for (k, v) in reserved {
        reserved_hash.insert(k, v);
    }

    reserved_hash
}

fn kebab_to_snake_case(input: &str) -> String {
    input.replace("-", "_")
}

#[derive(Debug, Clone)]
struct InputAddress {
    locality: String,
    node_type: String,
    node_name: String,
    node_property: String,
    property_specifier: String,
}

impl InputAddress {
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
}

impl TryFrom<&str> for InputAddress {
    type Error = String;

    fn try_from(input: &str) -> Result<Self, String> {
        let vals = input.split(".").collect::<Vec<&str>>();

        if vals.len() == 5 {
            let locality = vals[0].to_string();
            let node_type = vals[1].to_string();
            let node_name = vals[2].to_string();
            let node_property = vals[3].to_string();
            let property_specifier = vals[4].to_string();

            Ok(InputAddress::new(
                locality,
                node_type,
                node_name,
                node_property,
                property_specifier,
            ))
        } else {
            Err(input.to_string())
        }
    }
}

pub struct Composer<'a> {
    hash: String,
    build_files_seen: HashSet<String>,
    fqn_seen: HashSet<String>,
    release_name: String,
    main_struct: hcl::BodyBuilder,
    artifact_repr: &'a ArtifactRepr,
}

impl<'a> Composer<'a> {
    pub fn new(hash: String, artifact_repr: &ArtifactRepr) -> Composer {
        let memorable_words = memorable_wordlist::kebab_case(16);

        Composer {
            hash: hash,
            build_files_seen: HashSet::new(),
            fqn_seen: HashSet::new(),
            release_name: memorable_words,
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
        println!("output_node_fqn: {}", output_node_fqn);
        println!("{:?}", self.artifact_repr.nodes);
        self
            .artifact_repr
            .nodes
            .get(&output_node_fqn)
            .expect("Unable to map input address to node, make sure your mapping is correct.")
    }

    fn interpolate_inputs_into_helm_values(&self, value: &serde_yaml::Value) -> serde_yaml::Value {
        match value {
            serde_yaml::Value::String(s) => {
                if s.starts_with("self.") {
                    let torb_input_address = InputAddress::try_from(s.as_str());
                    let output_value = self.input_values_from_input_address(torb_input_address.clone());
                    let mut string_value = hcl::format::to_string(&output_value).unwrap(); 

                    string_value = match torb_input_address {
                        Ok(input_address) => {
                            if reserved_outputs().contains_key(input_address.property_specifier.as_str()) {
                                string_value.replace("\"", "")
                            } else {
                                format!("${{{}}}", string_value.replace("\"", ""))
                            }
                        },
                        Err(s) => {
                            s
                        }
                    };
                    println!("string_value: {:?}", string_value);
                    println!("string_value: {:?}", string_value);
                    println!("string_value: {:?}", string_value);
                    println!("string_value: {:?}", string_value);
                    

                    serde_yaml::Value::String(string_value)
                } else {
                    serde_yaml::Value::String(s.to_string())
                }
            }
            serde_yaml::Value::Mapping(m) => {
                let mut new_mapping = serde_yaml::Mapping::new();
                for (k, v) in m {
                    new_mapping.insert(k.clone(), self.interpolate_inputs_into_helm_values(v));
                }

                serde_yaml::Value::Mapping(new_mapping)
            }
            serde_yaml::Value::Sequence(s) => {
                let mut new_seq = serde_yaml::Sequence::new();
                for v in s {
                    new_seq.push(self.interpolate_inputs_into_helm_values(v).to_owned());
                }

                serde_yaml::Value::Sequence(new_seq)
            }
            serde_yaml::Value::Number(n) => {
                serde_yaml::Value::Number(n.to_owned())
            }
            serde_yaml::Value::Bool(b) => {
                serde_yaml::Value::Bool(b.to_owned())
            }
            _ => serde_yaml::Value::Null,
        }
    }

    fn k8s_value_from_reserved_input(&self, torb_input_address: InputAddress) -> Expression {
        let output_node = self.get_node_for_output_value(&torb_input_address);

        match torb_input_address.property_specifier.as_str() {
            "host" => {
                let name = format!("{}-{}", self.release_name, output_node.name);
                let namespace = self.artifact_repr.stack_name.clone();

                Expression::String(format!("{}.{}.svc.cluster.local", name, namespace))
            },
            _ => {
                panic!("Unable to map reserved value.")
            }
        }        
    }

    fn k8s_status_values_path_from_torb_input(&self, torb_input_address: InputAddress) -> String {
        let output_node = self.get_node_for_output_value(&torb_input_address);

        let kube_value = if torb_input_address.node_property == "output" {
            let (kube_val, _) = output_node
                .mapped_inputs
                .get(&torb_input_address.property_specifier)
                .expect("Unable to map input from output node. Key does not exist.");

            kube_val
        } else {
            panic!("Unable to map node_property to output attribute please check your inputs, ex: 'a.b.output.c or a.b.input.c");
        };

        let formatted_name = kebab_to_snake_case(&self.release_name);
        let block_name = format!("{}_{}", formatted_name, &output_node.name);

        format!("jsondecode(data.torb_helm_release.{}.values)[\"{}\"]", block_name, kube_value)
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
        let path = torb_path();
        let supporting_build_files_path = path.join("torb-artifacts/common");
        let buildstate_path = buildstate_path_or_create();
        let new_environment_path = buildstate_path.join("iac_environment");
        let dest =
            new_environment_path.join(supporting_build_files_path.as_path().file_name().unwrap());

        if !dest.exists() {
            fs::create_dir(dest.clone()).expect("Unable to create supporting buildfile directory at destination, please check torb has been initialized properly.");
        }

        self._copy_files_recursively(supporting_build_files_path, dest);

        let provider_path = path.join("torb-artifacts/common/providers");
        let dest = new_environment_path.clone();

        self._copy_files_recursively(provider_path, dest);

        Ok(())
    }

    fn _copy_files_recursively(&self, path: std::path::PathBuf, dest: std::path::PathBuf) -> () {
        let error_string = format!("Failed reading torb-artifacts dir: {}. Please check that torb is correctly initialized.", path.to_str().unwrap());
        for entry in path.read_dir().expect(&error_string) {
            let error_string = format!("Failed reading entry in torb-artifacts dir: {}. Please check that torb is correctly initialized.", path.to_str().unwrap());
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
        let namespace = node
            .fqn
            .split(".")
            .next()
            .unwrap()
            .to_string()
            .replace("_", "-");
        let name = node.fqn.clone().replace(".", "_");

        let data_block = Block::builder("data")
            .add_label("torb_helm_release")
            .add_label(format!("{}_{}", &snake_case_release_name, &node.name))
            .add_attribute(("release_name", format!("{}-{}", self.release_name.clone(), node.name)))
            .add_attribute(("namespace", namespace))
            .add_attribute(("depends_on", Expression::from(vec![RawExpression::from(format!("module.{}", name))])))
            .build();

        Ok(data_block)
    }

    fn copy_build_files_for_node(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let buildstate_path = buildstate_path_or_create();
        let environment_path = buildstate_path.join("iac_environment");
        let env_node_path = environment_path.join(format!("{}_module", &node.name));

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

        for (_, (spec, value)) in node.mapped_inputs.iter() {
            let input_address_result = InputAddress::try_from(value.clone().as_str());
            
            let mut input: Object<ObjectKey, Expression> = Object::new();

            input.insert(ObjectKey::Expression(Expression::String("name".to_string())), Expression::String(spec.clone()));
            input.insert(ObjectKey::Expression(Expression::String("value".to_string())), self.input_values_from_input_address(input_address_result));

            input_vals.push(input);
        }

        input_vals
    }

    fn input_values_from_input_address(&self, input_address: Result<InputAddress, String>) -> Expression {
        match input_address {
            Ok(input_address) => {
                if reserved_outputs().contains_key(input_address.property_specifier.as_str()) {
                    let val = self.k8s_value_from_reserved_input(input_address);
                    val.clone()

                } else {
                    let val = self.k8s_status_values_path_from_torb_input(input_address);

                    Expression::Raw(RawExpression::new(val.clone()))
                }
            },
            Err(input_result) => {
                Expression::String(input_result)
            },
        }
    }

    fn add_required_providers_to_main_struct(&mut self) {
        let required_providers = Block::builder("terraform")
            .add_block(Block::builder("required_providers")
                .add_attribute(("torb", Expression::from_iter(
                    vec![
                        ("source", "TorbFoundry/torb"),
                        ("version", "0.1.1")
                    ]
                )))
                .build()).build();

        let torb_provider = Block::builder("provider")
            .add_label("torb")
            .build();

        let mut builder = std::mem::take(&mut self.main_struct);

        builder = builder.add_block(required_providers);
        builder = builder.add_block(torb_provider);

        self.main_struct = builder;
    }

    fn add_stack_node_to_main_struct(
        &mut self,
        node: &ArtifactNodeRepr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = format!("./{}_module", node.name);
        let name = node.fqn.clone().replace(".", "_");
        let namespace = node
            .fqn
            .split(".")
            .next()
            .unwrap()
            .to_string()
            .replace("_", "-");

        let mut values = vec![];
        let mut attributes = vec![
            ("source", source),
            ("release_name", format!("{}-{}", self.release_name.clone(), node.name)),
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
            
            if build_step.registry != "" {
                image_key_map.insert("repository".to_string(), build_step.registry);
            } else {
                image_key_map.insert("repository".to_string(), node.name.clone());
            }

            map.insert("image".to_string(), image_key_map);

            values.push(serde_yaml::to_string(&map)?)
        }

        if node.deploy_steps["helm"].clone().unwrap()["repository"].clone() != "" {
            attributes.push((
                "repository",
                node.deploy_steps["helm"].clone().unwrap()["repository"].clone(),
            ));
            attributes.push(
            (
                "chart_name",
                node.deploy_steps["helm"].clone().unwrap()["chart"].clone(),
            ));
        } else {
            // If repository is not specified, we assume that the chart is local.
            let local_path = torb_path().join(node.deploy_steps["helm"].clone().unwrap()["chart"].clone());
            attributes.push(
            (
                "chart_name",
                local_path.to_str().unwrap().to_string(),
            ));
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
        let yaml_str = node.values.as_str();
        let serde_value: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap_or(serde_yaml::Value::Null);

        let mapped_values = self.interpolate_inputs_into_helm_values(&serde_value);

        let yaml_string = serde_yaml::to_string(&mapped_values).expect("Unable to convert values config to yaml.");

        values.push(yaml_string);

        let mut builder = std::mem::take(&mut self.main_struct);

        builder = builder.add_block(
            Block::builder("module")
                .add_label(&name)
                .add_attributes(attributes)
                .add_attribute(("values", values))
                .add_attribute(("inputs", inputs))
                .build(),
        );

        builder = builder.add_block(output_block);

        self.main_struct = builder;

        Ok(())
    }
}
