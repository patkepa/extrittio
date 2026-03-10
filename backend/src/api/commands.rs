use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::db::models::CommandRecord;
use crate::error::AppError;
use crate::repositories::{command_repo, device_repo};
use crate::services::command_service;
use crate::state::AppState;

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

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/devices/{id}/commands",
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
    let mut conn = state.db_pool.get()?;
    let record = command_service::send_command(&mut conn, &state.zenoh_session, &id, &body.command, params).await?;
    Ok((StatusCode::CREATED, Json(to_command_response(record))))
}

async fn list_commands(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<CommandsQuery>,
) -> Result<Json<Vec<CommandResponse>>, AppError> {
    let mut conn = state.db_pool.get()?;

    // Verify device exists
    device_repo::device_exists(&mut conn, &id)?;

    let limit = params.limit.unwrap_or(50).min(500);

    let records = command_repo::list_commands(
        &mut conn,
        &id,
        params.status.as_deref(),
        limit,
    )?;

    Ok(Json(records.into_iter().map(to_command_response).collect()))
}
