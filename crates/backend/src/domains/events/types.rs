use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

use extrittio_backend_core::rule_snapshots::DeviceRuleEvaluation;

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
