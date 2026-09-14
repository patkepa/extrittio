use super::require_permission;
use crate::audit::{AuditEventRecord, AuditRepository, NewAuditEventRecord};
use crate::{ApplicationError, Permission, TenantContext, TenantId};
use std::sync::Arc;
#[derive(Clone)]
pub struct AuditApplication {
    repository: Arc<dyn AuditRepository>,
}
impl AuditApplication {
    pub fn new(repository: Arc<dyn AuditRepository>) -> Self {
        Self { repository }
    }
    /// Host access logging supplies authenticated tenant attribution. A storage
    /// error is diagnostic only: the host must retain the original HTTP response.
    pub async fn record_access(
        &self,
        tenant: &TenantId,
        event: NewAuditEventRecord,
    ) -> Result<(), ApplicationError> {
        Ok(self.repository.record(tenant, event).await?)
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditEventRecord>, ApplicationError> {
        require_permission(ctx, Permission::ReadServerMetrics)?;
        Ok(self
            .repository
            .list(ctx.tenant_id(), limit.clamp(1, 200), offset.max(0))
            .await?)
    }
}
