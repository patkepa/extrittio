use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::Connection;
use diesel::prelude::*;
use serde_json::{Map, Value};

use crate::models::{DeviceConfig, NewDeviceConfig};
use crate::schema::{device_configs, devices};
use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::configuration::DeviceConfigRepository;
use extrittio_backend_core::configuration::{
    DeviceConfigRecord, GetDeviceConfigOutcome, MergeDeviceConfigOutcome, merge_config,
};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresConfigurationRepository {
    executor: PostgresExecutor,
}
impl PostgresConfigurationRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

fn to_record(row: DeviceConfig) -> DeviceConfigRecord {
    DeviceConfigRecord {
        device_id: row.device_id,
        config: row.config,
        updated_at: row.updated_at.and_utc(),
    }
}

#[async_trait]
impl DeviceConfigRepository for PostgresConfigurationRepository {
    async fn get_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<GetDeviceConfigOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                let result = devices::table
                    .left_outer_join(device_configs::table)
                    .filter(devices::tenant_id.eq(tenant_id))
                    .filter(devices::id.eq(&device_id))
                    .select((
                        device_configs::config.nullable(),
                        device_configs::updated_at.nullable(),
                    ))
                    .first::<(Option<Value>, Option<chrono::NaiveDateTime>)>(connection)
                    .optional()
                    .map_err(map_diesel_error)?;

                Ok(match result {
                    None => GetDeviceConfigOutcome::DeviceNotFound,
                    Some((None, None)) => GetDeviceConfigOutcome::Found(None),
                    Some((Some(config), Some(updated_at))) => {
                        GetDeviceConfigOutcome::Found(Some(DeviceConfigRecord {
                            device_id,
                            config,
                            updated_at: updated_at.and_utc(),
                        }))
                    }
                    Some(_) => {
                        return Err(PersistenceError::CorruptData(
                            "device configuration join returned partial data".to_string(),
                        ));
                    }
                })
            })
            .await
    }

    async fn merge_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<MergeDeviceConfigOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let device_exists = devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&device_id))
                            .select(devices::id)
                            .for_update()
                            .first::<String>(connection)
                            .optional()?;
                        if device_exists.is_none() {
                            return Ok(MergeDeviceConfigOutcome::DeviceNotFound);
                        }

                        let current = device_configs::table
                            .filter(device_configs::tenant_id.eq(&tenant_id))
                            .filter(device_configs::device_id.eq(&device_id))
                            .select(device_configs::config)
                            .first::<Value>(connection)
                            .optional()?
                            .unwrap_or_else(|| Value::Object(Map::default()));
                        let config = merge_config(current, &patch);

                        let row = diesel::insert_into(device_configs::table)
                            .values(NewDeviceConfig {
                                device_id: device_id.clone(),
                                tenant_id: tenant_id.clone(),
                                config: config.clone(),
                            })
                            .on_conflict(device_configs::device_id)
                            .do_update()
                            .set((
                                device_configs::config.eq(config),
                                device_configs::updated_at.eq(updated_at.naive_utc()),
                            ))
                            .returning(DeviceConfig::as_returning())
                            .get_result::<DeviceConfig>(connection)?;
                        Ok(MergeDeviceConfigOutcome::Updated(to_record(row)))
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
