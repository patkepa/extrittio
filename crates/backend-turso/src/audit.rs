use async_trait::async_trait;
use chrono::Utc;
use turso::params;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::audit::AuditRepository;
use extrittio_backend_core::audit::{AuditEventRecord, NewAuditEventRecord};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoAuditRepository {
    handles: TursoConnectionHandles,
}
impl TursoAuditRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

#[async_trait]
impl AuditRepository for TursoAuditRepository {
    async fn record(
        &self,
        tenant: &TenantId,
        event: NewAuditEventRecord,
    ) -> Result<(), PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer.execute("INSERT INTO audit_events (id, tenant_id, actor_type, actor_id, action, resource_type, resource_id, outcome, request_id, metadata, occurred_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![event.id, tenant.as_str(), event.actor_type, event.actor_id, event.action, event.resource_type, event.resource_id, event.outcome, event.request_id, serde_json::to_string(&event.metadata).map_err(|error| PersistenceError::Internal(error.to_string()))?, Utc::now().timestamp_micros()]).await.map(|_| ()).map_err(row::legacy_error)
    }

    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEventRecord>, PersistenceError> {
        let connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let mut rows = connection.query("SELECT id, actor_type, actor_id, action, resource_type, resource_id, outcome, request_id, metadata, occurred_at FROM audit_events WHERE tenant_id = ?1 ORDER BY occurred_at DESC, id COLLATE BINARY DESC LIMIT ?2 OFFSET ?3", params![tenant.as_str(), limit, offset]).await.map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            let metadata: String = record.get(8).map_err(row::legacy_error)?;
            records.push(AuditEventRecord {
                id: record.get(0).map_err(row::legacy_error)?,
                actor_type: record.get(1).map_err(row::legacy_error)?,
                actor_id: record.get(2).map_err(row::legacy_error)?,
                action: record.get(3).map_err(row::legacy_error)?,
                resource_type: record.get(4).map_err(row::legacy_error)?,
                resource_id: record.get(5).map_err(row::legacy_error)?,
                outcome: record.get(6).map_err(row::legacy_error)?,
                request_id: record.get(7).map_err(row::legacy_error)?,
                metadata: serde_json::from_str(&metadata)
                    .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
                occurred_at: row::datetime(record.get(9).map_err(row::legacy_error)?)?.naive_utc(),
            });
        }
        Ok(records)
    }
}
