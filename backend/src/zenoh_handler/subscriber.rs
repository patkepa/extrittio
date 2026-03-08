use chrono::Utc;
use diesel::prelude::*;
use prost::Message;
use tracing::{info, warn};

use crate::db::models::{Device, NewTelemetryRecord, UpdateDevice};
use crate::db::schema::{devices, telemetry};
use crate::state::DbPool;

use extrittio_proto::extrittio::{DeviceHeartbeat, DeviceTelemetry};

/// Start zenoh subscribers for telemetry and heartbeat topics.
///
/// Spawns one subscriber handler in a background tokio task and runs the other
/// in the current task. Both loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
pub async fn run_subscriber(
    session: &zenoh::Session,
    db_pool: DbPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let telemetry_sub = session
        .declare_subscriber("extrittio/devices/*/telemetry")
        .await?;

    let heartbeat_sub = session
        .declare_subscriber("extrittio/devices/*/heartbeat")
        .await?;

    info!("Zenoh subscribers declared for telemetry and heartbeat topics");

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

    let now = Utc::now().naive_utc();
    let changeset = UpdateDevice {
        status: Some(heartbeat_msg.status.clone()),
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
