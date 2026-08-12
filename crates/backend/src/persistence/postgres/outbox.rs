use std::time::Duration;

use async_trait::async_trait;
use diesel::{Connection, PgConnection};

use crate::db::models::{NewRuleActionOutboxEvent, RuleActionOutboxEvent};
use crate::domains::operations::outbox_repository::OutboxRepository;
use crate::domains::operations::outbox_types::{OutboxEventRecord, OutboxSummaryRecord};
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::repositories::rule_action_outbox_repo;
use crate::rule_engine::actions::outbox_event_for_action;
use crate::rule_engine::types::PendingAction;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn event_record(event: RuleActionOutboxEvent) -> OutboxEventRecord {
    OutboxEventRecord {
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

pub(super) fn enqueue_pending_actions(
    connection: &mut PgConnection,
    actions: &[PendingAction],
) -> Result<usize, AppError> {
    connection.transaction(|connection| {
        let mut inserted = 0;
        for action in actions {
            let event = outbox_event_for_action(action)?;
            inserted += rule_action_outbox_repo::insert_event(
                connection,
                &NewRuleActionOutboxEvent {
                    id: event.id,
                    tenant_id: event.tenant_id,
                    event_type: event.event_type,
                    aggregate_type: event.aggregate_type,
                    aggregate_id: event.aggregate_id,
                    idempotency_key: event.idempotency_key,
                    payload: event.payload,
                },
            )?;
        }
        Ok(inserted)
    })
}

#[async_trait]
impl OutboxRepository for PostgresAdapter {
    async fn claim_batch(
        &self,
        worker_id: &str,
        limit: i64,
        lease_timeout: Duration,
    ) -> Result<Vec<OutboxEventRecord>, PersistenceError> {
        let worker_id = worker_id.to_string();
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
        worker_id: &str,
    ) -> Result<bool, PersistenceError> {
        let event_id = event_id.to_string();
        let worker_id = worker_id.to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::mark_succeeded(connection, &event_id, &worker_id)
                    .map(|rows| rows == 1)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn mark_failed(
        &self,
        event_id: &str,
        worker_id: &str,
        attempts: i32,
        max_attempts: i32,
        error: &str,
    ) -> Result<bool, PersistenceError> {
        let event_id = event_id.to_string();
        let worker_id = worker_id.to_string();
        let error = error.to_string();
        self.executor
            .run(move |connection| {
                rule_action_outbox_repo::mark_failed(
                    connection,
                    &event_id,
                    &worker_id,
                    attempts,
                    max_attempts,
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
