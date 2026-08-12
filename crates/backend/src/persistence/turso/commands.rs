use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use turso::{Row, params};

use crate::domains::commands::port::CommandRepository;
use crate::domains::commands::types::{CommandQuery, CommandRecord, NewCommandRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::{TursoAdapter, row};

fn decode(record: &Row) -> Result<CommandRecord, PersistenceError> {
    Ok(CommandRecord {
        id: record.get(0).map_err(row::error)?,
        device_id: record.get(1).map_err(row::error)?,
        command: record.get(2).map_err(row::error)?,
        params: record.get(3).map_err(row::error)?,
        status: record.get(4).map_err(row::error)?,
        response_payload: record.get(5).map_err(row::error)?,
        created_at: row::datetime(record.get(6).map_err(row::error)?)?.naive_utc(),
        updated_at: row::datetime(record.get(7).map_err(row::error)?)?.naive_utc(),
    })
}

#[async_trait]
impl CommandRepository for TursoAdapter {
    async fn create(
        &self,
        tenant: &TenantId,
        device_id: &str,
        record: NewCommandRecord,
    ) -> Result<Option<CommandRecord>, PersistenceError> {
        let writer = self.database.writer().await;
        let now = Utc::now().timestamp_micros();
        let mut rows = writer.query("INSERT INTO command_history (id, tenant_id, device_id, command, params, status, created_at, updated_at) SELECT ?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6 WHERE EXISTS (SELECT 1 FROM devices WHERE tenant_id = ?2 AND id = ?3) RETURNING id, device_id, command, params, status, response_payload, created_at, updated_at", params![record.id, tenant.as_str(), device_id, record.command, record.params, now]).await.map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: CommandQuery,
    ) -> Result<Option<Vec<CommandRecord>>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut exists = connection
            .query(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::error)?;
        if exists
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::error)?
            == 0
        {
            return Ok(None);
        }
        let mut rows = connection.query("SELECT id, device_id, command, params, status, response_payload, created_at, updated_at FROM command_history WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR status = ?3) ORDER BY created_at DESC, id DESC LIMIT ?4", params![tenant.as_str(), device_id, query.status, query.limit]).await.map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(decode(&record)?);
        }
        Ok(Some(records))
    }

    async fn apply_response(
        &self,
        identity: &DeviceIdentity,
        correlation_id: String,
        device_status: String,
        payload: Option<String>,
    ) -> Result<Option<String>, PersistenceError> {
        let status = if matches!(device_status.as_str(), "failed" | "error") {
            "failed"
        } else {
            "completed"
        };
        let writer = self.database.writer().await;
        let mut rows = writer.query("UPDATE command_history SET status = ?4, response_payload = ?5, updated_at = ?6 WHERE tenant_id = ?1 AND device_id = ?2 AND id = ?3 AND status IN ('pending', 'sent') RETURNING status", params![identity.tenant_id_str(), identity.device_id(), correlation_id, status, payload, Utc::now().timestamp_micros()]).await.map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| record.get(0).map_err(row::error))
            .transpose()
    }

    async fn timeout_stale(
        &self,
        cutoff: NaiveDateTime,
        now: NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        let writer = self.database.writer().await;
        writer.execute("UPDATE command_history SET status = 'timeout', updated_at = ?2 WHERE status IN ('pending', 'sent') AND created_at < ?1", params![cutoff.and_utc().timestamp_micros(), now.and_utc().timestamp_micros()]).await.map(|count| count as usize).map_err(row::error)
    }
}
