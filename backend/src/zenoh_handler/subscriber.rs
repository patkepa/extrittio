use chrono::Utc;
use diesel::prelude::*;
use prost::Message;
use std::sync::Arc;
use tracing::{info, warn};

use crate::db::models::{Device, DeviceShadow, NewTelemetryRecord, UpdateDevice, UpdateShadow};
use crate::db::schema::{device_shadows, devices, telemetry};
use crate::state::DbPool;

use extrittio_proto::extrittio::{DeviceHeartbeat, DeviceTelemetry, ShadowDelta, ShadowGet, ShadowReport};

/// Start zenoh subscribers for telemetry and heartbeat topics.
///
/// Spawns one subscriber handler in a background tokio task and runs the other
/// in the current task. Both loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
pub async fn run_subscriber(
    session: Arc<zenoh::Session>,
    db_pool: DbPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let telemetry_sub = session
        .declare_subscriber("extrittio/devices/*/telemetry")
        .await?;

    let heartbeat_sub = session
        .declare_subscriber("extrittio/devices/*/heartbeat")
        .await?;

    let shadow_report_sub = session
        .declare_subscriber("extrittio/devices/*/shadow/report")
        .await?;

    let shadow_get_sub = session
        .declare_subscriber("extrittio/devices/*/shadow/get")
        .await?;

    info!("Zenoh subscribers declared for telemetry, heartbeat, and shadow topics");

    // Spawn heartbeat handler in a background task
    let heartbeat_pool = db_pool.clone();
    tokio::spawn(async move {
        loop {
            match heartbeat_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handle_heartbeat(&heartbeat_pool, &payload);
                }
                Err(e) => {
                    warn!("Heartbeat subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow report handler
    let shadow_report_pool = db_pool.clone();
    tokio::spawn(async move {
        loop {
            match shadow_report_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handle_shadow_report(&shadow_report_pool, &payload);
                }
                Err(e) => {
                    warn!("Shadow report subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow get handler
    let shadow_get_pool = db_pool.clone();
    let shadow_get_session = session.clone();
    tokio::spawn(async move {
        loop {
            match shadow_get_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handle_shadow_get(&shadow_get_pool, &shadow_get_session, &payload).await;
                }
                Err(e) => {
                    warn!("Shadow get subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Run telemetry handler in the current task
    loop {
        match telemetry_sub.recv_async().await {
            Ok(sample) => {
                let payload = sample.payload().to_bytes();
                handle_telemetry(&db_pool, &payload);
            }
            Err(e) => {
                warn!("Telemetry subscriber channel closed: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Decode a DeviceTelemetry protobuf message, insert a telemetry record, and
/// update the device's `last_seen` timestamp.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
fn handle_telemetry(db_pool: &DbPool, payload: &[u8]) {
    let telemetry_msg = match DeviceTelemetry::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceTelemetry: {}", e);
            return;
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    // Verify the device is registered
    let device_exists = devices::table
        .find(&telemetry_msg.device_id)
        .select(Device::as_select())
        .first(&mut conn)
        .optional();

    match device_exists {
        Ok(Some(_)) => {}
        Ok(None) => {
            warn!(
                "Dropping telemetry from unregistered device: {}",
                telemetry_msg.device_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

    // Build custom_json from metadata map (if non-empty)
    let custom_json = if telemetry_msg.metadata.is_empty() {
        None
    } else {
        match serde_json::to_string(&telemetry_msg.metadata) {
            Ok(json) => Some(json),
            Err(e) => {
                warn!("Failed to serialize metadata: {}", e);
                None
            }
        }
    };

    let new_record = NewTelemetryRecord {
        device_id: telemetry_msg.device_id.clone(),
        payload: payload.to_vec(),
        temperature: Some(telemetry_msg.temperature),
        humidity: Some(telemetry_msg.humidity),
        battery_level: Some(telemetry_msg.battery_level),
        custom_json,
    };

    if let Err(e) = diesel::insert_into(telemetry::table)
        .values(&new_record)
        .execute(&mut conn)
    {
        warn!("Failed to insert telemetry record: {}", e);
        return;
    }

    // Update device last_seen
    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };

    if let Err(e) = diesel::update(devices::table.find(&telemetry_msg.device_id))
        .set(&changeset)
        .execute(&mut conn)
    {
        warn!("Failed to update device last_seen: {}", e);
    }

    info!(
        "Recorded telemetry from device {}: temp={}, humidity={}, battery={}",
        telemetry_msg.device_id,
        telemetry_msg.temperature,
        telemetry_msg.humidity,
        telemetry_msg.battery_level
    );
}

/// Decode a DeviceHeartbeat protobuf message and update the device's status,
/// firmware, uptime, and `last_seen` timestamp.
///
/// Logs and drops messages from unregistered devices or malformed payloads.
fn handle_heartbeat(db_pool: &DbPool, payload: &[u8]) {
    let heartbeat_msg = match DeviceHeartbeat::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceHeartbeat: {}", e);
            return;
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    // Verify the device is registered
    let device_exists = devices::table
        .find(&heartbeat_msg.device_id)
        .select(Device::as_select())
        .first(&mut conn)
        .optional();

    match device_exists {
        Ok(Some(_)) => {}
        Ok(None) => {
            warn!(
                "Dropping heartbeat from unregistered device: {}",
                heartbeat_msg.device_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

    let valid_statuses = ["online", "offline", "warning"];
    let status = if valid_statuses.contains(&heartbeat_msg.status.as_str()) {
        heartbeat_msg.status.clone()
    } else {
        warn!("Invalid status '{}' from device {}, defaulting to 'online'", heartbeat_msg.status, heartbeat_msg.device_id);
        "online".to_string()
    };

    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        status: Some(status),
        firmware: Some(heartbeat_msg.firmware.clone()),
        uptime_seconds: Some(heartbeat_msg.uptime_seconds as i32),
        last_seen: Some(now),
        updated_at: Some(now),
        ..Default::default()
    };

    if let Err(e) = diesel::update(devices::table.find(&heartbeat_msg.device_id))
        .set(&changeset)
        .execute(&mut conn)
    {
        warn!("Failed to update device from heartbeat: {}", e);
        return;
    }

    info!(
        "Heartbeat from device {}: status={}, firmware={}, uptime={}s",
        heartbeat_msg.device_id,
        heartbeat_msg.status,
        heartbeat_msg.firmware,
        heartbeat_msg.uptime_seconds
    );
}

fn handle_shadow_report(db_pool: &DbPool, payload: &[u8]) {
    let report = match ShadowReport::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode ShadowReport: {}", e);
            return;
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    // Verify device exists
    let device_exists = devices::table
        .find(&report.device_id)
        .select(Device::as_select())
        .first(&mut conn)
        .optional();

    match device_exists {
        Ok(Some(_)) => {}
        Ok(None) => {
            warn!("Dropping shadow report from unregistered device: {}", report.device_id);
            return;
        }
        Err(e) => {
            warn!("DB error checking device: {}", e);
            return;
        }
    }

    // Parse incoming reported state
    let new_reported: serde_json::Value = match serde_json::from_str(&report.state_json) {
        Ok(v) => v,
        Err(e) => {
            warn!("Invalid JSON in ShadowReport: {}", e);
            return;
        }
    };

    // Read current shadow
    let shadow: DeviceShadow = match device_shadows::table
        .find(&report.device_id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
    {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to read shadow for device {}: {}", report.device_id, e);
            return;
        }
    };

    let current_desired: serde_json::Value = serde_json::from_str(&shadow.desired).unwrap_or_default();

    // Merge reported state
    let current_reported: serde_json::Value = serde_json::from_str(&shadow.reported).unwrap_or_default();
    let merged_reported = if let Some(new_obj) = new_reported.as_object() {
        let mut obj = current_reported.as_object().cloned().unwrap_or_default();
        for (key, val) in new_obj {
            if val.is_null() {
                obj.remove(key);
            } else {
                obj.insert(key.clone(), val.clone());
            }
        }
        serde_json::Value::Object(obj)
    } else {
        new_reported
    };

    // Compute delta
    let new_delta = compute_shadow_delta(&current_desired, &merged_reported);
    let now = chrono::Utc::now().naive_utc();

    let changeset = UpdateShadow {
        reported: Some(serde_json::to_string(&merged_reported).unwrap()),
        delta: Some(serde_json::to_string(&new_delta).unwrap()),
        version: Some(shadow.version + 1),
        updated_at: Some(now),
        ..Default::default()
    };

    if let Err(e) = diesel::update(device_shadows::table.find(&report.device_id))
        .set(&changeset)
        .execute(&mut conn)
    {
        warn!("Failed to update shadow: {}", e);
        return;
    }

    info!("Shadow report from device {}: version={}", report.device_id, shadow.version + 1);
}

async fn handle_shadow_get(db_pool: &DbPool, session: &zenoh::Session, payload: &[u8]) {
    let get_msg = match ShadowGet::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode ShadowGet: {}", e);
            return;
        }
    };

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    let shadow: DeviceShadow = match device_shadows::table
        .find(&get_msg.device_id)
        .select(DeviceShadow::as_select())
        .first(&mut conn)
    {
        Ok(s) => s,
        Err(e) => {
            warn!("Shadow not found for device {}: {}", get_msg.device_id, e);
            return;
        }
    };

    // Only send delta if non-empty
    let delta: serde_json::Value = serde_json::from_str(&shadow.delta).unwrap_or_default();
    if let Some(obj) = delta.as_object() {
        if obj.is_empty() {
            info!("Shadow get from device {}: already in sync", get_msg.device_id);
            return;
        }
    }

    let delta_msg = ShadowDelta {
        device_id: get_msg.device_id.clone(),
        delta_json: shadow.delta,
        version: shadow.version as i64,
    };

    let response_payload = prost::Message::encode_to_vec(&delta_msg);
    let topic = format!("extrittio/devices/{}/shadow/delta", get_msg.device_id);

    if let Err(e) = session.put(&topic, response_payload).await {
        warn!("Failed to publish shadow delta: {}", e);
    }

    info!("Shadow get from device {}: sent delta", get_msg.device_id);
}

fn compute_shadow_delta(desired: &serde_json::Value, reported: &serde_json::Value) -> serde_json::Value {
    let desired_obj = desired.as_object();
    let reported_obj = reported.as_object();

    match (desired_obj, reported_obj) {
        (Some(d), Some(r)) => {
            let mut delta = serde_json::Map::new();
            for (key, val) in d {
                match r.get(key) {
                    Some(reported_val) if reported_val == val => {}
                    _ => {
                        delta.insert(key.clone(), val.clone());
                    }
                }
            }
            serde_json::Value::Object(delta)
        }
        _ => desired.clone(),
    }
}
