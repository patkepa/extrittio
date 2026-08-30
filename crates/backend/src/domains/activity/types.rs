use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ActivityQuery {
    pub source: Option<String>,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub device_id: Option<String>,
    pub search: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub until: Option<NaiveDateTime>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct ActivityEventRecord {
    pub id: String,
    pub source: String,
    pub severity: String,
    pub event_type: String,
    pub category: String,
    pub message: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub request_id: Option<String>,
    pub metadata: Value,
    pub occurred_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct ActivityEventPage {
    pub data: Vec<ActivityEventRecord>,
    pub total: i64,
}
