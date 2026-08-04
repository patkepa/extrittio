use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRecord {
    pub id: i32,
    pub name: String,
    pub key_prefix: String,
    pub device_type_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeySummary {
    pub key: ApiKeyRecord,
    pub device_type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateApiKeyRecord {
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub device_type_id: Option<i32>,
}
