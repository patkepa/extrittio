use chrono::NaiveDateTime;
use diesel::prelude::*;

use super::schema::{ca_certificates, command_history, device_certificates, device_configs, device_logs, device_types, devices, firmware_blobs, firmware_updates, fleets, ota_deployments, telemetry, device_shadows, users, server_config};

// ---------------------------------------------------------------------------
// CA Certificates
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = ca_certificates)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeviceCertificate {
    pub id: i32,
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
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeviceType {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_types)]
pub struct NewDeviceType {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Firmware Updates
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = firmware_updates)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FirmwareUpdate {
    pub id: i32,
    pub device_type_id: i32,
    pub version: String,
    pub url: String,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub sha256: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = firmware_updates)]
pub struct NewFirmwareUpdate {
    pub device_type_id: i32,
    pub version: String,
    pub url: String,
    pub description: Option<String>,
    pub sha256: Option<String>,
}

// ---------------------------------------------------------------------------
// Firmware Blobs
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = firmware_blobs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FirmwareBlob {
    pub firmware_update_id: i32,
    pub data: Vec<u8>,
    pub size: i32,
    pub filename: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = firmware_blobs)]
pub struct NewFirmwareBlob {
    pub firmware_update_id: i32,
    pub data: Vec<u8>,
    pub size: i32,
    pub filename: String,
}

// ---------------------------------------------------------------------------
// OTA Deployments
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = ota_deployments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
    pub device_id: String,
    pub firmware_update_id: i32,
}

// ---------------------------------------------------------------------------
// Fleets
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = fleets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Fleet {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = fleets)]
pub struct NewFleet {
    pub name: String,
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Device {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub status: String,
    pub firmware: String,
    pub location: String,
    pub last_seen: Option<NaiveDateTime>,
    pub uptime_seconds: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = devices)]
pub struct NewDevice {
    pub id: String,
    pub name: String,
    pub device_type_id: i32,
    pub fleet_id: Option<i32>,
    pub location: String,
    pub firmware: String,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = devices)]
pub struct UpdateDevice {
    pub name: Option<String>,
    pub device_type_id: Option<i32>,
    pub fleet_id: Option<Option<i32>>,
    pub location: Option<String>,
    pub firmware: Option<String>,
    pub status: Option<String>,
    pub last_seen: Option<NaiveDateTime>,
    pub uptime_seconds: Option<i32>,
    pub updated_at: Option<NaiveDateTime>,
}

// ---------------------------------------------------------------------------
// Telemetry
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = telemetry)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TelemetryRecord {
    pub id: i32,
    pub device_id: String,
    pub payload: Vec<u8>,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<String>,
    pub received_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = telemetry)]
pub struct NewTelemetryRecord {
    pub device_id: String,
    pub payload: Vec<u8>,
    pub temperature: Option<f32>,
    pub humidity: Option<f32>,
    pub battery_level: Option<f32>,
    pub custom_json: Option<String>,
}

// ---------------------------------------------------------------------------
// Device Shadows
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = device_shadows)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeviceShadow {
    pub device_id: String,
    pub desired: String,
    pub reported: String,
    pub delta: String,
    pub version: i32,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_shadows)]
pub struct NewDeviceShadow {
    pub device_id: String,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = device_shadows)]
pub struct UpdateShadow {
    pub desired: Option<String>,
    pub reported: Option<String>,
    pub delta: Option<String>,
    pub version: Option<i32>,
    pub updated_at: Option<NaiveDateTime>,
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
    pub username: String,
    pub password_hash: String,
}

// ---------------------------------------------------------------------------
// Server Config
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = server_config)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeviceLog {
    pub id: i32,
    pub device_id: String,
    pub level: String,
    pub message: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_logs)]
pub struct NewDeviceLog {
    pub device_id: String,
    pub level: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Device Configs
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = device_configs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DeviceConfig {
    pub device_id: String,
    pub config: String,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = device_configs)]
pub struct NewDeviceConfig {
    pub device_id: String,
    pub config: String,
}

// ---------------------------------------------------------------------------
// Command History
// ---------------------------------------------------------------------------

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = command_history)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
    pub device_id: String,
    pub command: String,
    pub params: String,
}
