use crate::{DeviceIdentity, PersistenceError, TenantId};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde_json::Value;

use crate::rule_snapshots::DeviceRuleEvaluation;

#[derive(Debug, Clone)]
pub struct TelemetryRecord {
    pub id: i64,
    pub device_id: String,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<Value>,
    pub received_at: NaiveDateTime,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct TelemetryRollup {
    pub device_id: String,
    pub bucket_start: NaiveDateTime,
    pub sample_count: i64,
    pub avg_temperature: Option<f32>,
    pub min_temperature: Option<f32>,
    pub max_temperature: Option<f32>,
    pub avg_humidity: Option<f32>,
    pub min_humidity: Option<f32>,
    pub max_humidity: Option<f32>,
    pub avg_battery_level: Option<f32>,
    pub min_battery_level: Option<f32>,
    pub max_battery_level: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct TelemetryQuery {
    pub since: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub limit: i64,
}

/// Decoded transport values; compatibility and rule policy are applied in core.
#[derive(Debug, Clone)]
pub struct TelemetryInput {
    pub payload: Vec<u8>,
    pub temperature: f32,
    pub humidity: f32,
    pub battery_level: f32,
    pub metadata: std::collections::BTreeMap<String, String>,
    pub has_location: bool,
    pub latitude: f64,
    pub longitude: f64,
    pub speed: f32,
    pub altitude: f32,
    pub heading: f32,
}

#[derive(Debug, Clone)]
pub struct TelemetryWrite {
    pub expected_device_type_id: i32,
    pub expected_fleet_id: Option<i32>,
    pub payload: Vec<u8>,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<Value>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
    pub rule_evaluation: DeviceRuleEvaluation,
    /// Server receipt time, fixed across targeting retries; UTC microsecond precision.
    pub received_at: NaiveDateTime,
    /// Server evaluation time for this attempt, used for presence and rules.
    pub observed_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryWriteOutcome {
    pub recorded: bool,
    pub actions_enqueued: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartitionMaintenance {
    pub created_count: i32,
    pub dropped_count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryMaintenanceOutcome {
    pub rollups_upserted: usize,
    pub rows_deleted: usize,
    pub partitions: PartitionMaintenance,
}

#[async_trait]
pub trait TelemetryRepository: Send + Sync {
    async fn record(
        &self,
        identity: &DeviceIdentity,
        write: TelemetryWrite,
    ) -> Result<TelemetryWriteOutcome, PersistenceError>;

    async fn list(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRecord>>, PersistenceError>;

    async fn latest(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError>;

    async fn list_hourly(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: TelemetryQuery,
    ) -> Result<Option<Vec<TelemetryRollup>>, PersistenceError>;

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
    ) -> Result<Option<TelemetryRecord>, PersistenceError>;

    async fn maintain(
        &self,
        rollup_since: NaiveDateTime,
        rollup_before: NaiveDateTime,
        retention_cutoff: NaiveDateTime,
    ) -> Result<TelemetryMaintenanceOutcome, PersistenceError>;
}

/// Only recompute whole hours whose raw data has never been pruned. The boundary
/// is monotonic across retention changes and is loaded under the maintenance lock.
pub fn rollup_recompute_start(
    requested: NaiveDateTime,
    pruned_before: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
    use chrono::Timelike;
    let Some(boundary) = pruned_before else {
        return Some(requested);
    };
    let hour = boundary
        .with_minute(0)?
        .with_second(0)?
        .with_nanosecond(0)?;
    let first_complete_hour = if hour == boundary {
        hour
    } else {
        hour.checked_add_signed(chrono::Duration::hours(1))?
    };
    Some(requested.max(first_complete_hour))
}
