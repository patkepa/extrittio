use diesel::SqliteConnection;

use crate::db::models::{DeviceType, NewDeviceType};
use crate::error::AppError;
use crate::repositories::device_type_repo;

pub fn list(
    conn: &mut SqliteConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceType>, i64), AppError> {
    Ok(device_type_repo::list_device_types(conn, limit, offset)?)
}

pub fn create(conn: &mut SqliteConnection, name: &str) -> Result<DeviceType, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Device type name must not be empty".into(),
        ));
    }
    Ok(device_type_repo::insert_device_type(
        conn,
        &NewDeviceType {
            name: name.to_string(),
        },
    )?)
}

pub fn delete(conn: &mut SqliteConnection, id: i32) -> Result<(), AppError> {
    if id == 1 {
        return Err(AppError::UnprocessableEntity(
            "Cannot delete the default device type".into(),
        ));
    }

    let count = device_type_repo::count_devices_for_type(conn, id)?;
    if count > 0 {
        return Err(AppError::Conflict(format!(
            "Cannot delete device type: {count} device(s) still reference it"
        )));
    }

    let deleted = device_type_repo::delete_device_type(conn, id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Device type {id} not found")));
    }
    Ok(())
}
