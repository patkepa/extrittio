use async_trait::async_trait;
use diesel::OptionalExtension;
use diesel::prelude::*;

use crate::models::DeviceLog;
use crate::schema::{device_logs, devices};
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::logs::LogRepository;
use extrittio_backend_core::logs::{LogQuery, LogRecord};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresLogRepository {
    executor: PostgresExecutor,
}
impl PostgresLogRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

#[async_trait]
impl LogRepository for PostgresLogRepository {
    async fn record(
        &self,
        tenant: &TenantId,
        device_id: &str,
        level: String,
        message: String,
        observed_at: chrono::NaiveDateTime,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                use diesel::sql_types::{Text, Timestamp};
                diesel::sql_query("INSERT INTO device_logs (tenant_id, device_id, level, message, created_at) SELECT tenant_id, id, $3, $4, $5 FROM devices WHERE tenant_id = $1 AND id = $2 FOR KEY SHARE")
                    .bind::<Text,_>(&tenant_id).bind::<Text,_>(&device_id).bind::<Text,_>(&level).bind::<Text,_>(&message).bind::<Timestamp,_>(observed_at)
                    .execute(connection).map(|count| count == 1).map_err(map_diesel_error)
            })
            .await
    }

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: LogQuery,
    ) -> Result<Option<Vec<LogRecord>>, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let device_id = device_id.to_string();
        self.executor
            .run(move |connection| {
                let exists = devices::table
                    .filter(devices::tenant_id.eq(&tenant_id))
                    .filter(devices::id.eq(&device_id))
                    .select(devices::id)
                    .first::<String>(connection)
                    .optional()
                    .map_err(map_diesel_error)?
                    .is_some();
                if !exists {
                    return Ok(None);
                }
                let mut statement = device_logs::table
                    .filter(device_logs::tenant_id.eq(&tenant_id))
                    .filter(device_logs::device_id.eq(&device_id))
                    .into_boxed();
                if let Some(level) = query.level {
                    statement = statement.filter(device_logs::level.eq(level));
                }
                if let Some(since) = query.since {
                    statement = statement.filter(device_logs::created_at.ge(since));
                }
                let records = statement
                    .order((device_logs::created_at.desc(), device_logs::id.desc()))
                    .limit(query.limit)
                    .select(DeviceLog::as_select())
                    .load::<DeviceLog>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(|record| LogRecord {
                        id: record.id,
                        device_id: record.device_id,
                        level: record.level,
                        message: record.message,
                        created_at: record.created_at,
                    })
                    .collect();
                Ok(Some(records))
            })
            .await
    }

    async fn delete_older_than(
        &self,
        cutoff: chrono::NaiveDateTime,
    ) -> Result<usize, PersistenceError> {
        self.executor
            .run(move |connection| {
                diesel::delete(device_logs::table.filter(device_logs::created_at.lt(cutoff)))
                    .execute(connection)
                    .map_err(map_diesel_error)
            })
            .await
    }
}
