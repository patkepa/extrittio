use serde_json::Value;

/// Compute the delta between desired and reported shadow state.
/// Delta contains keys from desired that differ from reported.
#[must_use] 
pub fn compute_shadow_delta(desired: &Value, reported: &Value) -> Value {
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
        _ => desired.clone(),
    }
}
