use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct NewDeviceContractRecord {
    pub id: String,
    pub blueprint_revision_id: String,
    pub document: Value,
    /// Compiled configuration defaults and overrides, committed with assignment.
    pub initial_configuration: Option<Value>,
    pub contract_hash: String,
    pub created_at: DateTime<Utc>,
}
