use chrono::Utc;
use diesel::prelude::*;
use prost::Message;
use tracing::{info, warn};

use crate::db::schema::command_history;
use crate::repositories::command_repo;
use crate::state::DbPool;

use extrittio_proto::extrittio::DeviceCommandResponse;

/// Decode a `DeviceCommandResponse` protobuf message and update the corresponding
/// command record's status and response payload.
pub fn handle_command_response(db_pool: &DbPool, payload: &[u8]) {
    let response = match DeviceCommandResponse::decode(payload) {
        Ok(msg) => msg,
        Err(e) => {
            warn!("Failed to decode DeviceCommandResponse: {}", e);
            return;
        }
    };

    if response.correlation_id.is_empty() {
        warn!("Received command response with empty correlation_id");
        return;
    }

    let mut conn = match db_pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get DB connection: {}", e);
            return;
        }
    };

    // Look up the command record by correlation_id
    let record = match command_repo::find_command(&mut conn, &response.correlation_id) {
        Ok(r) => r,
        Err(diesel::result::Error::NotFound) => {
            warn!(
                "No command record found for correlation_id: {}",
                response.correlation_id
            );
            return;
        }
        Err(e) => {
            warn!("DB error looking up command record: {}", e);
            return;
        }
    };

    // Verify device_id matches
    if record.device_id != response.device_id {
        warn!(
            "Command response device_id mismatch: expected {}, got {}",
            record.device_id, response.device_id
        );
        return;
    }

    // Only update if command is still in a non-terminal state
    let terminal_states = ["succeeded", "failed", "timed_out"];
    if terminal_states.contains(&record.status.as_str()) {
        info!(
            "Command {} already in terminal state '{}', ignoring response",
            response.correlation_id, record.status
        );
        return;
    }

    // Map response status
    let new_status = match response.status.as_str() {
        "ack" => "delivered",
        "succeeded" => "succeeded",
        "failed" => "failed",
        other => {
            warn!(
                "Unknown command response status '{}', treating as 'delivered'",
                other
            );
            "delivered"
        }
    };

    let response_payload = if response.payload.is_empty() {
        None
    } else {
        Some(response.payload)
    };

    let now = Utc::now().naive_utc();

    if let Err(e) = diesel::update(command_history::table.find(&response.correlation_id))
        .set((
            command_history::status.eq(new_status),
            command_history::response_payload.eq(&response_payload),
            command_history::updated_at.eq(now),
        ))
        .execute(&mut conn)
    {
        warn!("Failed to update command record: {}", e);
        return;
    }

    info!(
        "Command {} for device {} -> {}",
        response.correlation_id, response.device_id, new_status
    );
}
