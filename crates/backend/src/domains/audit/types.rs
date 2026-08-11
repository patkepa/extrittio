use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AuditEventRecord {
    pub id: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub outcome: String,
    pub request_id: String,
    pub metadata: Value,
    pub occurred_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewAuditEventRecord {
    pub id: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub outcome: String,
    pub request_id: String,
    pub metadata: Value,
}
