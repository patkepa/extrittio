use async_trait::async_trait;

use crate::db::models::NewAuditEvent;
use crate::domains::audit::port::AuditRepository;
use crate::domains::audit::types::{AuditEventRecord, NewAuditEventRecord};
use crate::persistence::PersistenceError;
use crate::repositories::audit_repo;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

#[async_trait]
impl AuditRepository for PostgresAdapter {
    async fn record(
        &self,
        tenant: &TenantId,
        event: NewAuditEventRecord,
    ) -> Result<(), PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                audit_repo::insert(
                    connection,
                    &NewAuditEvent {
                        id: event.id,
                        tenant_id,
                        actor_type: event.actor_type,
                        actor_id: event.actor_id,
                        action: event.action,
                        resource_type: event.resource_type,
                        resource_id: event.resource_id,
                        outcome: event.outcome,
                        request_id: event.request_id,
                        metadata: event.metadata,
                    },
                )
                .map(|_| ())
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEventRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        self.executor
            .run(move |connection| {
                audit_repo::list(connection, &tenant_id, limit, offset)
                    .map(|events| {
                        events
                            .into_iter()
                            .map(|event| AuditEventRecord {
                                id: event.id,
                                actor_type: event.actor_type,
                                actor_id: event.actor_id,
                                action: event.action,
                                resource_type: event.resource_type,
                                resource_id: event.resource_id,
                                outcome: event.outcome,
                                request_id: event.request_id,
                                metadata: event.metadata,
                                occurred_at: event.occurred_at,
                            })
                            .collect()
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
