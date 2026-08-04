use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::api_key_types::{ApiKeyRecord, ApiKeySummary, CreateApiKeyRecord};

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateApiKeyRecord,
    ) -> Result<ApiKeyRecord, PersistenceError>;

    async fn list(&self, tenant: &TenantId) -> Result<Vec<ApiKeySummary>, PersistenceError>;

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError>;
}
