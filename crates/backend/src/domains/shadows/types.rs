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
    let desired = extrittio_common::shadow::merge_json(object_or_empty(shadow.desired), patch);
    let reported = object_or_empty(shadow.reported);
    shadow.delta = extrittio_common::shadow::compute_delta(&desired, &reported);
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
    let reported = extrittio_common::shadow::merge_json(object_or_empty(shadow.reported), patch);
    shadow.delta = extrittio_common::shadow::compute_delta(&desired, &reported);
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
