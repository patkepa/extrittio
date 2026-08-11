use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRecord {
    pub id: String,
    pub device_id: String,
    pub command: String,
    pub params: String,
    pub status: String,
    pub response_payload: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewCommandRecord {
    pub id: String,
    pub command: String,
    pub params: String,
}

#[derive(Debug, Clone)]
pub struct CommandQuery {
    pub status: Option<String>,
    pub limit: i64,
}
