use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{AnalyticsBlueprintRevision, AnalyticsQuery, AnalyticsQueryData};

#[async_trait]
pub trait AnalyticsRepository: Send + Sync {
    async fn blueprint_catalog(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<AnalyticsBlueprintRevision>, PersistenceError>;

    async fn query(
        &self,
        tenant: &TenantId,
        query: AnalyticsQuery,
    ) -> Result<AnalyticsQueryData, PersistenceError>;
}
