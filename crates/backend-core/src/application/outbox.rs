use super::require_permission;
use crate::outbox::{FailureClass, OutboxEventRecord, OutboxRepository, OutboxSummaryRecord};
use crate::{ApplicationError, Permission, TenantContext};
use std::{sync::Arc, time::Duration};

#[derive(Clone)]
pub struct OutboxApplication {
    repository: Arc<dyn OutboxRepository>,
}
impl OutboxApplication {
    pub fn new(repository: Arc<dyn OutboxRepository>) -> Self {
        Self { repository }
    }
    pub async fn summary(
        &self,
        ctx: &TenantContext,
    ) -> Result<OutboxSummaryRecord, ApplicationError> {
        require_permission(ctx, Permission::ReadServerMetrics)?;
        Ok(self.repository.summary(ctx.tenant_id()).await?)
    }
    pub async fn dead_letters(
        &self,
        ctx: &TenantContext,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OutboxEventRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadServerMetrics)?;
        Ok(self
            .repository
            .list_dead_letters(ctx.tenant_id(), limit.clamp(1, 200), offset.max(0))
            .await?)
    }
    pub async fn replay(
        &self,
        ctx: &TenantContext,
        event_ids: Vec<String>,
        replay_all: bool,
    ) -> Result<usize, ApplicationError> {
        if !replay_all && event_ids.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "provide event_ids or set replay_all to true".into(),
            ));
        }
        if replay_all && !event_ids.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "event_ids and replay_all are mutually exclusive".into(),
            ));
        }
        require_permission(ctx, Permission::ManageRules)?;
        let ids = (!replay_all).then(|| {
            event_ids
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        });
        Ok(self
            .repository
            .replay_dead_letters(ctx.tenant_id(), ids)
            .await?)
    }
}

/// System-scoped worker operations, kept off the HTTP application façade.
#[derive(Clone)]
pub struct OutboxWorkerApplication {
    repository: Arc<dyn OutboxRepository>,
}
impl OutboxWorkerApplication {
    pub fn new(repository: Arc<dyn OutboxRepository>) -> Self {
        Self { repository }
    }
    pub async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease: Duration,
    ) -> Result<Vec<OutboxEventRecord>, ApplicationError> {
        if worker_id.is_empty()
            || limit <= 0
            || lease.is_zero()
            || lease.as_micros() > i64::MAX as u128
        {
            return Err(ApplicationError::InvalidInput(
                "outbox claims require a worker label, positive batch size, and representable positive lease".into(),
            ));
        }
        let events = self.repository.claim_batch(worker_id, limit, lease).await?;
        if events
            .iter()
            .any(|event| event.claim_token.as_deref().is_none_or(str::is_empty))
        {
            return Err(ApplicationError::Internal(
                "claimed outbox event has no lease token".into(),
            ));
        }
        Ok(events)
    }
    pub async fn succeeded(&self, event: &OutboxEventRecord) -> Result<bool, ApplicationError> {
        let token = event
            .claim_token
            .as_deref()
            .ok_or_else(|| ApplicationError::Internal("event is not claimed".into()))?;
        Ok(self.repository.mark_succeeded(&event.id, token).await?)
    }
    pub async fn failed(
        &self,
        event: &OutboxEventRecord,
        failure: FailureClass,
        error: &str,
    ) -> Result<bool, ApplicationError> {
        let token = event
            .claim_token
            .as_deref()
            .ok_or_else(|| ApplicationError::Internal("event is not claimed".into()))?;
        Ok(self
            .repository
            .mark_failed(
                &event.id,
                token,
                event.attempts,
                event.max_attempts,
                failure,
                error,
            )
            .await?)
    }
}
