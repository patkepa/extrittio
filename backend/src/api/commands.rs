use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::db::models::CommandRecord;
use crate::error::AppError;
use crate::repositories::{command_repo, device_repo};
use crate::services::command_service;
use crate::state::{AppState, run_db};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendCommandRequest {
    pub command: String,
    pub params: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CommandResponse {
    pub id: String,
    pub device_id: String,
    pub command: String,
    #[schema(value_type = HashMap<String, Value>)]
    pub params: serde_json::Value,
    pub status: String,
    #[schema(value_type = Option<HashMap<String, Value>>)]
    pub response_payload: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct CommandsQuery {
    /// Maximum number of records to return (default 50, max 500).
    pub limit: Option<i64>,
    /// Filter by command status.
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

/// Send a command to a device.
#[utoipa::path(
    post,
    path = "/api/v1/devices/{id}/commands",
    tag = "commands",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Device ID")),
    request_body = SendCommandRequest,
    responses(
        (status = 201, description = "Command sent", body = CommandResponse),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn send_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SendCommandRequest>,
) -> Result<(StatusCode, Json<CommandResponse>), AppError> {
    if body.command.trim().is_empty() {
        return Err(AppError::BadRequest("Command must not be empty".into()));
    }

    let params = body.params.unwrap_or_default();
    let record = command_service::send_command(
        &state.db_pool,
        &state.zenoh_session,
        &id,
        &body.command,
        params,
        &state.zenoh_metrics,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(to_command_response(record))))
}

/// List commands for a device.
#[utoipa::path(
    get,
    path = "/api/v1/devices/{id}/commands",
    tag = "commands",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Device ID"),
        CommandsQuery,
    ),
    responses(
        (status = 200, description = "List of commands", body = Vec<CommandResponse>),
        (status = 404, description = "Device not found"),
    ),
)]
pub(crate) async fn list_commands(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<CommandsQuery>,
) -> Result<Json<Vec<CommandResponse>>, AppError> {
    let response = run_db(&state.db_pool, move |conn| {
        // Verify device exists
        if !device_repo::device_exists(conn, &id)? {
            return Err(AppError::NotFound(format!("Device '{id}' not found")));
        }

        let limit = params.limit.unwrap_or(50).min(500);

        let records = command_repo::list_commands(conn, &id, params.status.as_deref(), limit)?;

        Ok(records.into_iter().map(to_command_response).collect())
    })
    .await?;

    Ok(Json(response))
}
