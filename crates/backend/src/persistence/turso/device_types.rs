use async_trait::async_trait;
use chrono::Utc;
use turso::{Row, params};

use crate::domains::device_types::repository::DeviceTypeRepository;
use crate::domains::device_types::types::{
    CreateDeviceTypeRecord, DeleteDeviceTypeOutcome, DeviceTypeList, DeviceTypeRecord,
    UpdateDeviceTypeRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::TursoAdapter;
use super::row;

fn decode(record: &Row) -> Result<DeviceTypeRecord, PersistenceError> {
    Ok(DeviceTypeRecord {
        id: row::i32(record.get::<i64>(0).map_err(row::error)?, "device_types.id")?,
        name: record.get(1).map_err(row::error)?,
        icon: record.get(2).map_err(row::error)?,
        color_hex: record.get(3).map_err(row::error)?,
    })
}

#[async_trait]
impl DeviceTypeRepository for TursoAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<DeviceTypeList, PersistenceError> {
        let connection = self.database.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM device_types WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(row::error)?;
        let total = count_rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(row::error)?;
        let mut rows = connection
            .query(
                "SELECT id, name, icon, color_hex FROM device_types
                 WHERE tenant_id = ?1 ORDER BY name, id LIMIT ?2 OFFSET ?3",
                params![tenant.as_str(), limit, offset],
            )
            .await
            .map_err(row::error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            records.push(decode(&record)?);
        }
        Ok(DeviceTypeList { records, total })
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceTypeRecord,
    ) -> Result<DeviceTypeRecord, PersistenceError> {
        let writer = self.database.writer().await;
        let mut rows = writer
            .query(
                "INSERT INTO device_types (tenant_id, name, icon, color_hex, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id, name, icon, color_hex",
                params![
                    tenant.as_str(),
                    record.name,
                    record.icon,
                    record.color_hex,
                    Utc::now().timestamp_micros()
                ],
            )
            .await
            .map_err(row::error)?;
        decode(
            &rows
                .next()
                .await
                .map_err(row::error)?
                .ok_or(PersistenceError::NotFound)?,
        )
    }

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateDeviceTypeRecord,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let writer = self.database.writer().await;
        let mut rows = writer
            .query(
                "UPDATE device_types SET
                   name = COALESCE(?3, name), icon = COALESCE(?4, icon),
                   color_hex = COALESCE(?5, color_hex)
                 WHERE tenant_id = ?1 AND id = ?2 RETURNING id, name, icon, color_hex",
                params![
                    tenant.as_str(),
                    i64::from(id),
                    record.name,
                    record.icon,
                    record.color_hex
                ],
            )
            .await
            .map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn get_by_id(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let connection = self.database.connect()?;
        let mut rows = connection
            .query(
                "SELECT id, name, icon, color_hex FROM device_types WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map_err(row::error)?;
        rows.next()
            .await
            .map_err(row::error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn delete_if_unused(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteDeviceTypeOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
        let mut rows = transaction
            .query(
                "SELECT EXISTS(SELECT 1 FROM device_types WHERE tenant_id = ?1 AND id = ?2),
                        (SELECT count(*) FROM devices WHERE tenant_id = ?1 AND device_type_id = ?2)",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map_err(row::error)?;
        let result = rows
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?;
        let exists = result.get::<i64>(0).map_err(row::error)? != 0;
        let device_count = result.get::<i64>(1).map_err(row::error)?;
        drop(rows);
        let outcome = if !exists {
            DeleteDeviceTypeOutcome::NotFound
        } else if device_count > 0 {
            DeleteDeviceTypeOutcome::InUse { device_count }
        } else {
            transaction
                .execute(
                    "DELETE FROM device_types WHERE tenant_id = ?1 AND id = ?2",
                    params![tenant.as_str(), i64::from(id)],
                )
                .await
                .map_err(row::error)?;
            DeleteDeviceTypeOutcome::Deleted
        };
        transaction.commit().await.map_err(row::error)?;
        Ok(outcome)
    }
}
