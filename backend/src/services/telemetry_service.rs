use chrono::NaiveDateTime;
use chrono::Utc;
use diesel::SqliteConnection;

use crate::db::models::{NewTelemetryRecord, TelemetryRecord, UpdateDevice};
use crate::error::AppError;
use crate::repositories::{device_repo, telemetry_repo};

pub fn list(
    conn: &mut SqliteConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, AppError> {
    device_repo::find_device(conn, device_id)?;
    Ok(telemetry_repo::list_telemetry(conn, device_id, since, limit)?)
}

pub fn record(
    conn: &mut SqliteConnection,
    device_id: &str,
    temperature: Option<f32>,
    humidity: Option<f32>,
    battery_level: Option<f32>,
    custom_json: Option<String>,
    payload: Vec<u8>,
) -> Result<bool, AppError> {
    if !device_repo::device_exists(conn, device_id)? {
        return Ok(false);
    }

    let record = NewTelemetryRecord {
        device_id: device_id.to_string(),
        payload,
        temperature,
        humidity,
        battery_level,
        custom_json,
    };
    telemetry_repo::insert_telemetry(conn, &record)?;

    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };
    device_repo::update_device(conn, device_id, &changeset)?;

    Ok(true)
}
