use chrono::NaiveDateTime;
use diesel::PgConnection;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{DeviceLog, NewDeviceLog};
use crate::domains::logs::port::LogRepository;
use crate::domains::logs::types::{LogQuery, LogRecord};
use crate::error::AppError;
use crate::repositories::{device_repo, log_repo};
use crate::tenancy::DeviceIdentity;

const VALID_LEVELS: &[&str] = &["DEBUG", "INFO", "WARN", "ERROR"];

pub async fn list_with_repository(
    ctx: &RequestContext,
    repository: &dyn LogRepository,
    device_id: &str,
    query: LogQuery,
) -> Result<Vec<LogRecord>, AppError> {
    policy::require(ctx, Permission::ReadLogs)?;
    repository
        .list(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub async fn delete_older_than_with_repository(
    repository: &dyn LogRepository,
    cutoff: NaiveDateTime,
) -> Result<usize, AppError> {
    Ok(repository.delete_older_than(cutoff).await?)
}

pub async fn record_with_repository(
    repository: &dyn LogRepository,
    identity: &DeviceIdentity,
    level: &str,
    message: &str,
) -> Result<bool, AppError> {
    let normalized_level = level.to_uppercase();
    let level = if VALID_LEVELS.contains(&normalized_level.as_str()) {
        normalized_level
    } else {
        "INFO".to_string()
    };
    Ok(repository
        .record(identity, level, message.to_string())
        .await?)
}

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    level: Option<&str>,
    since: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<DeviceLog>, AppError> {
    policy::require(ctx, Permission::ReadLogs)?;

    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id).map_err(
        |e| match e {
            diesel::result::Error::NotFound => {
                AppError::NotFound(format!("Device '{device_id}' not found"))
            }
            other => AppError::Database(other),
        },
    )?;
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
    identity: &DeviceIdentity,
    level: &str,
    message: &str,
) -> Result<bool, AppError> {
    match device_repo::find_device_for_tenant(conn, identity.tenant_id_str(), identity.device_id())
    {
        Ok(_) => {}
        Err(diesel::result::Error::NotFound) => return Ok(false),
        Err(e) => return Err(AppError::Database(e)),
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
            tenant_id: identity.tenant_id_str().to_string(),
            device_id: identity.device_id().to_string(),
            level: level_str,
            message: message.to_string(),
        },
    )?;

    Ok(true)
}

pub fn delete_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, AppError> {
    Ok(log_repo::delete_older_than(conn, cutoff)?)
}
