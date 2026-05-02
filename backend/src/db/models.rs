use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::schema::{
    alerts, api_keys, app_metrics, ca_certificates, command_history, device_certificates,
    device_configs, device_logs, device_shadows, device_types, devices, firmware_blobs,
    firmware_updates, fleets, network_observed_hosts, ota_deployments, rule_action_outbox,
    rule_actions, rule_conditions, rule_cooldowns, rules, server_config, server_metrics, telemetry,
    telemetry_rollups_hourly, users, zones,
};

// ---------------------------------------------------------------------------
// Rule Action Outbox
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = rule_action_outbox)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RuleActionOutboxEvent {
    pub id: String,
    pub tenant_id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: JsonValue,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub available_at: NaiveDateTime,
    pub locked_at: Option<NaiveDateTime>,
    pub locked_by: Option<String>,
    pub last_error: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = rule_action_outbox)]
pub struct NewRuleActionOutboxEvent {
    pub id: String,
    pub tenant_id: String,
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub payload: JsonValue,
}

// ---------------------------------------------------------------------------
// CA Certificates
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = ca_certificates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CaCertificate {
    pub id: i32,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ca_certificates)]
pub struct NewCaCertificate {
    pub private_key_pem: String,
    pub certificate_pem: String,
}

// ---------------------------------------------------------------------------
// Device Certificates
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = device_certificates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DeviceCertificate {
    pub id: i32,
    pub tenant_id: String,
    pub device_id: String,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub fingerprint: String,
    pub expires_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_certificates)]
pub struct NewDeviceCertificate {
    pub tenant_id: String,
    pub device_id: String,
    pub private_key_pem: String,
    pub certificate_pem: String,
    pub fingerprint: String,
    pub expires_at: NaiveDateTime,
}

// ---------------------------------------------------------------------------
// Device Types
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = device_types)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DeviceType {
    pub id: i32,
    pub name: String,
    pub icon: String,
    pub color_hex: String,
    pub created_at: NaiveDateTime,
    pub tenant_id: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_types)]
pub struct NewDeviceType {
    pub tenant_id: String,
    pub name: String,
    pub icon: String,
    pub color_hex: String,
}

#[derive(AsChangeset, Debug)]
#[diesel(table_name = device_types)]
pub struct UpdateDeviceType {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color_hex: Option<String>,
}

// ---------------------------------------------------------------------------
// Firmware Updates
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = firmware_updates)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FirmwareUpdate {
    pub id: i32,
    pub device_type_id: i32,
    pub version: String,
    pub url: String,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub sha256: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<NaiveDateTime>,
    pub changelog: Option<String>,
    pub source: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = firmware_updates)]
pub struct NewFirmwareUpdate {
    pub tenant_id: String,
    pub device_type_id: i32,
    pub version: String,
    pub url: String,
    pub description: Option<String>,
    pub sha256: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub ci_run_url: Option<String>,
    pub build_timestamp: Option<NaiveDateTime>,
    pub changelog: Option<String>,
    pub source: Option<String>,
}

// ---------------------------------------------------------------------------
// Firmware Blobs
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = firmware_blobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FirmwareBlob {
    pub firmware_update_id: i32,
    pub tenant_id: String,
    pub data: Vec<u8>,
    pub size: i32,
    pub filename: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = firmware_blobs)]
pub struct NewFirmwareBlob {
    pub firmware_update_id: i32,
    pub tenant_id: String,
    pub data: Vec<u8>,
    pub size: i32,
    pub filename: String,
}

// ---------------------------------------------------------------------------
// OTA Deployments
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = ota_deployments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OtaDeployment {
    pub id: i32,
    pub device_id: String,
    pub firmware_update_id: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub initiated_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = ota_deployments)]
pub struct NewOtaDeployment {
    pub tenant_id: String,
    pub device_id: String,
    pub firmware_update_id: i32,
}

// ---------------------------------------------------------------------------
// Fleets
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = fleets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Fleet {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub tenant_id: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = fleets)]
pub struct NewFleet {
    pub tenant_id: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Device {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub status: String,
    pub firmware: String,
    pub last_seen: Option<NaiveDateTime>,
    pub uptime_seconds: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub latest_latitude: Option<f64>,
    pub latest_longitude: Option<f64>,
    pub declared_connections: JsonValue,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = devices)]
pub struct NewDevice {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub firmware: String,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = devices)]
pub struct UpdateDevice {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    pub fleet_id: Option<Option<i32>>,
    pub firmware: Option<String>,
    pub status: Option<String>,
    pub last_seen: Option<NaiveDateTime>,
    pub uptime_seconds: Option<i32>,
    pub updated_at: Option<NaiveDateTime>,
    pub declared_connections: Option<JsonValue>,
}

// ---------------------------------------------------------------------------
// Network Observed Hosts
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = network_observed_hosts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NetworkObservedHost {
    pub id: i64,
    pub tenant_id: String,
    pub analyzer_device_id: String,
    pub host_key: String,
    pub label: String,
    pub address: Option<String>,
    pub device_type: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = network_observed_hosts)]
pub struct NewNetworkObservedHost {
    pub tenant_id: String,
    pub analyzer_device_id: String,
    pub host_key: String,
    pub label: String,
    pub address: Option<String>,
    pub device_type: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub first_seen_at: NaiveDateTime,
    pub last_seen_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = telemetry)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TelemetryRecord {
    pub id: i64,
    pub tenant_id: String,
    pub device_id: String,
    pub payload: Vec<u8>,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<JsonValue>,
    pub received_at: NaiveDateTime,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = telemetry)]
pub struct NewTelemetryRecord {
    pub tenant_id: String,
    pub device_id: String,
    pub payload: Vec<u8>,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<JsonValue>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed: Option<f32>,
    pub altitude: Option<f32>,
    pub heading: Option<f32>,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = telemetry_rollups_hourly)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TelemetryRollupHourly {
    pub tenant_id: String,
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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// ---------------------------------------------------------------------------
// Device Shadows
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = device_shadows)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DeviceShadow {
    pub device_id: String,
    pub tenant_id: String,
    pub desired: JsonValue,
    pub reported: JsonValue,
    pub delta: JsonValue,
    pub version: i32,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_shadows)]
pub struct NewDeviceShadow {
    pub device_id: String,
    pub tenant_id: String,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = device_shadows)]
pub struct UpdateShadow {
    pub desired: Option<JsonValue>,
    pub reported: Option<JsonValue>,
    pub delta: Option<JsonValue>,
    pub version: Option<i32>,
    pub updated_at: Option<NaiveDateTime>,
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub tenant_id: String,
    pub username: String,
    pub password_hash: String,
}

// ---------------------------------------------------------------------------
// Server Config
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = server_config)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ServerConfigEntry {
    pub key: String,
    pub value: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = server_config)]
pub struct NewServerConfigEntry {
    pub key: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Device Logs
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = device_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DeviceLog {
    pub id: i64,
    pub tenant_id: String,
    pub device_id: String,
    pub level: String,
    pub message: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_logs)]
pub struct NewDeviceLog {
    pub tenant_id: String,
    pub device_id: String,
    pub level: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Device Configs
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = device_configs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DeviceConfig {
    pub device_id: String,
    pub tenant_id: String,
    pub config: JsonValue,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_configs)]
pub struct NewDeviceConfig {
    pub device_id: String,
    pub tenant_id: String,
    pub config: JsonValue,
}

// ---------------------------------------------------------------------------
// Command History
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = command_history)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CommandRecord {
    pub id: String,
    pub device_id: String,
    pub command: String,
    pub params: String,
    pub status: String,
    pub response_payload: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = command_history)]
pub struct NewCommandRecord {
    pub id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub command: String,
    pub params: String,
}

// ---------------------------------------------------------------------------
// API Keys
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = api_keys)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ApiKey {
    pub id: i32,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub device_type_id: Option<i32>,
    pub created_at: NaiveDateTime,
    pub last_used_at: Option<NaiveDateTime>,
    pub tenant_id: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = api_keys)]
pub struct NewApiKey {
    pub tenant_id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub device_type_id: Option<i32>,
}

// ---------------------------------------------------------------------------
// Server Diagnostics
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = server_metrics)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ServerMetric {
    pub id: i64,
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

#[derive(Insertable, Debug)]
#[diesel(table_name = server_metrics)]
pub struct NewServerMetric {
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

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = app_metrics)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AppMetric {
    pub id: i64,
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

#[derive(Insertable, Debug)]
#[diesel(table_name = app_metrics)]
pub struct NewAppMetric {
    pub request_count: i32,
    pub error_count: i32,
    pub avg_latency_ms: f32,
    pub p95_latency_ms: f32,
    pub db_pool_active: i32,
    pub db_pool_idle: i32,
    pub zenoh_messages_in: i32,
    pub zenoh_messages_out: i32,
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = rules)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Rule {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = rules)]
pub struct NewRule {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub trigger_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub cooldown_seconds: i32,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = rules)]
pub struct UpdateRule {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub enabled: Option<bool>,
    pub trigger_type: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<Option<String>>,
    pub cooldown_seconds: Option<i32>,
    pub updated_at: Option<NaiveDateTime>,
}

// ---------------------------------------------------------------------------
// Rule Conditions
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = rule_conditions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RuleCondition {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: String,
    pub field: String,
    pub operator: String,
    pub value: String,
    pub condition_group: i32,
    pub zone_id: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = rule_conditions)]
pub struct NewRuleCondition {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: String,
    pub field: String,
    pub operator: String,
    pub value: String,
    pub condition_group: i32,
    pub zone_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Rule Actions
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = rule_actions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RuleAction {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: String,
    pub action_type: String,
    pub config: JsonValue,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = rule_actions)]
pub struct NewRuleAction {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: String,
    pub action_type: String,
    pub config: JsonValue,
}

// ---------------------------------------------------------------------------
// Alerts
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = alerts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Alert {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: Option<String>,
    pub device_id: String,
    pub severity: String,
    pub status: String,
    pub message: String,
    pub triggered_value: Option<String>,
    pub resolved_at: Option<NaiveDateTime>,
    pub acknowledged_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = alerts)]
pub struct NewAlert {
    pub id: String,
    pub tenant_id: String,
    pub rule_id: Option<String>,
    pub device_id: String,
    pub severity: String,
    pub message: String,
    pub triggered_value: Option<String>,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = alerts)]
pub struct UpdateAlert {
    pub status: Option<String>,
    pub resolved_at: Option<Option<NaiveDateTime>>,
    pub acknowledged_at: Option<Option<NaiveDateTime>>,
}

// ---------------------------------------------------------------------------
// Rule Cooldowns
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = rule_cooldowns)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RuleCooldown {
    pub tenant_id: String,
    pub rule_id: String,
    pub device_id: String,
    pub last_fired_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = rule_cooldowns)]
pub struct NewRuleCooldown {
    pub tenant_id: String,
    pub rule_id: String,
    pub device_id: String,
    pub last_fired_at: NaiveDateTime,
}

// ---------------------------------------------------------------------------
// Zones
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Serialize, Deserialize)]
#[diesel(table_name = zones)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Zone {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: JsonValue,
    pub color: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = zones)]
pub struct NewZone {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: JsonValue,
    pub color: String,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = zones)]
pub struct UpdateZone {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<JsonValue>,
    pub color: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
}
