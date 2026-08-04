use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{GetDeviceConfigOutcome, MergeDeviceConfigOutcome};

#[async_trait]
pub trait DeviceConfigRepository: Send + Sync {
    async fn get_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<GetDeviceConfigOutcome, PersistenceError>;

    /// Locks the device row, merges the patch against the latest stored value,
    /// and upserts the result in one transaction.
    async fn merge_for_device(
        &self,
        tenant: &TenantId,
        device_id: &str,
        patch: Map<String, Value>,
        updated_at: DateTime<Utc>,
    ) -> Result<MergeDeviceConfigOutcome, PersistenceError>;
}
