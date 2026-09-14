use async_trait::async_trait;
use chrono::NaiveDateTime;
use turso::params;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::logs::LogRepository;
use extrittio_backend_core::logs::{LogQuery, LogRecord};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoLogRepository {
    handles: TursoConnectionHandles,
}
impl TursoLogRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

#[async_trait]
impl LogRepository for TursoLogRepository {
    async fn record(
        &self,
        tenant: &TenantId,
        device_id: &str,
        level: String,
        message: String,
        observed_at: chrono::NaiveDateTime,
    ) -> Result<bool, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "INSERT INTO device_logs (tenant_id, device_id, level, message, created_at)
             SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS
               (SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
                params![
                    tenant.as_str(),
                    device_id,
                    level,
                    message,
                    observed_at.and_utc().timestamp_micros()
                ],
            )
            .await
            .map(|count| count > 0)
            .map_err(row::legacy_error)
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: LogQuery,
    ) -> Result<Option<Vec<LogRecord>>, PersistenceError> {
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
            .map_err(row::legacy_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            records.push(LogRecord {
                id: record.get(0).map_err(row::legacy_error)?,
                device_id: record.get(1).map_err(row::legacy_error)?,
                level: record.get(2).map_err(row::legacy_error)?,
                message: record.get(3).map_err(row::legacy_error)?,
                created_at: row::datetime(record.get(4).map_err(row::legacy_error)?)?.naive_utc(),
            });
        }
        Ok(Some(records))
    }

    async fn delete_older_than(&self, cutoff: NaiveDateTime) -> Result<usize, PersistenceError> {
        let writer = self.handles.lock_writer().await;
        writer
            .execute(
                "DELETE FROM device_logs WHERE created_at < ?1",
                params![cutoff.and_utc().timestamp_micros()],
            )
            .await
            .map(|count| count as usize)
            .map_err(row::legacy_error)
    }
}
