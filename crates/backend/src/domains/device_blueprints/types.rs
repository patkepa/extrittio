use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct BlueprintRecord {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub latest_revision: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BlueprintList {
    pub records: Vec<BlueprintRecord>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct BlueprintDraftRecord {
    pub id: String,
    pub blueprint_id: String,
    pub document: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BlueprintRevisionRecord {
    pub id: String,
    pub blueprint_id: String,
    pub revision: i32,
    pub document: Value,
    pub document_hash: String,
    pub compatibility: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateBlueprintRecord {
    pub id: String,
    pub draft_id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub document: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReplaceBlueprintDraftRecord {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub document: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PublishBlueprintRecord {
    pub revision_id: String,
    pub expected_draft_updated_at: DateTime<Utc>,
    pub document: Value,
    pub document_hash: String,
    pub compatibility: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum PublishBlueprintOutcome {
    Published(BlueprintRevisionRecord),
    BlueprintNotFound,
    DraftChanged,
}
