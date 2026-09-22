use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceConfigRecord {
    pub device_id: String,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GetDeviceConfigOutcome {
    DeviceNotFound,
    Found(Option<DeviceConfigRecord>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MergeDeviceConfigOutcome {
    DeviceNotFound,
    Updated(DeviceConfigRecord),
}

#[must_use]
pub fn merge_config(current: Value, patch: &Map<String, Value>) -> Value {
    let mut object = current.as_object().cloned().unwrap_or_default();
    for (key, value) in patch {
        if value.is_null() {
            object.remove(key);
        } else {
            object.insert(key.clone(), value.clone());
        }
    }
    Value::Object(object)
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "tests stay next to the configuration merge helper they cover"
)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn merge_replaces_adds_and_removes_object_keys() {
        let patch = json!({ "keep": 2, "add": true, "remove": null });
        let merged = merge_config(
            json!({ "keep": 1, "remove": "old" }),
            patch.as_object().unwrap(),
        );

        assert_eq!(merged, json!({ "keep": 2, "add": true }));
    }

    #[test]
    fn merge_normalizes_non_object_current_values() {
        let patch = json!({ "enabled": true });
        assert_eq!(
            merge_config(json!([1, 2]), patch.as_object().unwrap()),
            patch
        );
    }
}

#[async_trait]
pub trait DeviceConfigRepository: Send + Sync {
    async fn get_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<GetDeviceConfigOutcome, PersistenceError>;

    /// Locks the device row, merges the patch against the latest stored value,
    /// and upserts the result in one transaction.
    async fn merge_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<MergeDeviceConfigOutcome, PersistenceError>;
}
