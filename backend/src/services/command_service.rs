// Command service — business logic for device commands

use diesel::SqliteConnection;
use prost::Message;
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::models::{CommandRecord, NewCommandRecord};
use crate::error::AppError;
use crate::repositories::{command_repo, device_repo};
use extrittio_proto::extrittio::DeviceCommand;

/// Send a command to a device: verify it exists, persist the record, publish via
/// Zenoh, then return the persisted record (with DB-generated timestamps).
///
/// # Errors
///
/// Returns `AppError::NotFound` if the device does not exist, or other
/// `AppError` variants on database/Zenoh failures.
#[allow(clippy::implicit_hasher)]
pub async fn send_command(
    conn: &mut SqliteConnection,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    command: &str,
    params: HashMap<String, String>,
) -> Result<CommandRecord, AppError> {
    // Verify device exists
    device_repo::find_device(conn, device_id)?;

    let correlation_id = uuid::Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string());

    let new_record = NewCommandRecord {
        id: correlation_id.clone(),
        device_id: device_id.to_string(),
        command: command.to_string(),
        params: params_json,
    };

    command_repo::insert_command(conn, &new_record)?;

    // Build and publish protobuf
    let proto_command = DeviceCommand {
        command: command.to_string(),
        params,
        correlation_id: correlation_id.clone(),
    };

    let payload = proto_command.encode_to_vec();
    let topic = format!("extrittio/devices/{device_id}/commands");

    zenoh_session
        .put(&topic, payload)
        .await
        .map_err(|e| AppError::Zenoh(e.to_string()))?;

    // Re-read the record to get the DB-generated timestamps
    let record = command_repo::find_command(conn, &correlation_id)?;
    Ok(record)
}
