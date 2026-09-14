use async_trait::async_trait;

use crate::audit_sql as audit_repo;
use crate::models::NewAuditEvent;
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::audit::AuditRepository;
use extrittio_backend_core::audit::{AuditEventRecord, NewAuditEventRecord};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresAuditRepository {
    executor: PostgresExecutor,
}
impl PostgresAuditRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

#[async_trait]
impl AuditRepository for PostgresAuditRepository {
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
