use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::audit::port::AuditRepository;
use crate::domains::audit::types::{AuditEventRecord, NewAuditEventRecord};
use crate::error::AppError;
use crate::tenancy::TenantId;

pub async fn record(
    repository: &dyn AuditRepository,
    tenant: &TenantId,
    event: NewAuditEventRecord,
) -> Result<(), AppError> {
    Ok(repository.record(tenant, event).await?)
}

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn AuditRepository,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditEventRecord>, AppError> {
    policy::require(ctx, Permission::ReadServerMetrics)?;
    Ok(repository.list(ctx.tenant_id(), limit, offset).await?)
}
