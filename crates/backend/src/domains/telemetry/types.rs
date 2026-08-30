use chrono::NaiveDateTime;
use serde_json::Value;

use crate::rule_engine::types::PendingAction;

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
    pub pending_actions: Vec<PendingAction>,
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
