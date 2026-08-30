use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::error::AppError;

use super::repository::DeviceEventRepository;
use super::types::{DeviceMetricQuery, DeviceMetricRecord};

pub async fn list_metrics(
    ctx: &RequestContext,
    repository: &dyn DeviceEventRepository,
    device_id: &str,
    query: DeviceMetricQuery,
) -> Result<Vec<DeviceMetricRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    repository
        .list_metrics(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}
