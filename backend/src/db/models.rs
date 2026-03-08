use chrono::NaiveDateTime;
use diesel::prelude::*;

use super::schema::{devices, telemetry};

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = devices)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Device {
    pub id: String,
    pub name: String,
    pub device_type: String,
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
    pub device_type: String,
    pub location: String,
    pub firmware: String,
}

#[derive(AsChangeset, Debug, Default)]
#[diesel(table_name = devices)]
pub struct UpdateDevice {
    pub name: Option<String>,
    pub device_type: Option<String>,
    pub location: Option<String>,
    pub firmware: Option<String>,
    pub status: Option<String>,
    pub last_seen: Option<NaiveDateTime>,
    pub uptime_seconds: Option<i32>,
    pub updated_at: Option<NaiveDateTime>,
}

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
