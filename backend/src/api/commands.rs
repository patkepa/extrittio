use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use diesel::prelude::*;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::models::{CommandRecord, Device, NewCommandRecord};
use crate::db::schema::{command_history, devices};
use crate::error::AppError;
use crate::state::AppState;
use extrittio_proto::extrittio::DeviceCommand;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SendCommandRequest {
    pub command: String,
    pub params: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub id: String,
    pub device_id: String,
    pub command: String,
    pub params: serde_json::Value,
    pub status: String,
    pub response_payload: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CommandsQuery {
    pub limit: Option<i64>,
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_command_response(record: CommandRecord) -> CommandResponse {
    CommandResponse {
        id: record.id,
        device_id: record.device_id,
        command: record.command,
        params: serde_json::from_str(&record.params).unwrap_or_default(),
        status: record.status,
        response_payload: record
            .response_payload
            .and_then(|s| serde_json::from_str(&s).ok()),
        created_at: record.created_at.to_string(),
        updated_at: record.updated_at.to_string(),
    }
}

/// Shared internal logic for sending a command to a device.
/// Used by both the generic POST endpoint and the legacy restart endpoint.
///
/// # Errors
///
/// Returns `AppError::NotFound` if the device does not exist, or other
/// `AppError` variants on database/Zenoh failures.
#[allow(clippy::implicit_hasher)]
pub async fn send_command_internal(
    state: &AppState,
    device_id: &str,
    command: &str,
    params: HashMap<String, String>,
) -> Result<CommandRecord, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    devices::table
        .find(device_id)
        .select(Device::as_select())
        .first(&mut conn)?;

    let correlation_id = uuid::Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string());

    // Insert command record
    let new_record = NewCommandRecord {
        id: correlation_id.clone(),
        device_id: device_id.to_string(),
        command: command.to_string(),
        params: params_json,
    };

    diesel::insert_into(command_history::table)
        .values(&new_record)
        .execute(&mut conn)?;

    // Build and publish protobuf
    let proto_command = DeviceCommand {
        command: command.to_string(),
        params,
        correlation_id: correlation_id.clone(),
    };

    let payload = proto_command.encode_to_vec();
    let topic = format!("extrittio/devices/{device_id}/commands");

    state
        .zenoh_session
        .put(&topic, payload)
        .await
        .map_err(|e| AppError::Zenoh(e.to_string()))?;

    // Re-read the record to get the DB-generated timestamps
    let record = command_history::table
        .find(&correlation_id)
        .select(CommandRecord::as_select())
        .first(&mut conn)?;

    Ok(record)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/devices/{id}/commands",
        get(list_commands).post(send_command),
    )
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn send_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SendCommandRequest>,
) -> Result<(StatusCode, Json<CommandResponse>), AppError> {
    let params = body.params.unwrap_or_default();
    let record = send_command_internal(&state, &id, &body.command, params).await?;
    Ok((StatusCode::CREATED, Json(to_command_response(record))))
}

async fn list_commands(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<CommandsQuery>,
) -> Result<Json<Vec<CommandResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    devices::table
        .find(&id)
        .select(devices::id)
        .first::<String>(&mut conn)?;

    let limit = params.limit.unwrap_or(50).min(500);

    let mut query = command_history::table
        .filter(command_history::device_id.eq(&id))
        .into_boxed();

    if let Some(ref status) = params.status {
        query = query.filter(command_history::status.eq(status));
    }

    let records: Vec<CommandRecord> = query
        .order(command_history::created_at.desc())
        .limit(limit)
        .select(CommandRecord::as_select())
        .load(&mut conn)?;

    Ok(Json(records.into_iter().map(to_command_response).collect()))
}
