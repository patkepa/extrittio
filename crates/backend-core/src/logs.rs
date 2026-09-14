use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub id: i64,
    pub device_id: String,
    pub level: String,
    pub message: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct LogQuery {
    pub level: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub limit: i64,
}

#[async_trait]
pub trait LogRepository: Send + Sync {
    /// Records a device-originated log after rechecking the tenant-qualified
    /// device identity. Returns false if the device disappeared concurrently.
    async fn record(
        &self,
        tenant: &TenantId,
        device_id: &str,
        level: String,
        message: String,
        observed_at: NaiveDateTime,
    ) -> Result<bool, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: LogQuery,
    ) -> Result<Option<Vec<LogRecord>>, PersistenceError>;

    async fn delete_older_than(&self, cutoff: NaiveDateTime) -> Result<usize, PersistenceError>;
}
