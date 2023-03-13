// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.5-03.13
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use crate::artifacts::{ArtifactNodeRepr, TorbInput};
use crate::composer::InputAddress;
use serde_yaml::Value;

use thiserror::Error;

const INIT_TOKEN: &str = "TORB";

#[derive(Error, Debug)]
pub enum TorbInputResolverErrors {}

pub const NO_INPUTS_FN: Option<Box<dyn FnMut(&String, Result<InputAddress, TorbInput>) -> String>> =
    None::<Box<dyn FnMut(&String, Result<InputAddress, TorbInput>) -> String>>;

pub const NO_VALUES_FN: Option<Box<dyn FnMut(Result<InputAddress, TorbInput>) -> String>> =
    None::<Box<dyn FnMut(Result<InputAddress, TorbInput>) -> String>>;

pub const NO_INITS_FN: Option<bool> = None;

pub struct InputResolver<'a, F, U> {
    node: &'a ArtifactNodeRepr,
    values_fn: Option<F>,
    inputs_fn: Option<U>,
    inits_fn: Option<bool>
}

impl<'a, F, U> InputResolver<'a, F, U> {
    pub fn resolve(
        node: &'a ArtifactNodeRepr,
        values_fn: Option<F>,
        inputs_fn: Option<U>,
        inits_fn: Option<bool>,
    ) -> Result<(Option<String>, Option<Vec<(String, String)>>, Option<Vec<String>>), Box<dyn std::error::Error>>
    where
        F: FnMut(Result<InputAddress, TorbInput>) -> String,
        U: FnMut(&String, Result<InputAddress, TorbInput>) -> String,
    {
        let mut resolver = InputResolver {
            node: node,
            values_fn,
            inputs_fn,
            inits_fn
        };

        let values_fn_out = if resolver.values_fn.is_some() {
            Some(resolver.resolve_inputs_in_values())
        } else {
            None
        };

        let inputs_fn_out = if resolver.inputs_fn.is_some() {
            Some(resolver.resolve_inputs_in_mapped_inputs())
        } else {
            None
        };

        let inits_fn_out = if resolver.inits_fn.is_some() {
            Some(resolver.resolve_node_init_script_inputs())
        } else {
            None
        };

        Ok((values_fn_out, inputs_fn_out, inits_fn_out))
    }

    fn resolve_inputs_in_mapped_inputs(&mut self) -> Vec<(String, String)>
    where
        U: FnMut(&String, Result<InputAddress, TorbInput>) -> String,
    {
        let f = self.inputs_fn.as_mut().unwrap();

        let mut out: Vec<(String, String)> = vec![];

        for (_, (spec, value)) in self.node.mapped_inputs.iter() {
            let input_address_result = InputAddress::try_from(value);

            let res = f(&spec.clone(), input_address_result.clone());

            out.push((spec.clone(), res));
        }

        out
    }


    pub fn resolve_node_init_script_inputs(&mut self) -> Vec<String> {
        let steps = self.node.init_step.clone().unwrap();
        steps.iter().map(|step| {
            self.resolve_torb_value_interpolation(step)
        }).collect::<Vec<String>>()
    }
    /*
        Case 1: Token at start
            Remaining = anything after token
        Case 2: Token in middle
            Remaining = anything before or after token
        Case 3: Token at end
            Remaining = anything before token
     */
    fn resolve_torb_value_interpolation(&mut self, script_step: &String) -> String {
        let start_option: Option<usize> = script_step.find(INIT_TOKEN);
        match start_option {
            Some(start) => {
                let mut end = script_step.split_at(start).1.find(" ").unwrap_or(script_step.len());
                end = script_step.split_at(start).1.find("/").unwrap_or(end);

                let remaining = if start == 0 && end == script_step.len() {
                    let resolved_token = self.resolve_inputs_in_init_step(script_step.to_string());
                    let serialized_token = resolved_token.serialize_for_init();

                    serialized_token
                } else if end == script_step.len() {
                    let parts = script_step.split_at(start);
                    let resolved_token = self.resolve_inputs_in_init_step(parts.1.to_string());
                    let remaining = parts.0.to_string();
                    let serialized_token = resolved_token.serialize_for_init();

                    format!("{}{}", remaining, serialized_token)
                } else if start == 0 {
                    let parts = script_step.split_at(end);
                    let resolved_token = self.resolve_inputs_in_init_step(parts.0.to_string());
                    let serialized_token = resolved_token.serialize_for_init();
                    let remaining = parts.1.to_string();
                    format!("{}{}", serialized_token, remaining)
                } else {
                    let parts = script_step.split_at(start);
                    let remaining_1 = parts.0.to_string();
                    let parts = parts.1.split_at(end);
                    let token = parts.0.to_string();
                    let remaining_2 = parts.1.to_string();

                    let resolved_token = self.resolve_inputs_in_init_step(token);

                    let serialized_token = resolved_token.serialize_for_init();
                    format!("{}{}{}", remaining_1, serialized_token, remaining_2)
                };

                self.resolve_torb_value_interpolation(&remaining.to_string())
            },
            None => {
                script_step.clone()
            }
        }
    }

    pub fn resolve_inputs_in_init_step(&mut self, token: String) -> TorbInput
    {
        let input = token.split("TORB.inputs.").collect::<Vec<&str>>()[1];

        let (_, val) = self.node.mapped_inputs.get(input).unwrap();

        val.clone()
    }

    pub fn resolve_inputs_in_values(&mut self) -> String
    where
        F: FnMut(Result<InputAddress, TorbInput>) -> String,
    {
        let yaml_str = self.node.values.as_str();
        let serde_value: Value = serde_yaml::from_str(yaml_str).unwrap_or(Value::Null);
        let resolved_values = self.resolve_inputs_in_helm_values(&serde_value);

        serde_yaml::to_string(&resolved_values).expect("Unable to convert value to string in resolver.")
    }

    fn resolve_inputs_in_helm_values(&mut self, value: &Value) -> Value
    where
        F: FnMut(Result<InputAddress, TorbInput>) -> String,
    {
        let f = self.values_fn.as_mut().unwrap();

        match value {
            Value::String(s) => {
                if s.starts_with("self.") {
                    let torb_input_address = InputAddress::try_from(s.as_str());

                    let string_value = f(torb_input_address);

                    Value::String(string_value)
                } else {
                    Value::String(s.to_string())
                }
            }
            Value::Mapping(m) => {
                let mut new_mapping = serde_yaml::Mapping::new();
                for (k, v) in m {
                    new_mapping.insert(k.clone(), self.resolve_inputs_in_helm_values(v));
                }

                Value::Mapping(new_mapping)
            }
            Value::Sequence(s) => {
                let mut new_seq = serde_yaml::Sequence::new();
                for v in s {
                    new_seq.push(self.resolve_inputs_in_helm_values(v).to_owned());
                }

                Value::Sequence(new_seq)
            }
            Value::Number(n) => Value::Number(n.to_owned()),
            Value::Bool(b) => Value::Bool(b.to_owned()),
            _ => Value::Null,
        }
    }
}
