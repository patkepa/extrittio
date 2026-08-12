use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::telemetry::port::TelemetryRepository;
use crate::domains::telemetry::types::{TelemetryQuery, TelemetryRecord, TelemetryRollup};
use crate::error::AppError;

pub async fn list_with_repository(
    ctx: &RequestContext,
    repository: &dyn TelemetryRepository,
    device_id: &str,
    query: TelemetryQuery,
) -> Result<Vec<TelemetryRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    repository
        .list(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub async fn latest_with_repository(
    ctx: &RequestContext,
    repository: &dyn TelemetryRepository,
    device_id: &str,
) -> Result<Option<TelemetryRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    Ok(repository.latest(ctx.tenant_id(), device_id).await?)
}

pub async fn list_hourly_with_repository(
    ctx: &RequestContext,
    repository: &dyn TelemetryRepository,
    device_id: &str,
    query: TelemetryQuery,
) -> Result<Vec<TelemetryRollup>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    repository
        .list_hourly(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}
