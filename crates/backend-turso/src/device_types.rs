use async_trait::async_trait;
use chrono::Utc;
use turso::{Row, params};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::device_types::DeviceTypeRepository;
use extrittio_backend_core::device_types::{
    CreateDeviceTypeRecord, DeleteDeviceTypeOutcome, DeviceTypeList, DeviceTypeRecord,
    UpdateDeviceTypeRecord,
};

use crate::row::legacy_error as map_error;
use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoDeviceTypeRepository {
    handles: TursoConnectionHandles,
}
impl TursoDeviceTypeRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

fn decode(record: &Row) -> Result<DeviceTypeRecord, PersistenceError> {
    Ok(DeviceTypeRecord {
        id: row::i32(record.get::<i64>(0).map_err(map_error)?, "device_types.id")?,
        name: record.get(1).map_err(map_error)?,
        icon: record.get(2).map_err(map_error)?,
        color_hex: record.get(3).map_err(map_error)?,
    })
}

#[async_trait]
impl DeviceTypeRepository for TursoDeviceTypeRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<DeviceTypeList, PersistenceError> {
        let connection = self.connect()?;
        let mut count_rows = connection
            .query(
                "SELECT count(*) FROM device_types WHERE tenant_id = ?1",
                params![tenant.as_str()],
            )
            .await
            .map_err(map_error)?;
        let total = count_rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?
            .get(0)
            .map_err(map_error)?;
        let mut rows = connection
            .query(
                "SELECT id, name, icon, color_hex FROM device_types
                 WHERE tenant_id = ?1 ORDER BY name COLLATE BINARY, id LIMIT ?2 OFFSET ?3",
                params![tenant.as_str(), limit, offset],
            )
            .await
            .map_err(map_error)?;
        let mut records = Vec::new();
        while let Some(record) = rows.next().await.map_err(map_error)? {
            records.push(decode(&record)?);
        }
        Ok(DeviceTypeList { records, total })
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceTypeRecord,
    ) -> Result<DeviceTypeRecord, PersistenceError> {
        let writer = self.handles.lock_writer().await;
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
            .map_err(map_error)?;
        decode(
            &rows
                .next()
                .await
                .map_err(map_error)?
                .ok_or(PersistenceError::NotFound)?,
        )
    }

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateDeviceTypeRecord,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let writer = self.handles.lock_writer().await;
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
            .map_err(map_error)?;
        rows.next()
            .await
            .map_err(map_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn get_by_id(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT id, name, icon, color_hex FROM device_types WHERE tenant_id = ?1 AND id = ?2",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map_err(map_error)?;
        rows.next()
            .await
            .map_err(map_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn get_by_name(
        &self,
        tenant: &TenantId,
        name: &str,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT id, name, icon, color_hex FROM device_types WHERE tenant_id = ?1 AND name = ?2",
                params![tenant.as_str(), name],
            )
            .await
            .map_err(map_error)?;
        rows.next()
            .await
            .map_err(map_error)?
            .map(|record| decode(&record))
            .transpose()
    }

    async fn delete_if_unused(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteDeviceTypeOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(map_error)?;
        let mut rows = transaction
            .query(
                "SELECT EXISTS(SELECT 1 FROM device_types WHERE tenant_id = ?1 AND id = ?2),
                        (SELECT count(*) FROM devices WHERE tenant_id = ?1 AND device_type_id = ?2)",
                params![tenant.as_str(), i64::from(id)],
            )
            .await
            .map_err(map_error)?;
        let result = rows
            .next()
            .await
            .map_err(map_error)?
            .ok_or(PersistenceError::NotFound)?;
        let exists = result.get::<i64>(0).map_err(map_error)? != 0;
        let device_count = result.get::<i64>(1).map_err(map_error)?;
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
                .map_err(map_error)?;
            DeleteDeviceTypeOutcome::Deleted
        };
        transaction.commit().await.map_err(map_error)?;
        Ok(outcome)
    }
}
