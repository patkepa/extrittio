use async_trait::async_trait;
use chrono::Utc;
use turso::params;

use crate::domains::audit::port::AuditRepository;
use crate::domains::audit::types::{AuditEventRecord, NewAuditEventRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

#[async_trait]
impl AuditRepository for TursoAdapter {
    async fn record(
        &self,
        tenant: &TenantId,
        event: NewAuditEventRecord,
    ) -> Result<(), PersistenceError> {
        let writer = self.database.writer().await;
        writer.execute("INSERT INTO audit_events (id, tenant_id, actor_type, actor_id, action, resource_type, resource_id, outcome, request_id, metadata, occurred_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)", params![event.id, tenant.as_str(), event.actor_type, event.actor_id, event.action, event.resource_type, event.resource_id, event.outcome, event.request_id, serde_json::to_string(&event.metadata).map_err(|error| PersistenceError::Internal(error.to_string()))?, Utc::now().timestamp_micros()]).await.map(|_| ()).map_err(row::error)
    }

    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEventRecord>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection.query("SELECT id, actor_type, actor_id, action, resource_type, resource_id, outcome, request_id, metadata, occurred_at FROM audit_events WHERE tenant_id = ?1 ORDER BY occurred_at DESC, id DESC LIMIT ?2 OFFSET ?3", params![tenant.as_str(), limit, offset]).await.map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            let metadata: String = record.get(8).map_err(row::error)?;
            records.push(AuditEventRecord {
                id: record.get(0).map_err(row::error)?,
                actor_type: record.get(1).map_err(row::error)?,
                actor_id: record.get(2).map_err(row::error)?,
                action: record.get(3).map_err(row::error)?,
                resource_type: record.get(4).map_err(row::error)?,
                resource_id: record.get(5).map_err(row::error)?,
                outcome: record.get(6).map_err(row::error)?,
                request_id: record.get(7).map_err(row::error)?,
                metadata: serde_json::from_str(&metadata)
                    .map_err(|error| PersistenceError::CorruptData(error.to_string()))?,
                occurred_at: row::datetime(record.get(9).map_err(row::error)?)?.naive_utc(),
            });
        }
        Ok(records)
    }
}
