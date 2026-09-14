use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
/// Backend-neutral dashboard projection returned by persistence adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardSummary {
    pub total_devices: i64,
    pub online_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

#[async_trait]
pub trait DashboardReadRepository: Send + Sync {
    async fn get_summary(&self, tenant: &TenantId) -> Result<DashboardSummary, PersistenceError>;
}
