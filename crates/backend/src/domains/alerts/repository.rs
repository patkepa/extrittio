use async_trait::async_trait;
use chrono::NaiveDateTime;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, CooldownRecord,
};

#[async_trait]
pub trait AlertRepository: Send + Sync {
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
    async fn delete_resolved_before(
        &self,
        tenant: &TenantId,
        cutoff: NaiveDateTime,
    ) -> Result<usize, PersistenceError>;
}
