use std::time::Duration;

use async_trait::async_trait;

use crate::models::RuleActionOutboxEvent;
use crate::outbox_sql as rule_action_outbox_repo;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::outbox::OutboxRepository;
use extrittio_backend_core::outbox::{OutboxEventRecord, OutboxSummaryRecord};

use crate::error::map_diesel_error;
use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresOutboxRepository {
    executor: PostgresExecutor,
}
impl PostgresOutboxRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}

fn event_record(event: RuleActionOutboxEvent) -> OutboxEventRecord {
    OutboxEventRecord {
        claim_token: event.locked_by,
        id: event.id,
        tenant_id: event.tenant_id,
        event_type: event.event_type,
        aggregate_type: event.aggregate_type,
        aggregate_id: event.aggregate_id,
        payload: event.payload,
        attempts: event.attempts,
        max_attempts: event.max_attempts,
        last_error: event.last_error,
        created_at: event.created_at,
        updated_at: event.updated_at,
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_timeout: Duration,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError> {
        let worker_id = format!("{worker_id}:{}", uuid::Uuid::new_v4());
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::claim_batch(connection, &worker_id, limit, lease_timeout)
                    .map(|events| events.into_iter().map(event_record).collect())
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn mark_succeeded(
        &self,
        event_id: &str,
        claim_token: &str,
    ) -> Result<bool, PersistenceError> {
        let event_id = event_id.to_string();
        let claim_token = claim_token.to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::mark_succeeded(connection, &event_id, &claim_token)
                    .map(|rows| rows == 1)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn mark_failed(
        &self,
        event_id: &str,
        claim_token: &str,
        attempts: i32,
        max_attempts: i32,
        failure: extrittio_backend_core::outbox::FailureClass,
        error: &str,
    ) -> Result<bool, PersistenceError> {
        let event_id = event_id.to_string();
        let claim_token = claim_token.to_string();
        let error = error.to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::mark_failed(
                    connection,
                    &event_id,
                    &claim_token,
                    attempts,
                    max_attempts,
                    failure,
                    &error,
                )
                .map(|rows| rows == 1)
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn summary(&self, tenant: &TenantId) -> Result<OutboxSummaryRecord, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::summarize_for_tenant(connection, &tenant_id)
                    .map(|summary| OutboxSummaryRecord {
                        pending_count: summary.pending_count,
                        processing_count: summary.processing_count,
                        failed_count: summary.failed_count,
                        dead_letter_count: summary.dead_letter_count,
                        succeeded_count: summary.succeeded_count,
                        oldest_pending_at: summary.oldest_pending_at,
                        oldest_pending_age_seconds: summary.oldest_pending_age_seconds,
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn list_dead_letters(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::list_dead_letters(connection, &tenant_id, limit, offset)
                    .map(|events| events.into_iter().map(event_record).collect())
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn replay_dead_letters(
        &self,
        tenant: &TenantId,
        event_ids: Option<Vec<String>>,
    ) -> Result<usize, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::replay_dead_letters(
                    connection,
                    &tenant_id,
                    event_ids.as_deref(),
                )
                .map_err(map_diesel_error)
            })
            .await
    }
}

/// Participate in the caller's ingress transaction; never commits independently.
pub fn enqueue_actions_in_transaction(
    connection: &mut diesel::PgConnection,
    actions: &[extrittio_backend_core::rule_engine::types::PendingAction],
) -> Result<usize, PersistenceError> {
    use diesel::prelude::*;
    let mut inserted = 0;
    for action in actions {
        let event = extrittio_backend_core::rule_actions::outbox_event_for_action(action)
            .map_err(|e| PersistenceError::Internal(e.to_string()))?;
        inserted += diesel::insert_into(crate::schema::rule_action_outbox::table)
            .values(crate::models::NewRuleActionOutboxEvent {
                id: event.id,
                tenant_id: event.tenant_id,
                event_type: event.event_type,
                aggregate_type: event.aggregate_type,
                aggregate_id: event.aggregate_id,
                idempotency_key: event.idempotency_key,
                payload: event.payload,
            })
            .on_conflict_do_nothing()
            .execute(connection)
            .map_err(map_diesel_error)?;
    }
    Ok(inserted)
}
