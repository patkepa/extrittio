use diesel::PgConnection;

use crate::db::models::{DeviceType, NewDeviceType, UpdateDeviceType};
use crate::error::AppError;
use crate::repositories::device_type_repo;

pub fn list(
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceType>, i64), AppError> {
    Ok(device_type_repo::list_device_types(conn, limit, offset)?)
}

const DEFAULT_ICON: &str = "cube";
const DEFAULT_COLOR_HEX: &str = "#8ABBFF";

fn validate_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(
            "Device type name must not be empty".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_icon(icon: &str) -> Result<String, AppError> {
    let trimmed = icon.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(AppError::BadRequest(
            "Device type icon must be 1-64 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AppError::BadRequest(
            "Device type icon must use lowercase letters, digits, hyphen, or underscore".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_color_hex(color_hex: &str) -> Result<String, AppError> {
    let trimmed = color_hex.trim();
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if !valid {
        return Err(AppError::BadRequest(
            "Device type color must be a #RRGGBB hex value".into(),
        ));
    }
    Ok(trimmed.to_ascii_uppercase())
}

pub fn create(
    conn: &mut PgConnection,
    name: &str,
    icon: Option<&str>,
    color_hex: Option<&str>,
) -> Result<DeviceType, AppError> {
    let name = validate_name(name)?;
    let icon = validate_icon(icon.unwrap_or(DEFAULT_ICON))?;
    let color_hex = validate_color_hex(color_hex.unwrap_or(DEFAULT_COLOR_HEX))?;

    Ok(device_type_repo::insert_device_type(
        conn,
        &NewDeviceType {
            name,
            icon,
            color_hex,
        },
    )?)
}

pub fn update(
    conn: &mut PgConnection,
    id: i32,
    name: Option<&str>,
    icon: Option<&str>,
    color_hex: Option<&str>,
) -> Result<DeviceType, AppError> {
    let changes = UpdateDeviceType {
        name: name.map(validate_name).transpose()?,
        icon: icon.map(validate_icon).transpose()?,
        color_hex: color_hex.map(validate_color_hex).transpose()?,
    };

    if changes.name.is_none() && changes.icon.is_none() && changes.color_hex.is_none() {
        return find_by_id(conn, id);
    }

    device_type_repo::update_device_type(conn, id, &changes).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Device type {id} not found"))
        }
        other => AppError::Database(other),
    })
}

pub fn delete(conn: &mut PgConnection, id: i32) -> Result<(), AppError> {
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

pub fn find_by_id(conn: &mut PgConnection, id: i32) -> Result<DeviceType, AppError> {
    device_type_repo::find_device_type_by_id(conn, id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Device type {id} not found"))
        }
        other => AppError::Database(other),
    })
}
