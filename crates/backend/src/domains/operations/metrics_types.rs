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
