use async_trait::async_trait;

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::types::{
    DeviceMetricQuery, DeviceMetricRecord, RecordDeviceEvent, RecordDeviceEventOutcome,
};

#[async_trait]
pub trait DeviceEventRepository: Send + Sync {
    /// Idempotently stores a validated event and all extracted typed metrics in
    /// one transaction. A duplicate event ID returns `recorded=false`.
    async fn record(
        &self,
        tenant: &TenantId,
        event: RecordDeviceEvent,
    ) -> Result<RecordDeviceEventOutcome, PersistenceError>;

    /// Returns typed metric samples for one tenant-owned device. `None`
    /// distinguishes an unknown device from a known device with no samples.
    async fn list_metrics(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: DeviceMetricQuery,
    ) -> Result<Option<Vec<DeviceMetricRecord>>, PersistenceError>;
}
