use std::time::Duration;

use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::outbox_types::{OutboxEventRecord, OutboxSummaryRecord};

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_timeout: Duration,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError>;
    async fn mark_succeeded(
        &self,
        event_id: &str,
        worker_id: &str,
    ) -> Result<bool, PersistenceError>;
    async fn mark_failed(
        &self,
        event_id: &str,
        worker_id: &str,
        attempts: i32,
        max_attempts: i32,
        error: &str,
    ) -> Result<bool, PersistenceError>;
    async fn summary(&self, tenant: &TenantId) -> Result<OutboxSummaryRecord, PersistenceError>;
    async fn list_dead_letters(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError>;
    async fn replay_dead_letters(
        &self,
        tenant: &TenantId,
        event_ids: Option<Vec<String>>,
    ) -> Result<usize, PersistenceError>;
}
