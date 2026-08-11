use async_trait::async_trait;
use diesel::OptionalExtension;
use diesel::prelude::*;

use crate::db::models::{DeviceLog, NewDeviceLog};
use crate::db::schema::{device_logs, devices};
use crate::domains::logs::port::LogRepository;
use crate::domains::logs::types::{LogQuery, LogRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::DeviceIdentity;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

#[async_trait]
impl LogRepository for PostgresAdapter {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        level: String,
        message: String,
    ) -> Result<bool, PersistenceError> {
        let tenant_id = identity.tenant_id_str().to_string();
        let device_id = identity.device_id().to_string();
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
                    return Ok(false);
                }

                diesel::insert_into(device_logs::table)
                    .values(NewDeviceLog {
                        tenant_id,
                        device_id,
                        level,
                        message,
                    })
                    .execute(connection)
                    .map_err(map_diesel_error)?;
                Ok(true)
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
                    statement = statement.filter(device_logs::created_at.gt(since));
                }
                let records = statement
                    .order(device_logs::created_at.desc())
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
