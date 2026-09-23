use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
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
    ActiveConflict(String),
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
pub struct NewRuleAlertRecord {
    pub id: String,
    pub rule_id: String,
    pub device_id: String,
    pub severity: String,
    pub message: String,
    pub triggered_value: Option<String>,
}

#[async_trait]
pub trait AlertRepository: Send + Sync {
    /// Persist an action receipt in the same transaction as inserting/reusing
    /// an active alert. A receipt prevents re-creation after resolve/retention.
    /// None means the delivery was applied but its alert has since been removed.
    async fn create_or_get_active(
        &self,
        tenant: &TenantId,
        record: NewRuleAlertRecord,
    ) -> Result<Option<AlertRecord>, PersistenceError>;
    async fn update_triggered_value(
        &self,
        tenant: &TenantId,
        id: &str,
        value: String,
    ) -> Result<bool, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        filter: AlertListFilter,
    ) -> Result<(Vec<AlertRecord>, i64), PersistenceError>;
    async fn get(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<AlertRecord>, PersistenceError>;
    async fn transition(
        &self,
        tenant: &TenantId,
        id: &str,
        transition: AlertTransition,
    ) -> Result<AlertTransitionOutcome, PersistenceError>;
    async fn transition_many(
        &self,
        tenant: &TenantId,
        ids: Vec<String>,
        transition: AlertTransition,
    ) -> Result<Vec<AlertRecord>, PersistenceError>;
    async fn summary(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<(String, String, i64)>, PersistenceError>;
    /// System-scoped retention operation. Unlike tenant-facing alert CRUD,
    /// retention must cover every tenant and must never fabricate a default
    /// tenant identity.
    async fn delete_all_resolved_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}

impl AlertTransition {
    pub fn requires_active_slot(self) -> bool {
        matches!(self, Self::Reactivate)
    }
    /// Evaluated by adapters while the alert is locked in its transaction.
    pub fn accepts(self, status: &str) -> bool {
        match self {
            Self::Acknowledge => status == "active",
            Self::Resolve => status != "resolved",
            Self::Reactivate => status != "active",
        }
    }
}
