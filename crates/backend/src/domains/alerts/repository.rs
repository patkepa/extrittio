use async_trait::async_trait;
use chrono::NaiveDateTime;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, CooldownRecord,
    NewAlertRecord,
};

#[async_trait]
pub trait AlertRepository: Send + Sync {
    async fn create(
        &self,
        tenant: &TenantId,
        record: NewAlertRecord,
    ) -> Result<AlertRecord, PersistenceError>;
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
    async fn persist_cooldowns(
        &self,
        cooldowns: Vec<CooldownRecord>,
    ) -> Result<(), PersistenceError>;
    /// System-scoped retention operation. Unlike tenant-facing alert CRUD,
    /// retention must cover every tenant and must never fabricate a default
    /// tenant identity.
    async fn delete_all_resolved_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}
