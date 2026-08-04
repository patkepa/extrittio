use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::DashboardSummary;

#[async_trait]
pub trait DashboardReadRepository: Send + Sync {
    async fn get_summary(&self, tenant: &TenantId) -> Result<DashboardSummary, PersistenceError>;
}
