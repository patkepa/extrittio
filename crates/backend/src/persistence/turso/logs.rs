use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::params;

use crate::domains::logs::port::LogRepository;
use crate::domains::logs::types::{LogQuery, LogRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

use super::{TursoAdapter, row};

#[async_trait]
impl LogRepository for TursoAdapter {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        level: String,
        message: String,
    ) -> Result<bool, PersistenceError> {
        let writer = self.database.writer().await;
        writer
            .execute(
                "INSERT INTO device_logs (tenant_id, device_id, level, message, created_at)
             SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS
               (SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
                params![
                    identity.tenant_id_str(),
                    identity.device_id(),
                    level,
                    message,
                    chrono::Utc::now().timestamp_micros()
                ],
            )
            .await
            .map(|count| count > 0)
            .map_err(row::error)
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: LogQuery,
    ) -> Result<Option<Vec<LogRecord>>, PersistenceError> {
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
        let mut rows = connection
            .query(
                "SELECT id, device_id, level, message, created_at FROM device_logs
             WHERE tenant_id = ?1 AND device_id = ?2 AND (?3 IS NULL OR level = ?3)
               AND (?4 IS NULL OR created_at >= ?4)
             ORDER BY created_at DESC, id DESC LIMIT ?5",
                params![
                    tenant.as_str(),
                    device_id,
                    query.level,
                    query.since.map(|value| value.and_utc().timestamp_micros()),
                    query.limit
                ],
            )
            .await
            .map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(LogRecord {
                id: record.get(0).map_err(row::error)?,
                device_id: record.get(1).map_err(row::error)?,
                level: record.get(2).map_err(row::error)?,
                message: record.get(3).map_err(row::error)?,
                created_at: row::datetime(record.get(4).map_err(row::error)?)?.naive_utc(),
            });
        }
        Ok(Some(records))
    }

    async fn delete_older_than(&self, cutoff: NaiveDateTime) -> Result<usize, PersistenceError> {
        let writer = self.database.writer().await;
        writer
            .execute(
                "DELETE FROM device_logs WHERE created_at < ?1",
                params![cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map(|count| count as usize)
            .map_err(row::error)
    }
}
