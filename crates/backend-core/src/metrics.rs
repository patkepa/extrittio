use crate::PersistenceError;
use async_trait::async_trait;
use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct SystemMetricRecord {
    pub cpu_usage_percent: f32,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    pub disk_used_bytes: i64,
    pub disk_total_bytes: i64,
    pub network_rx_bytes_delta: i64,
    pub network_tx_bytes_delta: i64,
    pub load_avg_1m: f32,
    pub load_avg_5m: f32,
    pub load_avg_15m: f32,
    pub recorded_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewSystemMetricRecord {
    pub cpu_usage_percent: f32,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    pub disk_used_bytes: i64,
    pub disk_total_bytes: i64,
    pub network_rx_bytes_delta: i64,
    pub network_tx_bytes_delta: i64,
    pub load_avg_1m: f32,
    pub load_avg_5m: f32,
    pub load_avg_15m: f32,
}

#[derive(Debug, Clone)]
pub struct AppMetricRecord {
    pub request_count: i32,
    pub error_count: i32,
    pub avg_latency_ms: f32,
    pub p95_latency_ms: f32,
    pub db_pool_active: i32,
    pub db_pool_idle: i32,
    pub zenoh_messages_in: i32,
    pub zenoh_messages_out: i32,
    pub recorded_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct NewAppMetricRecord {
    pub request_count: i32,
    pub error_count: i32,
    pub avg_latency_ms: f32,
    pub p95_latency_ms: f32,
    pub db_pool_active: i32,
    pub db_pool_idle: i32,
    pub zenoh_messages_in: i32,
    pub zenoh_messages_out: i32,
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub system: Option<SystemMetricRecord>,
    pub app: Option<AppMetricRecord>,
}

#[derive(Debug, Clone)]
pub struct MetricsHistory {
    pub system: Vec<SystemMetricRecord>,
    pub app: Vec<AppMetricRecord>,
}

#[async_trait]
pub trait MetricsRepository: Send + Sync {
    async fn insert_system(&self, record: NewSystemMetricRecord) -> Result<(), PersistenceError>;
    async fn insert_app(&self, record: NewAppMetricRecord) -> Result<(), PersistenceError>;
    /// Latest sample per stream, ordered by timestamp then numeric ID descending.
    /// System/app reads are independent; this does not promise a joint snapshot.
    async fn current(&self) -> Result<MetricsSnapshot, PersistenceError>;
    /// Inclusive microsecond `since`, ascending timestamp/ID in raw mode (<=10s,
    /// first 10,000 rows per stream); larger resolutions use UTC epoch floor
    /// buckets, with no gap filling or bucket cap. Callers supply a resolution
    /// representable as positive i64 microseconds and a microsecond lower bound.
    /// Sum counters/deltas, average gauges, and take max sampled p95 (ADR-015).
    /// Integer gauges use integer sum/count, truncating toward zero; aggregates
    /// outside supported integer/timestamp ranges fail instead of wrapping.
    /// Streams are read independently, preserving existing snapshot semantics.
    async fn history(
        &self,
        since: NaiveDateTime,
        resolution_secs: i64,
    ) -> Result<MetricsHistory, PersistenceError>;
    /// Delete both system and application samples strictly before the cutoff
    /// in one transaction. A failure must leave both sets unchanged.
    async fn delete_before(
        &self,
        cutoff: NaiveDateTime,
    ) -> Result<(usize, usize), PersistenceError>;
}
