use chrono::NaiveDateTime;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::logs::port::LogRepository;
use crate::domains::logs::types::{LogQuery, LogRecord};
use crate::error::AppError;
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
