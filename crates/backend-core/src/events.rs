use crate::{PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

use crate::rule_snapshots::DeviceRuleEvaluation;

#[derive(Debug, Clone, PartialEq)]
pub enum MetricValue {
    Float64(f64),
    Int64(i64),
    String(String),
    Boolean(bool),
    Json(Value),
}

impl MetricValue {
    #[must_use]
    pub const fn value_type(&self) -> &'static str {
        match self {
            Self::Float64(_) => "float64",
            Self::Int64(_) => "int64",
            Self::String(_) => "string",
            Self::Boolean(_) => "boolean",
            Self::Json(_) => "json",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceMetricSample {
    pub stream_key: String,
    pub field_path: String,
    pub value: MetricValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceMetricRecord {
    pub event_id: String,
    pub device_id: String,
    pub stream_key: String,
    pub field_path: String,
    pub value: MetricValue,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceMetricQuery {
    pub stream_key: Option<String>,
    pub field_path: Option<String>,
    pub since: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub limit: i64,
}

#[derive(Debug, Clone)]
pub struct RecordDeviceEvent {
    pub event_id: String,
    pub device_id: String,
    pub contract_id: String,
    pub route_key: String,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub payload: Value,
    pub metrics: Vec<DeviceMetricSample>,
    pub rule_evaluation: DeviceRuleEvaluation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordDeviceEventOutcome {
    pub recorded: bool,
    pub metrics_recorded: usize,
    pub actions_enqueued: usize,
}

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

/// Parsed transport values, independent of envelope spelling or encoding.
#[derive(Debug, Clone)]
pub struct EventInput {
    pub api_version: u32,
    pub event_id: String,
    pub contract_hash: String,
    pub occurred_at: DateTime<Utc>,
    pub payload: Value,
    pub encoded_size: u64,
}
