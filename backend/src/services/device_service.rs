// Device service — business logic for device management

use diesel::Connection;
use diesel::PgConnection;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{NewDevice, NewDeviceLog, NewDeviceShadow, NewOtaDeployment, UpdateDevice};
use crate::error::AppError;
use crate::repositories::{
    cert_repo, device_repo, device_type_repo, firmware_repo, log_repo, network_observed_host_repo,
    shadow_repo, telemetry_repo,
};
use crate::services::{cert_service, device_connections, shadow_service};
use crate::state::{DbPool, ZenohMetrics, run_db};
use crate::tenancy::DEFAULT_TENANT_ID;

pub use crate::repositories::device_repo::DeviceWithJoins;

const NETWORK_ANALYZER_DEVICE_TYPE: &str = "network-analyzer";
const NETWORK_OBSERVED_HOST_RETENTION_DAYS: i64 = 30;

fn is_network_analyzer_device(
    device: &crate::db::models::Device,
    device_type: &crate::db::models::DeviceType,
) -> bool {
    device_type.name == NETWORK_ANALYZER_DEVICE_TYPE
        || device.id.contains("network-analyzer")
        || device.name.contains("network-analyzer")
        || device.firmware.contains("network-analyzer")
        || device.firmware.contains("network_analyzer")
}

fn has_no_declared_connections(value: &JsonValue) -> bool {
    value.as_array().is_none_or(Vec::is_empty)
}

fn hydrate_declared_connections_from_telemetry(
    conn: &mut PgConnection,
    devices: &mut [device_repo::DeviceWithJoins],
) -> Result<(), AppError> {
    let candidate_ids: Vec<String> = devices
        .iter()
        .filter_map(|(device, _, _)| {
            has_no_declared_connections(&device.declared_connections).then(|| device.id.clone())
        })
        .collect();

    if candidate_ids.is_empty() {
        return Ok(());
    }

    let sources = telemetry_repo::latest_connection_sources_for_devices(conn, &candidate_ids)?;
    let connections_by_device: HashMap<String, JsonValue> = sources
        .into_iter()
        .filter_map(|source| {
            device_connections::declared_connections_from_custom_json(&source.custom_json)
                .map(|connections| (source.device_id, connections))
        })
        .collect();

    if connections_by_device.is_empty() {
        return Ok(());
    }

    for (device, _, _) in devices {
        if !has_no_declared_connections(&device.declared_connections) {
            continue;
        }
        if let Some(connections) = connections_by_device.get(&device.id) {
            device.declared_connections = connections.clone();
        }
    }

    Ok(())
}

fn hydrate_network_observed_hosts(
    conn: &mut PgConnection,
    tenant_id: &str,
    devices: &mut [device_repo::DeviceWithJoins],
) -> Result<(), AppError> {
    let analyzer_ids: Vec<String> = devices
        .iter()
        .filter_map(|(device, device_type, _)| {
            is_network_analyzer_device(device, device_type).then(|| device.id.clone())
        })
        .collect();

    if analyzer_ids.is_empty() {
        return Ok(());
    }

    let cutoff = chrono::Utc::now().naive_utc()
        - chrono::Duration::days(NETWORK_OBSERVED_HOST_RETENTION_DAYS);
    let observed_hosts = network_observed_host_repo::list_recent_for_analyzers(
        conn,
        tenant_id,
        &analyzer_ids,
        cutoff,
    )?;

    let mut connections_by_analyzer: HashMap<String, Vec<JsonValue>> = HashMap::new();
    for host in observed_hosts {
        let observed = device_connections::ObservedNetworkHost {
            host_key: host.host_key,
            label: host.label,
            address: host.address,
            device_type: host.device_type,
            source: host.source,
        };
        connections_by_analyzer
            .entry(host.analyzer_device_id)
            .or_default()
            .push(device_connections::network_host_connection(
                &observed,
                &host.status,
                Some(host.first_seen_at),
                Some(host.last_seen_at),
            ));
    }

    for (device, device_type, _) in devices {
        if !is_network_analyzer_device(device, device_type) {
            continue;
        }
        if let Some(connections) = connections_by_analyzer.remove(&device.id) {
            device.declared_connections = JsonValue::Array(connections);
        }
    }

    Ok(())
}

/// Create a device and its associated shadow record atomically.
pub fn create_device(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    new_device: &NewDevice,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageDevices)?;

    conn.transaction(|conn| {
        device_repo::insert_device(conn, new_device)?;
        let new_shadow = NewDeviceShadow {
            device_id: new_device.id.clone(),
            tenant_id: ctx.tenant_id_str().to_string(),
        };
        shadow_repo::insert_shadow(conn, &new_shadow)?;

        // Generate device certificate if CA exists
        if let Some(ca) = cert_repo::get_ca_certificate(conn)? {
            let new_cert = cert_service::generate_device_certificate_for_tenant(
                ctx.tenant_id_str(),
                &new_device.id,
                &ca,
            )?;
            cert_repo::insert_device_certificate(conn, &new_cert)?;
        }

        Ok(())
    })
}

/// Infer the device type name from the firmware version string.
fn infer_device_type(firmware: &str) -> &'static str {
    let firmware = firmware.to_ascii_lowercase();
    if firmware.contains("network-analyzer") || firmware.contains("network_analyzer") {
        "network-analyzer"
    } else if firmware.contains("organbath")
        || firmware.contains("organ-bath")
        || firmware.contains("organ_bath")
    {
        "OrganBath"
    } else if firmware.contains("macos") {
        "mac-device"
    } else {
        "default"
    }
}

/// Auto-register a device on first heartbeat. Infers device type from firmware
/// string and logs the registration event.
///
/// Returns `true` if the device was newly registered, `false` if it already
/// existed, or `None` if registration failed.
pub fn auto_register_device(
    conn: &mut PgConnection,
    device_id: &str,
    firmware: &str,
) -> Option<bool> {
    match device_repo::device_exists(conn, device_id) {
        Ok(true) => return Some(false),
        Ok(false) => {}
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return None;
        }
    }

    let type_name = infer_device_type(firmware);
    let device_type_id =
        match device_type_repo::find_device_type_by_name(conn, DEFAULT_TENANT_ID, type_name) {
            Ok(Some(dt)) => dt.id,
            Ok(None) => {
                warn!(
                    "Device type '{}' not found, falling back to default",
                    type_name
                );
                match device_type_repo::find_default_device_type_id(conn) {
                    Ok(Some(id)) => id,
                    _ => {
                        warn!("No default device type found, dropping heartbeat");
                        return None;
                    }
                }
            }
            Err(e) => {
                warn!("DB error looking up device type: {}", e);
                return None;
            }
        };

    let new_device = NewDevice {
        id: device_id.to_string(),
        tenant_id: DEFAULT_TENANT_ID.to_string(),
        name: device_id.to_string(),
        device_type_id,
        fleet_id: None,
        firmware: firmware.to_string(),
    };

    if let Err(e) = conn.transaction(|conn| {
        device_repo::insert_device(conn, &new_device)?;
        let new_shadow = NewDeviceShadow {
            device_id: device_id.to_string(),
            tenant_id: DEFAULT_TENANT_ID.to_string(),
        };
        shadow_repo::insert_shadow(conn, &new_shadow)?;
        Ok::<(), diesel::result::Error>(())
    }) {
        warn!("Failed to auto-register device {}: {}", device_id, e);
        return None;
    }

    info!(
        "Auto-registered device {} as type '{}'",
        device_id, type_name
    );

    let _ = log_repo::insert_log(
        conn,
        &NewDeviceLog {
            tenant_id: DEFAULT_TENANT_ID.to_string(),
            device_id: device_id.to_string(),
            level: "INFO".to_string(),
            message: "Device registered and came online".to_string(),
        },
    );

    Some(true)
}

/// Orchestrate an OTA update: validate device/firmware compatibility, update
/// the device shadow desired state, and record the deployment.
///
/// All DB operations run in a single transaction. The Zenoh delta publish
/// happens after the transaction commits so the connection is not held across
/// the async boundary.
pub async fn trigger_ota(
    ctx: &RequestContext,
    pool: &DbPool,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    firmware_update_id: i32,
    public_url: &str,
    zenoh_metrics: &ZenohMetrics,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::DeployFirmware)?;

    let d_id = device_id.to_string();
    let d_id_for_publish = d_id.clone();
    let public_url = public_url.to_string();
    let tenant_id = ctx.tenant_id_str().to_string();

    let (delta, version) = run_db(pool, move |conn| {
        conn.transaction(|conn| {
            let device = device_repo::find_device_for_tenant(conn, &tenant_id, &d_id)?;
            let fw = firmware_repo::find_firmware_update(conn, &tenant_id, firmware_update_id)?;

            if fw.device_type_id != device.device_type_id {
                return Err(AppError::BadRequest(
                    "Firmware device type does not match device".into(),
                ));
            }

            // Build OTA patch and apply via shadow service (DB-only)
            use extrittio_common::ota::fields as ota_fields;

            let mut ota_payload = serde_json::json!({
                ota_fields::FIRMWARE_VERSION: fw.version,
                ota_fields::FIRMWARE_URL: firmware_download_url(&public_url, &fw.url),
                ota_fields::FIRMWARE_UPDATE_ID: fw.id,
            });
            if let Some(ref hash) = fw.sha256 {
                ota_payload[ota_fields::SHA256] = serde_json::Value::String(hash.clone());
            }

            let mut patch = serde_json::Map::new();
            patch.insert(ota_fields::SHADOW_KEY.to_string(), ota_payload);

            let (delta, version) =
                shadow_service::update_desired_db(conn, &tenant_id, &d_id, &patch)?;

            let deployment = NewOtaDeployment {
                tenant_id: tenant_id.clone(),
                device_id: d_id,
                firmware_update_id: fw.id,
            };
            firmware_repo::insert_ota_deployment(conn, &deployment)?;

            Ok((delta, version))
        })
    })
    .await?;

    shadow_service::publish_delta_if_nonempty(
        zenoh_session,
        &d_id_for_publish,
        &delta,
        version,
        zenoh_metrics,
    )
    .await;

    Ok(())
}

pub fn authorize_deploy_firmware(ctx: &RequestContext) -> Result<(), AppError> {
    policy::require(ctx, Permission::DeployFirmware)
}

fn firmware_download_url(public_url: &str, stored_url: &str) -> String {
    if stored_url.starts_with("http://") || stored_url.starts_with("https://") {
        return stored_url.to_string();
    }

    format!(
        "{}/{}",
        public_url.trim_end_matches('/'),
        stored_url.trim_start_matches('/')
    )
}

/// List devices with filtering and pagination.
pub fn list_devices(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<device_repo::DeviceWithJoins>, i64), AppError> {
    policy::require(ctx, Permission::ReadDevices)?;

    let (mut devices, total) = device_repo::list_devices(
        conn,
        ctx.tenant_id_str(),
        status_filter,
        search_filter,
        fleet_id_filter,
        limit,
        offset,
    )?;
    hydrate_declared_connections_from_telemetry(conn, &mut devices)?;
    hydrate_network_observed_hosts(conn, ctx.tenant_id_str(), &mut devices)?;
    Ok((devices, total))
}

/// Get a single device with joined type and fleet info.
pub fn get_device(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<device_repo::DeviceWithJoins, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;

    let device = device_repo::find_device_with_joins(conn, ctx.tenant_id_str(), device_id)?;
    let mut devices = vec![device];
    hydrate_declared_connections_from_telemetry(conn, &mut devices)?;
    hydrate_network_observed_hosts(conn, ctx.tenant_id_str(), &mut devices)?;
    Ok(devices.remove(0))
}

/// Update a device. Returns the updated device with joins.
pub fn update_device(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    changeset: &UpdateDevice,
) -> Result<device_repo::DeviceWithJoins, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;

    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id)?;
    device_repo::update_device(conn, ctx.tenant_id_str(), device_id, changeset)?;
    Ok(device_repo::find_device_with_joins(
        conn,
        ctx.tenant_id_str(),
        device_id,
    )?)
}

/// Delete a device by ID.
pub fn delete_device(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageDevices)?;

    let deleted = device_repo::delete_device(conn, ctx.tenant_id_str(), device_id)?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "Device '{device_id}' not found"
        )));
    }
    Ok(())
}

/// Resolve device IDs from explicit list or filters (for bulk operations).
///
/// When `select_all` is true, resolves IDs using the provided filters.
/// When `select_all` is false, uses the explicit `device_ids` (returns
/// `BadRequest` if `None`).
/// Returns `BadRequest` if the result exceeds `max_size`.
pub fn resolve_target_ids(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_ids: Option<&[String]>,
    select_all: bool,
    status_filter: Option<&str>,
    search_filter: Option<&str>,
    fleet_id_filter: Option<i32>,
    max_size: usize,
) -> Result<Vec<String>, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;

    let ids = if select_all {
        device_repo::resolve_device_ids(
            conn,
            ctx.tenant_id_str(),
            status_filter,
            search_filter,
            fleet_id_filter,
        )?
    } else {
        device_ids
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Either device_ids or select_all with filters is required".into(),
                )
            })?
            .to_vec()
    };

    if ids.len() > max_size {
        return Err(AppError::BadRequest(format!(
            "Too many devices ({}). Maximum is {max_size}. Narrow your filters.",
            ids.len()
        )));
    }

    Ok(ids)
}

/// Bulk-change fleet assignment.
pub fn bulk_change_fleet(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    ids: &[String],
    fleet_id: Option<i32>,
) -> Result<usize, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;

    let now = chrono::Utc::now().naive_utc();
    Ok(device_repo::bulk_update_fleet(
        conn,
        ctx.tenant_id_str(),
        ids,
        fleet_id,
        now,
    )?)
}

/// Bulk-delete devices.
pub fn bulk_delete(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    ids: &[String],
) -> Result<usize, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;

    Ok(device_repo::bulk_delete_devices(
        conn,
        ctx.tenant_id_str(),
        ids,
    )?)
}

/// List OTA deployments for a device with pagination.
/// Returns 404 if the device does not exist.
pub fn list_ota_deployments(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    device_id: &str,
    limit: i64,
    offset: i64,
) -> Result<
    (
        Vec<(
            crate::db::models::OtaDeployment,
            crate::db::models::FirmwareUpdate,
        )>,
        i64,
    ),
    AppError,
> {
    policy::require(ctx, Permission::ReadDevices)?;

    device_repo::find_device_for_tenant(conn, ctx.tenant_id_str(), device_id)?;
    Ok(firmware_repo::list_ota_deployments(
        conn,
        ctx.tenant_id_str(),
        device_id,
        limit,
        offset,
    )?)
}

/// Mark a pre-determined list of devices as offline and log the transition.
/// Use this when the caller has already identified which devices should go
/// offline (e.g. using a shared cutoff) to avoid TOCTOU races.
pub fn mark_devices_offline(
    conn: &mut PgConnection,
    device_ids: &[String],
) -> Result<usize, AppError> {
    if device_ids.is_empty() {
        return Ok(0);
    }

    use crate::db::schema::devices;
    use diesel::prelude::*;

    let count = diesel::update(
        devices::table.filter(
            devices::id
                .eq_any(device_ids)
                .and(devices::status.ne("offline")),
        ),
    )
    .set(devices::status.eq("offline"))
    .execute(conn)?;

    for device_id in device_ids {
        let message = "Device went offline (no heartbeat)".to_string();
        if let Err(e) = log_repo::insert_log(
            conn,
            &NewDeviceLog {
                tenant_id: DEFAULT_TENANT_ID.to_string(),
                device_id: device_id.clone(),
                level: "WARN".to_string(),
                message,
            },
        ) {
            tracing::warn!("Failed to insert offline log for {device_id}: {e}");
        }
    }

    Ok(count)
}

/// Mark devices as offline if they haven't been seen since timeout_secs.
pub fn check_offline_devices(
    conn: &mut PgConnection,
    timeout_secs: u64,
) -> Result<usize, AppError> {
    #[allow(clippy::cast_possible_wrap)]
    let cutoff = chrono::Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);

    let (going_offline, count) = conn.transaction::<_, diesel::result::Error, _>(|conn| {
        let going_offline = device_repo::find_devices_going_offline(conn, cutoff)?;
        let count = going_offline.len();
        if count > 0 {
            device_repo::mark_devices_offline(conn, cutoff)?;
        }
        Ok((going_offline, count))
    })?;

    for device_id in &going_offline {
        let message = format!("Device went offline (no heartbeat for {timeout_secs}s)");
        if let Err(e) = log_repo::insert_log(
            conn,
            &NewDeviceLog {
                tenant_id: DEFAULT_TENANT_ID.to_string(),
                device_id: device_id.clone(),
                level: "WARN".to_string(),
                message,
            },
        ) {
            tracing::warn!("Failed to insert offline log for {device_id}: {e}");
        }
    }

    Ok(count)
}

/// Update a device from a heartbeat message.
/// Returns `Some(StatusChange)` when the device's status actually changed,
/// or `None` when the status remained the same.
pub fn update_from_heartbeat(
    conn: &mut PgConnection,
    device_id: &str,
    reported_status: &str,
    firmware: &str,
    uptime_seconds: u64,
) -> Result<Option<crate::rule_engine::types::StatusChange>, AppError> {
    use extrittio_common::device_status;

    let status = if device_status::is_valid(reported_status) {
        reported_status.to_string()
    } else {
        tracing::warn!(
            "Invalid status '{}' from device {}, defaulting to '{}'",
            reported_status,
            device_id,
            device_status::ONLINE
        );
        device_status::ONLINE.to_string()
    };

    let previous_status = device_repo::find_device(conn, device_id)
        .ok()
        .map(|d| d.status);

    let now = chrono::Utc::now().naive_utc();
    #[allow(clippy::cast_possible_truncation)]
    let changeset = UpdateDevice {
        status: Some(status.clone()),
        firmware: Some(firmware.to_string()),
        uptime_seconds: Some(uptime_seconds as i32),
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };

    device_repo::update_device(conn, DEFAULT_TENANT_ID, device_id, &changeset)?;

    let status_change = if let Some(ref prev) = previous_status {
        if prev != &status {
            let message = format!("Device status changed from {prev} to {status}");
            let _ = log_repo::insert_log(
                conn,
                &NewDeviceLog {
                    tenant_id: DEFAULT_TENANT_ID.to_string(),
                    device_id: device_id.to_string(),
                    level: "INFO".to_string(),
                    message,
                },
            );
            Some(crate::rule_engine::types::StatusChange {
                old_status: prev.clone(),
                new_status: status,
            })
        } else {
            None
        }
    } else {
        None
    };

    Ok(status_change)
}

/// Format uptime seconds into a human-readable string.
pub fn format_uptime(seconds: Option<i32>) -> Option<String> {
    let secs = seconds? as i64;
    if secs <= 0 {
        return Some("0s".to_string());
    }
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{minutes}m"));
    }
    Some(parts.join(" "))
}

/// Format a last-seen timestamp into a relative string.
pub fn format_last_seen(last_seen: Option<chrono::NaiveDateTime>) -> Option<String> {
    let ts = last_seen?;
    let now = chrono::Utc::now().naive_utc();
    let secs = now.signed_duration_since(ts).num_seconds();
    if secs < 0 {
        return Some("just now".to_string());
    }
    let result = if secs < 60 {
        format!("{secs} seconds ago")
    } else if secs < 3600 {
        let m = secs / 60;
        format!("{m} minute{} ago", if m == 1 { "" } else { "s" })
    } else if secs < 86400 {
        let h = secs / 3600;
        format!("{h} hour{} ago", if h == 1 { "" } else { "s" })
    } else {
        let d = secs / 86400;
        format!("{d} day{} ago", if d == 1 { "" } else { "s" })
    };
    Some(result)
}
