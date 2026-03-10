// Repository functions for device types

use diesel::prelude::*;
use diesel::SqliteConnection;

use crate::db::models::{DeviceType, NewDeviceType};
use crate::db::schema::{device_types, devices};

pub fn list_device_types(
    conn: &mut SqliteConnection,
) -> Result<Vec<DeviceType>, diesel::result::Error> {
    device_types::table
        .select(DeviceType::as_select())
        .order(device_types::name.asc())
        .load(conn)
}

pub fn insert_device_type(
    conn: &mut SqliteConnection,
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

pub fn delete_device_type(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<bool, diesel::result::Error> {
    let rows = diesel::delete(device_types::table.find(id)).execute(conn)?;
    Ok(rows > 0)
}

pub fn find_default_device_type_id(
    conn: &mut SqliteConnection,
) -> Result<Option<i32>, diesel::result::Error> {
    device_types::table
        .select(device_types::id)
        .order(device_types::id.asc())
        .first::<i32>(conn)
        .optional()
}

pub fn count_devices_for_type(
    conn: &mut SqliteConnection,
    device_type_id: i32,
) -> Result<i64, diesel::result::Error> {
    devices::table
        .filter(devices::device_type_id.eq(device_type_id))
        .count()
        .get_result(conn)
}
