use async_trait::async_trait;
use diesel::Connection;
use diesel::prelude::*;

use crate::db::models::{DeviceType, NewDeviceType, UpdateDeviceType};
use crate::db::schema::{device_types, devices};
use crate::domains::device_types::repository::DeviceTypeRepository;
use crate::domains::device_types::types::{
    CreateDeviceTypeRecord, DeleteDeviceTypeOutcome, DeviceTypeList, DeviceTypeRecord,
    UpdateDeviceTypeRecord,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

fn to_record(row: DeviceType) -> DeviceTypeRecord {
    DeviceTypeRecord {
        id: row.id,
        name: row.name,
        icon: row.icon,
        color_hex: row.color_hex,
    }
}

#[async_trait]
impl DeviceTypeRepository for PostgresAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<DeviceTypeList, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                let total = device_types::table
                    .filter(device_types::tenant_id.eq(&tenant_id))
                    .count()
                    .get_result(connection)
                    .map_err(map_diesel_error)?;
                let records = device_types::table
                    .filter(device_types::tenant_id.eq(&tenant_id))
                    .select(DeviceType::as_select())
                    .order((device_types::name.asc(), device_types::id.asc()))
                    .limit(limit)
                    .offset(offset)
                    .load::<DeviceType>(connection)
                    .map_err(map_diesel_error)?
                    .into_iter()
                    .map(to_record)
                    .collect();

                Ok(DeviceTypeList { records, total })
            })
            .await
    }

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateDeviceTypeRecord,
    ) -> Result<DeviceTypeRecord, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::insert_into(device_types::table)
                    .values(NewDeviceType {
                        tenant_id,
                        name: record.name,
                        icon: record.icon,
                        color_hex: record.color_hex,
                    })
                    .returning(DeviceType::as_returning())
                    .get_result(connection)
                    .map(to_record)
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn update(
        &self,
        tenant: &TenantId,
        id: i32,
        record: UpdateDeviceTypeRecord,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::update(
                    device_types::table
                        .filter(device_types::tenant_id.eq(tenant_id))
                        .filter(device_types::id.eq(id)),
                )
                .set(UpdateDeviceType {
                    name: record.name,
                    icon: record.icon,
                    color_hex: record.color_hex,
                })
                .returning(DeviceType::as_returning())
                .get_result(connection)
                .optional()
                .map(|row| row.map(to_record))
                .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_by_id(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                device_types::table
                    .filter(device_types::tenant_id.eq(tenant_id))
                    .filter(device_types::id.eq(id))
                    .select(DeviceType::as_select())
                    .first::<DeviceType>(connection)
                    .optional()
                    .map(|row| row.map(to_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn get_by_name(
        &self,
        tenant: &TenantId,
        name: &str,
    ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let name = name.to_owned();
        self.executor
            .run(move |connection| {
                device_types::table
                    .filter(device_types::tenant_id.eq(tenant_id))
                    .filter(device_types::name.eq(name))
                    .select(DeviceType::as_select())
                    .first::<DeviceType>(connection)
                    .optional()
                    .map(|row| row.map(to_record))
                    .map_err(map_diesel_error)
            })
            .await
    }

    async fn delete_if_unused(
        &self,
        tenant: &TenantId,
        id: i32,
    ) -> Result<DeleteDeviceTypeOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction(|connection| {
                        let exists = device_types::table
                            .filter(device_types::tenant_id.eq(&tenant_id))
                            .filter(device_types::id.eq(id))
                            .select(device_types::id)
                            .for_update()
                            .first::<i32>(connection)
                            .optional()?;
                        if exists.is_none() {
                            return Ok(DeleteDeviceTypeOutcome::NotFound);
                        }

                        let device_count = devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::device_type_id.eq(id))
                            .count()
                            .get_result::<i64>(connection)?;
                        if device_count > 0 {
                            return Ok(DeleteDeviceTypeOutcome::InUse { device_count });
                        }

                        diesel::delete(
                            device_types::table
                                .filter(device_types::tenant_id.eq(&tenant_id))
                                .filter(device_types::id.eq(id)),
                        )
                        .execute(connection)?;
                        Ok(DeleteDeviceTypeOutcome::Deleted)
                    })
                    .map_err(map_diesel_error)
            })
            .await
    }
}
