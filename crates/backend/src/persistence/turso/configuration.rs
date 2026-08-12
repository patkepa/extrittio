use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use turso::params;

use crate::domains::configuration::repository::DeviceConfigRepository;
use crate::domains::configuration::types::{
    DeviceConfigRecord, GetDeviceConfigOutcome, MergeDeviceConfigOutcome, merge_config,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

#[async_trait]
impl DeviceConfigRepository for TursoAdapter {
    async fn get_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<GetDeviceConfigOutcome, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                "SELECT c.config, c.updated_at FROM devices d
                 LEFT JOIN device_configs c ON c.tenant_id = d.tenant_id AND c.device_id = d.id
                 WHERE d.tenant_id = ?1 AND d.id = ?2",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::error)?;
        let Some(record) = rows.next().await.map_err(row::error)? else {
            return Ok(GetDeviceConfigOutcome::DeviceNotFound);
        };
        let config_text: Option<String> = record.get(0).map_err(row::error)?;
        let updated_at: Option<i64> = record.get(1).map_err(row::error)?;
        match (config_text, updated_at) {
            (None, None) => Ok(GetDeviceConfigOutcome::Found(None)),
            (Some(config), Some(updated_at)) => {
                Ok(GetDeviceConfigOutcome::Found(Some(DeviceConfigRecord {
                    device_id: device_id.to_string(),
                    config: serde_json::from_str(&config).map_err(|error| {
                        PersistenceError::CorruptData(format!(
                            "invalid device config JSON: {error}"
                        ))
                    })?,
                    updated_at: row::datetime(updated_at)?,
                })))
            }
            _ => Err(PersistenceError::CorruptData(
                "device configuration join returned partial data".to_string(),
            )),
        }
    }

    async fn merge_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<MergeDeviceConfigOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let mut rows = transaction
            .query(
                "SELECT c.config FROM devices d LEFT JOIN device_configs c
                   ON c.tenant_id = d.tenant_id AND c.device_id = d.id
                 WHERE d.tenant_id = ?1 AND d.id = ?2",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::error)?;
        let Some(record) = rows.next().await.map_err(row::error)? else {
            transaction.rollback().await.map_err(row::error)?;
            return Ok(MergeDeviceConfigOutcome::DeviceNotFound);
        };
        let current = record
            .get::<Option<String>>(0)
            .map_err(row::error)?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| PersistenceError::CorruptData(error.to_string()))?
            .unwrap_or_else(|| Value::Object(Map::new()));
        drop(rows);
        let config = merge_config(current, &patch);
        let encoded = serde_json::to_string(&config)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO device_configs (tenant_id, device_id, config, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (tenant_id, device_id) DO UPDATE
                 SET config = excluded.config, updated_at = excluded.updated_at",
                params![
                    tenant.as_str(),
                    device_id,
                    encoded,
                    updated_at.timestamp_micros()
                ],
            )
            .await
            .map_err(row::error)?;
        transaction.commit().await.map_err(row::error)?;
        Ok(MergeDeviceConfigOutcome::Updated(DeviceConfigRecord {
            device_id: device_id.to_string(),
            config,
            updated_at,
        }))
    }
}
