use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OutboxEventRecord {
    /// Opaque token assigned to this lease. Absent for unclaimed/dead-letter rows.
    pub claim_token: Option<String>,
    pub id: String,
    pub tenant_id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: Value,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct OutboxSummaryRecord {
    pub pending_count: i64,
    pub processing_count: i64,
    pub failed_count: i64,
    pub dead_letter_count: i64,
    pub succeeded_count: i64,
    pub oldest_pending_at: Option<NaiveDateTime>,
    pub oldest_pending_age_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewOutboxEventRecord {
    pub id: String,
    pub tenant_id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub idempotency_key: Option<String>,
    pub payload: Value,
}

#[async_trait]
pub trait OutboxRepository: Send + Sync {
    /// System-scoped claim. Rank eligible rows within each tenant by
    /// (available_at, created_at, id), then across tenants by (rank,
    /// available_at, created_at, id). Return that order, omitting locked rows.
    /// Each claim assigns a fresh opaque token, including same-worker reclaims.
    async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_timeout: Duration,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError>;
    async fn mark_succeeded(
        &self,
        event_id: &str,
        claim_token: &str,
    ) -> Result<bool, PersistenceError>;
    /// Update only the processing row matching this claim token and attempt.
    /// False means ownership was superseded; callers must not retry the update
    /// using a newer token. Claim ownership does not imply exactly-once delivery.
    async fn mark_failed(
        &self,
        event_id: &str,
        claim_token: &str,
        attempts: i32,
        max_attempts: i32,
        failure: FailureClass,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    Retryable,
    Permanent,
}

/// Preserve the existing exponential schedule: 2 through 256 seconds.
pub fn failure_disposition(
    attempts: i32,
    max_attempts: i32,
    failure: FailureClass,
) -> (&'static str, i64) {
    let terminal = failure == FailureClass::Permanent || attempts >= max_attempts;
    (
        if terminal { "dead_letter" } else { "failed" },
        2_i64.pow(attempts.clamp(1, 8) as u32).min(300),
    )
}
