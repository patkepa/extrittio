use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::ContractError;

pub(crate) fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ContractError> {
    let value = serde_json::to_value(value)?;
    serde_json::to_vec(&sort_value(value)).map_err(ContractError::from)
}

pub(crate) fn sha256(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(sort_value).collect()),
        Value::Object(values) => {
            let mut keys = values.keys().cloned().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut sorted = Map::new();
            for key in keys {
                if let Some(value) = values.get(&key) {
                    sorted.insert(key, sort_value(value.clone()));
                }
            }
            Value::Object(sorted)
        }
        primitive => primitive,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn recursively_sorts_object_keys() {
        let left = json!({"z": {"b": 2, "a": 1}, "a": 0});
        let right = json!({"a": 0, "z": {"a": 1, "b": 2}});
        assert_eq!(
            canonical_json(&left).unwrap(),
            canonical_json(&right).unwrap()
        );
    }
}
