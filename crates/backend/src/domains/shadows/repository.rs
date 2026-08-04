use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::ShadowRecord;

#[async_trait]
pub trait ShadowRepository: Send + Sync {
    async fn get(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<ShadowRecord>, PersistenceError>;

    async fn update_desired(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError>;

    async fn update_reported(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<Option<ShadowRecord>, PersistenceError>;

    async fn reset(
        &self,
        tenant: &TenantId,
        device_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError>;
}
