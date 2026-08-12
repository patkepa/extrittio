use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct AlertRecord {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: Option<String>,
    pub device_id: String,
    pub severity: String,
    pub status: String,
    pub message: String,
    pub triggered_value: Option<String>,
    pub resolved_at: Option<NaiveDateTime>,
    pub acknowledged_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AlertListFilter {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub device_id: Option<String>,
    pub rule_id: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum AlertTransition {
    Acknowledge,
    Resolve,
    Reactivate,
}

#[derive(Debug, Clone)]
pub enum AlertTransitionOutcome {
    NotFound,
    InvalidStatus(String),
    Updated(Box<AlertRecord>),
}

#[derive(Debug, Clone)]
pub struct CooldownRecord {
    pub tenant_id: String,
    pub rule_id: String,
    pub device_id: String,
    pub last_fired_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewAlertRecord {
    pub id: String,
    pub rule_id: Option<String>,
    pub device_id: String,
    pub severity: String,
    pub message: String,
    pub triggered_value: Option<String>,
}
