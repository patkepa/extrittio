use crate::{PersistenceError, TenantId};
use async_trait::async_trait;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowRecord {
    pub device_id: String,
    pub desired: Value,
    pub reported: Value,
    pub delta: Value,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShadowMutationError {
    #[error("shadow version overflow")]
    VersionOverflow,
}

fn object_or_empty(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        Value::Object(Map::default())
    }
}

fn next_version(version: i32) -> Result<i32, ShadowMutationError> {
    version
        .checked_add(1)
        .ok_or(ShadowMutationError::VersionOverflow)
}

pub fn apply_desired_patch(
    mut shadow: ShadowRecord,
    patch: &Map<String, Value>,
    updated_at: DateTime<Utc>,
) -> Result<ShadowRecord, ShadowMutationError> {
    let desired = merge_json(object_or_empty(shadow.desired), patch);
    let reported = object_or_empty(shadow.reported);
    shadow.delta = compute_delta(&desired, &reported);
    shadow.desired = desired;
    shadow.reported = reported;
    shadow.version = next_version(shadow.version)?;
    shadow.updated_at = updated_at;
    Ok(shadow)
}

pub fn apply_reported_patch(
    mut shadow: ShadowRecord,
    patch: &Map<String, Value>,
    updated_at: DateTime<Utc>,
) -> Result<ShadowRecord, ShadowMutationError> {
    let desired = object_or_empty(shadow.desired);
    let reported = merge_json(object_or_empty(shadow.reported), patch);
    shadow.delta = compute_delta(&desired, &reported);
    shadow.desired = desired;
    shadow.reported = reported;
    shadow.version = next_version(shadow.version)?;
    shadow.updated_at = updated_at;
    Ok(shadow)
}

pub fn reset_shadow(
    mut shadow: ShadowRecord,
    updated_at: DateTime<Utc>,
) -> Result<ShadowRecord, ShadowMutationError> {
    let empty = Value::Object(Map::default());
    shadow.desired = empty.clone();
    shadow.reported = empty.clone();
    shadow.delta = empty;
    shadow.version = next_version(shadow.version)?;
    shadow.updated_at = updated_at;
    Ok(shadow)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;

    fn shadow() -> ShadowRecord {
        ShadowRecord {
            device_id: "device-1".to_string(),
            desired: json!({ "sample_rate": 10, "enabled": true }),
            reported: json!({ "sample_rate": 5, "enabled": true }),
            delta: json!({}),
            version: 3,
            updated_at: Utc.timestamp_opt(1, 0).unwrap(),
        }
    }

    #[test]
    fn desired_patch_recomputes_delta_and_increments_version() {
        let now = Utc.timestamp_opt(2, 0).unwrap();
        let patch = json!({ "sample_rate": 5, "enabled": null, "mode": "eco" });

        let result = apply_desired_patch(shadow(), patch.as_object().unwrap(), now).unwrap();

        assert_eq!(result.desired, json!({ "sample_rate": 5, "mode": "eco" }));
        assert_eq!(result.delta, json!({ "mode": "eco" }));
        assert_eq!(result.version, 4);
        assert_eq!(result.updated_at, now);
    }

    #[test]
    fn reported_patch_recomputes_delta_and_increments_version() {
        let patch = json!({ "sample_rate": 10 });
        let result = apply_reported_patch(
            shadow(),
            patch.as_object().unwrap(),
            Utc.timestamp_opt(2, 0).unwrap(),
        )
        .unwrap();

        assert_eq!(result.reported["sample_rate"], 10);
        assert_eq!(result.delta, json!({}));
        assert_eq!(result.version, 4);
    }

    #[test]
    fn version_overflow_is_rejected() {
        let mut record = shadow();
        record.version = i32::MAX;
        assert_eq!(
            reset_shadow(record, Utc::now()).unwrap_err(),
            ShadowMutationError::VersionOverflow
        );
    }
}

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

#[async_trait]
pub trait ShadowRepository: Send + Sync {
    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<ShadowRecord>, PersistenceError>;

    async fn update_desired(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError>;

    async fn update_reported(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError>;

    async fn reset(
        &self,
        tenant: &TenantId,
        device_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError>;
}
