use crate::artifacts::ArtifactNodeRepr;
use crate::composer::InputAddress;
use serde_yaml::Value;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TorbInputResolverErrors {}

pub const NO_INPUTS_FN: Option<Box<dyn FnMut(&String, Result<InputAddress, String>) -> String>> =
    None::<Box<dyn FnMut(&String, Result<InputAddress, String>) -> String>>;

pub const NO_VALUES_FN: Option<Box<dyn FnMut(Result<InputAddress, String>) -> String>> =
    None::<Box<dyn FnMut(Result<InputAddress, String>) -> String>>;

pub struct InputResolver<'a, F, U> {
    node: &'a ArtifactNodeRepr,
    values_fn: Option<F>,
    inputs_fn: Option<U>,
}

impl<'a, F, U> InputResolver<'a, F, U> {
    pub fn resolve(
        node: &'a ArtifactNodeRepr,
        values_fn: Option<F>,
        inputs_fn: Option<U>,
    ) -> Result<(Option<String>, Option<Vec<(String, String)>>), Box<dyn std::error::Error>>
    where
        F: FnMut(Result<InputAddress, String>) -> String,
        U: FnMut(&String, Result<InputAddress, String>) -> String,
    {
        let mut resolver = InputResolver {
            node: node,
            values_fn,
            inputs_fn,
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

        Ok((values_fn_out, inputs_fn_out))
    }

    fn resolve_inputs_in_mapped_inputs(&mut self) -> Vec<(String, String)>
    where
        U: FnMut(&String, Result<InputAddress, String>) -> String,
    {
        let f = self.inputs_fn.as_mut().unwrap();

        let mut out: Vec<(String, String)> = vec![];

        for (_, (spec, value)) in self.node.mapped_inputs.iter() {
            let input_address_result = InputAddress::try_from(value.clone().as_str());

            let res = f(&spec.clone(), input_address_result.clone());

            out.push((spec.clone(), res));
        }

        out
    }

    pub fn resolve_inputs_in_values(&mut self) -> String
    where
        F: FnMut(Result<InputAddress, String>) -> String,
    {
        let yaml_str = self.node.values.as_str();
        let serde_value: Value = serde_yaml::from_str(yaml_str).unwrap_or(Value::Null);
        let resolved_values = self.resolve_inputs_in_helm_values(&serde_value);

        serde_yaml::to_string(&resolved_values).expect("Unable to convet value to string in resolver.")
    }

    fn resolve_inputs_in_helm_values(&mut self, value: &Value) -> Value
    where
        F: FnMut(Result<InputAddress, String>) -> String,
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
