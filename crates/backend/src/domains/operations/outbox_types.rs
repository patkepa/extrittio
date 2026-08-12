use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OutboxEventRecord {
    pub id: String,
    pub tenant_id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct OutboxSummaryRecord {
    pub pending_count: i64,
    pub processing_count: i64,
    pub failed_count: i64,
    pub dead_letter_count: i64,
    pub succeeded_count: i64,
    pub oldest_pending_at: Option<NaiveDateTime>,
    pub oldest_pending_age_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewOutboxEventRecord {
    pub id: String,
    pub tenant_id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub idempotency_key: Option<String>,
    pub payload: Value,
}
