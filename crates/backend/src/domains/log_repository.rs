use async_trait::async_trait;

use chrono::NaiveDateTime;

use crate::domains::logs::types::{LogQuery, LogRecord};
use crate::persistence::PersistenceError;
use crate::tenancy::{DeviceIdentity, TenantId};

#[async_trait]
pub trait LogRepository: Send + Sync {
    /// Records a device-originated log after rechecking the tenant-qualified
    /// device identity. Returns false if the device disappeared concurrently.
    async fn record(
        &self,
        identity: &DeviceIdentity,
        level: String,
        message: String,
    ) -> Result<bool, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: LogQuery,
    ) -> Result<Option<Vec<LogRecord>>, PersistenceError>;

    async fn delete_older_than(&self, cutoff: NaiveDateTime) -> Result<usize, PersistenceError>;
}
