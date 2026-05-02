use chrono::NaiveDateTime;
use chrono::Utc;
use diesel::OptionalExtension;
use diesel::PgConnection;
use serde_json::Value as JsonValue;

use crate::db::models::{Device, NewTelemetryRecord, TelemetryRecord, UpdateDevice};
use crate::error::AppError;
use crate::repositories::{device_repo, network_observed_host_repo, telemetry_repo};
use crate::services::device_connections::ObservedNetworkHost;

const NETWORK_OBSERVED_HOST_RETENTION_DAYS: i64 = 30;

pub fn list(
    conn: &mut PgConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, AppError> {
    device_repo::find_device(conn, device_id)?;
    Ok(telemetry_repo::list_telemetry(
        conn, device_id, since, before, limit,
    )?)
}

pub fn record(
    conn: &mut PgConnection,
    record: NewTelemetryRecord,
    declared_connections: Option<JsonValue>,
    observed_network_hosts: Option<Vec<ObservedNetworkHost>>,
) -> Result<Option<Device>, AppError> {
    let device: Option<Device> = device_repo::find_device(conn, &record.device_id)
        .optional()
        .map_err(AppError::Database)?;

    let device = match device {
        Some(d) => d,
        None => return Ok(None),
    };

    let device_id = record.device_id.clone();
    telemetry_repo::insert_telemetry(conn, &record)?;

    let now = Utc::now().naive_utc();
    if let Some(hosts) = observed_network_hosts {
        network_observed_host_repo::replace_active_scan(conn, &device_id, &hosts, now)?;
        network_observed_host_repo::delete_older_than(
            conn,
            now - chrono::Duration::days(NETWORK_OBSERVED_HOST_RETENTION_DAYS),
        )?;
    }

    let changeset = UpdateDevice {
        last_seen: Some(now),
        updated_at: Some(now),
        declared_connections,
        ..Default::default()
    };
    device_repo::update_device(conn, &device_id, &changeset)?;

    Ok(Some(device))
}
