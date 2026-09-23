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
    pub contract_id: String,
    pub blueprint_id: String,
    pub blueprint_name: String,
    pub blueprint_revision_id: String,
    pub blueprint_revision: i32,
    pub event_id: String,
    pub device_id: String,
    pub stream_key: String,
    pub field_path: String,
    pub value: MetricValue,
    pub field: DeviceMetricFieldMetadata,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceMetricFieldMetadata {
    pub label: String,
    pub unit: Option<String>,
    pub semantic: Option<String>,
    pub presentation: Option<extrittio_device_contract::FieldPresentation>,
}

pub fn metric_field_metadata(
    definition: &Value,
    sample_type: &str,
) -> Result<DeviceMetricFieldMetadata, PersistenceError> {
    let value_type = definition.get("valueType").and_then(Value::as_str);
    let label = definition.get("label").and_then(Value::as_str);
    if value_type != Some(sample_type) || label.is_none() {
        return Err(PersistenceError::CorruptData(
            "stored metric does not match its originating contract field".into(),
        ));
    }
    let optional_string = |key| {
        definition
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    PersistenceError::CorruptData(format!(
                        "originating contract metric has invalid {key}"
                    ))
                })
            })
            .transpose()
    };
    let presentation = definition
        .get("presentation")
        .filter(|value| !value.is_null())
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|error| {
                PersistenceError::CorruptData(format!(
                    "originating contract metric has invalid presentation: {error}"
                ))
            })
        })
        .transpose()?;
    Ok(DeviceMetricFieldMetadata {
        label: label.unwrap().to_owned(),
        unit: optional_string("unit")?,
        semantic: optional_string("semantic")?,
        presentation,
    })
}

#[cfg(test)]
mod history_field_tests {
    use super::*;

    #[test]
    fn reads_origin_presentation_and_rejects_type_mismatch() {
        let definition = serde_json::json!({
            "valueType": "int64", "label": "Counter", "unit": "ticks",
            "presentation": {"chart": "step", "precision": 0, "color": "#123456"}
        });
        let field = metric_field_metadata(&definition, "int64").unwrap();
        assert_eq!(field.label, "Counter");
        assert_eq!(field.unit.as_deref(), Some("ticks"));
        assert_eq!(
            field.presentation.unwrap().chart,
            Some(extrittio_device_contract::ChartKind::Step)
        );
        assert!(metric_field_metadata(&definition, "float64").is_err());
        assert!(metric_field_metadata(
            &serde_json::json!({"valueType":"int64","label":"Counter","presentation":{"chart":"invalid"}}),
            "int64"
        ).is_err());
    }
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
pub struct DeviceLocationQuery {
    pub contract_id: String,
    pub stream_key: String,
    pub latitude_path: String,
    pub longitude_path: String,
    pub since: DateTime<Utc>,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceLocationRecord {
    pub contract_id: String,
    pub event_id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub occurred_at: DateTime<Utc>,
    /// Deadline from the originating contract's location binding.
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocatedDeviceRecord {
    pub device_id: String,
    pub location: DeviceLocationRecord,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricRetentionCutoffs {
    /// Keep the whole hour containing the policy cutoff for partial reads.
    pub raw_retained_since: DateTime<Utc>,
    pub rollup_retained_since: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricPruneOutcome {
    pub events_deleted: u64,
    pub rollups_deleted: u64,
    pub receipts_deleted: u64,
}

#[async_trait]
pub trait DeviceEventRepository: Send + Sync {
    /// Advance monotonic retention watermarks and prune raw events, rollups and
    /// durable delivery receipts in one transaction.
    async fn prune_metrics(
        &self,
        cutoffs: MetricRetentionCutoffs,
    ) -> Result<MetricPruneOutcome, PersistenceError>;

    /// One bounded read of current contract-declared locations. Missing bindings
    /// and devices without fresh valid observations are omitted, never zeroed.
    async fn latest_locations(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        now: DateTime<Utc>,
    ) -> Result<Vec<LocatedDeviceRecord>, PersistenceError>;

    /// Latest fresh, valid coordinate pair from one event under the current
    /// assignment. Must not combine events, contracts, streams or devices.
    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: DeviceLocationQuery,
    ) -> Result<Option<DeviceLocationRecord>, PersistenceError>;

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
