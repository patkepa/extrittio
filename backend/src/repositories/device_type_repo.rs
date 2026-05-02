// Repository functions for device types

use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{DeviceType, NewDeviceType, UpdateDeviceType};
use crate::db::schema::{device_types, devices};

pub fn list_device_types(
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceType>, i64), diesel::result::Error> {
    let total: i64 = device_types::table.count().get_result(conn)?;

    let results = device_types::table
        .select(DeviceType::as_select())
        .order(device_types::name.asc())
        .limit(limit)
        .offset(offset)
        .load(conn)?;

    Ok((results, total))
}

/// List all device types without pagination (used internally for lookups).
pub fn list_all_device_types(
    conn: &mut PgConnection,
) -> Result<Vec<DeviceType>, diesel::result::Error> {
    device_types::table
        .select(DeviceType::as_select())
        .order(device_types::name.asc())
        .load(conn)
}

pub fn insert_device_type(
    conn: &mut PgConnection,
    dt: &NewDeviceType,
) -> Result<DeviceType, diesel::result::Error> {
    diesel::insert_into(device_types::table)
        .values(dt)
        .execute(conn)?;

    device_types::table
        .order(device_types::id.desc())
        .select(DeviceType::as_select())
        .first(conn)
}

pub fn update_device_type(
    conn: &mut PgConnection,
    id: i32,
    dt: &UpdateDeviceType,
) -> Result<DeviceType, diesel::result::Error> {
    diesel::update(device_types::table.find(id))
        .set(dt)
        .execute(conn)?;

    device_types::table
        .find(id)
        .select(DeviceType::as_select())
        .first(conn)
}

pub fn delete_device_type(conn: &mut PgConnection, id: i32) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(device_types::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}

pub fn find_default_device_type_id(
    conn: &mut PgConnection,
) -> Result<Option<i32>, diesel::result::Error> {
    device_types::table
        .select(device_types::id)
        .order(device_types::id.asc())
        .first::<i32>(conn)
        .optional()
}

pub fn find_device_type_by_name(
    conn: &mut PgConnection,
    name: &str,
) -> Result<Option<DeviceType>, diesel::result::Error> {
    device_types::table
        .filter(device_types::name.eq(name))
        .select(DeviceType::as_select())
        .first(conn)
        .optional()
}

pub fn find_device_type_by_id(
    conn: &mut PgConnection,
    id: i32,
) -> Result<DeviceType, diesel::result::Error> {
    device_types::table
        .find(id)
        .select(DeviceType::as_select())
        .first(conn)
}

pub fn count_devices_for_type(
    conn: &mut PgConnection,
    device_type_id: i32,
) -> Result<i64, diesel::result::Error> {
    devices::table
        .filter(devices::device_type_id.eq(device_type_id))
        .count()
        .get_result(conn)
}
