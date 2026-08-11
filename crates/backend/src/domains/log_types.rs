use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub id: i64,
    pub device_id: String,
    pub level: String,
    pub message: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct LogQuery {
    pub level: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub limit: i64,
}
