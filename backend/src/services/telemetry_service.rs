use chrono::NaiveDateTime;
use chrono::Utc;
use diesel::SqliteConnection;
use diesel::OptionalExtension;

use crate::db::models::{Device, NewTelemetryRecord, TelemetryRecord, UpdateDevice};
use crate::error::AppError;
use crate::repositories::{device_repo, telemetry_repo};

pub fn list(
    conn: &mut SqliteConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, AppError> {
    device_repo::find_device(conn, device_id)?;
    Ok(telemetry_repo::list_telemetry(conn, device_id, since, before, limit)?)
}

pub fn record(
    conn: &mut SqliteConnection,
    device_id: &str,
    temperature: Option<f32>,
    humidity: Option<f32>,
    battery_level: Option<f32>,
    custom_json: Option<String>,
    payload: Vec<u8>,
) -> Result<Option<Device>, AppError> {
    let device: Option<Device> = device_repo::find_device(conn, device_id)
        .optional()
        .map_err(AppError::Database)?;

    let device = match device {
        Some(d) => d,
        None => return Ok(None),
    };

    let record = NewTelemetryRecord {
        device_id: device_id.to_string(),
        payload,
        temperature,
        humidity,
        battery_level,
        custom_json,
        latitude: None,
        longitude: None,
        speed: None,
        altitude: None,
        heading: None,
    };
    telemetry_repo::insert_telemetry(conn, &record)?;

    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };
    device_repo::update_device(conn, device_id, &changeset)?;

    Ok(Some(device))
}
