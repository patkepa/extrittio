use chrono::NaiveDateTime;
use chrono::Utc;
use diesel::Connection;
use diesel::OptionalExtension;
use diesel::PgConnection;
use serde_json::Value as JsonValue;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{
    Device, NewTelemetryRecord, TelemetryRecord, TelemetryRollupHourly, UpdateDevice,
};
use crate::domains::telemetry::port::TelemetryRepository;
use crate::domains::telemetry::types::{
    TelemetryQuery, TelemetryRecord as PortTelemetryRecord, TelemetryRollup,
};
use crate::error::AppError;
use crate::repositories::{device_repo, network_observed_host_repo, telemetry_repo};
use crate::services::device_connections::ObservedNetworkHost;

const NETWORK_OBSERVED_HOST_RETENTION_DAYS: i64 = 30;

pub async fn list_with_repository(
    ctx: &RequestContext,
    repository: &dyn TelemetryRepository,
    device_id: &str,
    query: TelemetryQuery,
) -> Result<Vec<PortTelemetryRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    repository
        .list(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub async fn latest_with_repository(
    ctx: &RequestContext,
    repository: &dyn TelemetryRepository,
    device_id: &str,
) -> Result<Option<PortTelemetryRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    Ok(repository.latest(ctx.tenant_id(), device_id).await?)
}

pub async fn list_hourly_with_repository(
    ctx: &RequestContext,
    repository: &dyn TelemetryRepository,
    device_id: &str,
    query: TelemetryQuery,
) -> Result<Vec<TelemetryRollup>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    repository
        .list_hourly(ctx.tenant_id(), device_id, query)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;

    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id)?;
    Ok(telemetry_repo::list_telemetry(
        conn,
        ctx.tenant_id_str(),
        device_id,
        since,
        before,
        limit,
    )?)
}

pub fn latest(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<Option<TelemetryRecord>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id)?;
    Ok(telemetry_repo::latest_telemetry(
        conn,
        ctx.tenant_id_str(),
        device_id,
    )?)
}

pub fn list_hourly(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
) -> Result<Vec<TelemetryRollupHourly>, AppError> {
    policy::require(ctx, Permission::ReadTelemetry)?;
    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id)?;
    Ok(telemetry_repo::list_hourly_rollups(
        conn,
        ctx.tenant_id_str(),
        device_id,
        since,
        before,
        limit,
    )?)
}

pub fn record(
    conn: &mut PgConnection,
    record: NewTelemetryRecord,
    declared_connections: Option<JsonValue>,
    observed_network_hosts: Option<Vec<ObservedNetworkHost>>,
) -> Result<Option<Device>, AppError> {
    conn.transaction(|conn| {
        let device: Option<Device> =
            device_repo::find_device_for_tenant(conn, &record.tenant_id, &record.device_id)
                .optional()
                .map_err(AppError::Database)?;

        let device = match device {
            Some(device) => device,
            None => return Ok(None),
        };

        let device_id = record.device_id.clone();
        let tenant_id = record.tenant_id.clone();
        let (telemetry_id, received_at) = telemetry_repo::insert_telemetry(conn, &record)?;
        telemetry_repo::upsert_latest_state(conn, &record, telemetry_id, received_at)?;

        let now = Utc::now().naive_utc();
        if let Some(hosts) = observed_network_hosts {
            network_observed_host_repo::replace_active_scan(
                conn, &tenant_id, &device_id, &hosts, now,
            )?;
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
        device_repo::update_device(conn, &tenant_id, &device_id, &changeset)?;

        Ok(Some(device))
    })
}

pub fn upsert_hourly_rollups(
    conn: &mut PgConnection,
    since: NaiveDateTime,
    before: NaiveDateTime,
) -> Result<usize, AppError> {
    Ok(telemetry_repo::upsert_hourly_rollups(conn, since, before)?)
}

pub fn delete_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, AppError> {
    Ok(telemetry_repo::delete_older_than(conn, cutoff)?)
}

pub fn maintain_partitions(
    conn: &mut PgConnection,
    months_ahead: i32,
    cutoff: NaiveDateTime,
) -> Result<telemetry_repo::PartitionMaintenanceResult, AppError> {
    Ok(telemetry_repo::maintain_partitions(
        conn,
        months_ahead,
        cutoff,
    )?)
}
