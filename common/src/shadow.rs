use serde_json::Value;

/// Compute the delta between desired and reported shadow state.
/// Delta contains keys from desired that differ from reported.
pub fn compute_delta(desired: &Value, reported: &Value) -> Value {
    let desired_obj = desired.as_object();
    let reported_obj = reported.as_object();

    match (desired_obj, reported_obj) {
        (Some(d), Some(r)) => {
            let mut delta = serde_json::Map::new();
            for (key, val) in d {
                match r.get(key) {
                    Some(reported_val) if reported_val == val => {}
                    _ => {
                        delta.insert(key.clone(), val.clone());
                    }
                }
            }
            Value::Object(delta)
        }
        (Some(_d), None) => desired.clone(),
        _ => Value::Object(serde_json::Map::new()),
    }
}

/// Merge a JSON patch into an existing JSON object.
/// Keys with null values are removed; other keys are upserted.
/// Takes ownership of the existing value to avoid cloning.
pub fn merge_json(existing: Value, patch: &serde_json::Map<String, Value>) -> Value {
    let mut obj = match existing {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    for (key, val) in patch {
        if val.is_null() {
            obj.remove(key);
        } else {
            obj.insert(key.clone(), val.clone());
        }
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compute_delta_finds_differences() {
        let desired = json!({"a": 1, "b": 2, "c": 3});
        let reported = json!({"a": 1, "b": 99});
        let delta = compute_delta(&desired, &reported);
        assert_eq!(delta, json!({"b": 2, "c": 3}));
    }

    #[test]
    fn test_compute_delta_empty_when_in_sync() {
        let state = json!({"a": 1, "b": 2});
        let delta = compute_delta(&state, &state);
        assert_eq!(delta, json!({}));
    }

    #[test]
    fn test_merge_json_upserts() {
        let existing = json!({"a": 1, "b": 2});
        let mut patch = serde_json::Map::new();
        patch.insert("b".into(), json!(99));
        patch.insert("c".into(), json!(3));
        let result = merge_json(existing, &patch);
        assert_eq!(result, json!({"a": 1, "b": 99, "c": 3}));
    }

    #[test]
    fn test_merge_json_removes_nulls() {
        let existing = json!({"a": 1, "b": 2});
        let mut patch = serde_json::Map::new();
        patch.insert("b".into(), Value::Null);
        let result = merge_json(existing, &patch);
        assert_eq!(result, json!({"a": 1}));
    }
}
