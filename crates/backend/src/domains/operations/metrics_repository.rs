use async_trait::async_trait;
use chrono::NaiveDateTime;

use crate::persistence::PersistenceError;

use super::metrics_types::{
    MetricsHistory, MetricsSnapshot, NewAppMetricRecord, NewSystemMetricRecord,
};

#[async_trait]
pub trait MetricsRepository: Send + Sync {
    async fn insert_system(&self, record: NewSystemMetricRecord) -> Result<(), PersistenceError>;
    async fn insert_app(&self, record: NewAppMetricRecord) -> Result<(), PersistenceError>;
    async fn current(&self) -> Result<MetricsSnapshot, PersistenceError>;
    async fn history(
        &self,
        since: NaiveDateTime,
        resolution_secs: i64,
    ) -> Result<MetricsHistory, PersistenceError>;
    async fn delete_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<(usize, usize), PersistenceError>;
}
