use diesel::PgConnection;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{DeviceType, NewDeviceType, UpdateDeviceType};
use crate::error::AppError;
use crate::repositories::device_type_repo;
use crate::tenancy::DEFAULT_TENANT_ID;

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceType>, i64), AppError> {
    policy::require(ctx, Permission::ReadDeviceTypes)?;
    Ok(device_type_repo::list_device_types(
        conn,
        ctx.tenant_id_str(),
        limit,
        offset,
    )?)
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
    ctx: &RequestContext,
    conn: &mut PgConnection,
    name: &str,
    icon: Option<&str>,
    color_hex: Option<&str>,
) -> Result<DeviceType, AppError> {
    policy::require(ctx, Permission::ManageDeviceTypes)?;

    let name = validate_name(name)?;
    let icon = validate_icon(icon.unwrap_or(DEFAULT_ICON))?;
    let color_hex = validate_color_hex(color_hex.unwrap_or(DEFAULT_COLOR_HEX))?;

    Ok(device_type_repo::insert_device_type(
        conn,
        ctx.tenant_id_str(),
        &NewDeviceType {
            tenant_id: ctx.tenant_id_str().to_string(),
            name,
            icon,
            color_hex,
        },
    )?)
}

pub fn update(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: i32,
    name: Option<&str>,
    icon: Option<&str>,
    color_hex: Option<&str>,
) -> Result<DeviceType, AppError> {
    policy::require(ctx, Permission::ManageDeviceTypes)?;

    let changes = UpdateDeviceType {
        name: name.map(validate_name).transpose()?,
        icon: icon.map(validate_icon).transpose()?,
        color_hex: color_hex.map(validate_color_hex).transpose()?,
    };

    if changes.name.is_none() && changes.icon.is_none() && changes.color_hex.is_none() {
        return find_by_id(ctx, conn, id);
    }

    device_type_repo::update_device_type(conn, ctx.tenant_id_str(), id, &changes).map_err(|e| {
        match e {
            diesel::result::Error::NotFound => {
                AppError::NotFound(format!("Device type {id} not found"))
            }
            other => AppError::Database(other),
        }
    })
}

pub fn delete(ctx: &RequestContext, conn: &mut PgConnection, id: i32) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageDeviceTypes)?;

    if id == 1 {
        return Err(AppError::UnprocessableEntity(
            "Cannot delete the default device type".into(),
        ));
    }

    let count = device_type_repo::count_devices_for_type(conn, ctx.tenant_id_str(), id)?;
    if count > 0 {
        return Err(AppError::Conflict(format!(
            "Cannot delete device type: {count} device(s) still reference it"
        )));
    }

    let deleted = device_type_repo::delete_device_type(conn, ctx.tenant_id_str(), id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Device type {id} not found")));
    }
    Ok(())
}

pub fn find_by_id(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: i32,
) -> Result<DeviceType, AppError> {
    device_type_repo::find_device_type_by_id(conn, ctx.tenant_id_str(), id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Device type {id} not found"))
        }
        other => AppError::Database(other),
    })
}

pub fn find_default_by_id(conn: &mut PgConnection, id: i32) -> Result<DeviceType, AppError> {
    device_type_repo::find_device_type_by_id(conn, DEFAULT_TENANT_ID, id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Device type {id} not found"))
        }
        other => AppError::Database(other),
    })
}
