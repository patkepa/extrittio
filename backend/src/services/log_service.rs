use chrono::NaiveDateTime;
use diesel::PgConnection;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{DeviceLog, NewDeviceLog};
use crate::error::AppError;
use crate::repositories::{device_repo, log_repo};

const VALID_LEVELS: &[&str] = &["DEBUG", "INFO", "WARN", "ERROR"];

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    level: Option<&str>,
    since: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<DeviceLog>, AppError> {
    policy::require(ctx, Permission::ReadLogs)?;

    if !device_repo::device_exists(conn, device_id)? {
        return Err(AppError::NotFound(format!(
            "Device '{device_id}' not found"
        )));
    }
    Ok(log_repo::list_logs(
        conn,
        ctx.tenant_id_str(),
        device_id,
        level,
        since,
        limit,
    )?)
}

pub fn record(
    conn: &mut PgConnection,
    device_id: &str,
    level: &str,
    message: &str,
) -> Result<bool, AppError> {
    if !device_repo::device_exists(conn, device_id)? {
        return Ok(false);
    }

    let normalized_level = level.to_uppercase();
    let level_str = if VALID_LEVELS.contains(&normalized_level.as_str()) {
        normalized_level
    } else {
        "INFO".to_string()
    };

    log_repo::insert_log(
        conn,
        &NewDeviceLog {
            tenant_id: crate::tenancy::DEFAULT_TENANT_ID.to_string(),
            device_id: device_id.to_string(),
            level: level_str,
            message: message.to_string(),
        },
    )?;

    Ok(true)
}
