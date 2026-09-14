use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use turso::{Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::commands::CommandRepository;
use extrittio_backend_core::commands::{CommandQuery, CommandRecord, NewCommandRecord};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoCommandRepository {
    handles: TursoConnectionHandles,
}
impl TursoCommandRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|e| PersistenceError::Unavailable(e.to_string()))
    }
}
/// Read historical edge states without rewriting stored rows.
fn canonical_status(status: String) -> String {
    match status.as_str() {
        "pending" => "sent".into(),
        "completed" => "succeeded".into(),
        "timeout" => "timed_out".into(),
        _ => status,
    }
}

fn decode(record: &Row) -> Result<CommandRecord, PersistenceError> {
    Ok(CommandRecord {
        id: record.get(0).map_err(row::legacy_error)?,
        device_id: record.get(1).map_err(row::legacy_error)?,
        command: record.get(2).map_err(row::legacy_error)?,
        params: record.get(3).map_err(row::legacy_error)?,
        status: canonical_status(record.get(4).map_err(row::legacy_error)?),
        response_payload: record.get(5).map_err(row::legacy_error)?,
        created_at: row::datetime(record.get(6).map_err(row::legacy_error)?)?.naive_utc(),
        updated_at: row::datetime(record.get(7).map_err(row::legacy_error)?)?.naive_utc(),
    })
}

#[async_trait]
impl CommandRepository for TursoCommandRepository {
    async fn find(
        &self,
        tenant: &TenantId,
        id: &str,
    ) -> Result<Option<CommandRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection.query("SELECT id, device_id, command, params, status, response_payload, created_at, updated_at FROM command_history WHERE tenant_id = ?1 AND id = ?2", params![tenant.as_str(), id]).await.map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|row| decode(&row))
            .transpose()
    }

    async fn create(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: NewCommandRecord,
    ) -> Result<Option<CommandRecord>, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        let now = Utc::now().timestamp_micros();
        let mut rows = writer.query("INSERT INTO command_history (id, tenant_id, device_id, command, params, status, created_at, updated_at) SELECT ?1, ?2, ?3, ?4, ?5, 'sent', ?6, ?6 WHERE EXISTS (SELECT 1 FROM devices WHERE tenant_id = ?2 AND id = ?3) RETURNING id, device_id, command, params, status, response_payload, created_at, updated_at", params![record.id, tenant.as_str(), device_id, record.command, record.params, now]).await.map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: CommandQuery,
    ) -> Result<Option<Vec<CommandRecord>>, PersistenceError> {
        let connection = self.connect()?;
        let mut exists = connection
            .query(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::legacy_error)?;
        if exists
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::legacy_error)?
            == 0
        {
            return Ok(None);
        }
        let mut rows = connection.query("SELECT id, device_id, command, params, status, response_payload, created_at, updated_at FROM command_history WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR CASE status WHEN 'pending' THEN 'sent' WHEN 'completed' THEN 'succeeded' WHEN 'timeout' THEN 'timed_out' ELSE status END = ?3) ORDER BY created_at DESC, id DESC LIMIT ?4", params![tenant.as_str(), device_id, query.status.map(canonical_status), query.limit]).await.map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            records.push(decode(&record)?);
        }
        Ok(Some(records))
    }

    async fn apply_response(
        &self,
        tenant: &TenantId,
        device_id: &str,
        correlation_id: String,
        status: extrittio_backend_core::commands::CommandResponseStatus,
        payload: Option<String>,
        updated_at: chrono::NaiveDateTime,
    ) -> Result<Option<String>, PersistenceError> {
        let status = status.as_str();
        let writer = self.handles.lock_writer().await;
        let mut rows = writer.query("UPDATE command_history SET status = ?4, response_payload = ?5, updated_at = ?6 WHERE tenant_id = ?1 AND device_id = ?2 AND id = ?3 AND status IN ('pending', 'sent', 'delivered') RETURNING status", params![tenant.as_str(), device_id, correlation_id, status, payload, updated_at.and_utc().timestamp_micros()]).await.map_err(row::legacy_error)?;
        rows.next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| record.get(0).map_err(row::legacy_error))
            .transpose()
    }

    async fn timeout_stale(
        &self,
        cutoff: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer.execute("UPDATE command_history SET status = 'timed_out', updated_at = ?2 WHERE status IN ('pending', 'sent', 'delivered') AND created_at < ?1", params![cutoff.and_utc().timestamp_micros(), now.and_utc().timestamp_micros()]).await.map(|count| count as usize).map_err(row::legacy_error)
    }
}
