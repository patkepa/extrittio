use super::require_permission;
use crate::dashboard::DashboardReadRepository;
use crate::{ApplicationError, Permission, TenantContext};
#[derive(Clone)]
pub struct DashboardApplication {
    repository: std::sync::Arc<dyn DashboardReadRepository>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardStats {
    pub total_devices: i64,
    pub active_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

impl DashboardApplication {
    pub fn new(repository: std::sync::Arc<dyn DashboardReadRepository>) -> Self {
        Self { repository }
    }
    pub async fn get_stats(&self, ctx: &TenantContext) -> Result<DashboardStats, ApplicationError> {
        require_permission(ctx, Permission::ReadDevices)?;
        let counts = self.repository.get_summary(ctx.tenant_id()).await?;
        Ok(DashboardStats {
            total_devices: counts.total_devices,
            active_devices: counts.online_devices,
            offline_devices: counts.offline_devices,
            total_messages: counts.total_messages,
        })
    }
}
