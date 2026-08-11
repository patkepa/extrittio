use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ZoneRecord {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: Value,
    pub color: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewZoneRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: Value,
    pub color: String,
}

#[derive(Debug, Clone)]
pub struct UpdateZoneRecord {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<Value>,
    pub color: Option<String>,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy)]
pub enum DeleteZoneOutcome {
    NotFound,
    InUse,
    Deleted,
}
