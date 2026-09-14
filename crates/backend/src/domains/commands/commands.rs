use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::auth::context::RequestContext;
use crate::domains::commands::types::{CommandQuery, CommandRecord};
use crate::error::AppError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendCommandRequest {
    pub command: String,
    /// JSON input validated against the selected command's blueprint schema.
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CommandResponse {
    pub id: String,
    pub device_id: String,
    pub command: String,
    #[schema(value_type = Object)]
    pub params: serde_json::Value,
    pub status: String,
    #[schema(value_type = Option<Object>)]
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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<SendCommandRequest>,
) -> Result<(StatusCode, Json<CommandResponse>), AppError> {
    let params = body.params.unwrap_or_else(|| serde_json::json!({}));
    let record = state
        .application()
        .commands()
        .send(&ctx.tenant_context(), &id, &body.command, params)
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
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(params): Query<CommandsQuery>,
) -> Result<Json<Vec<CommandResponse>>, AppError> {
    let records = state
        .application()
        .commands()
        .list(
            &ctx.tenant_context(),
            &id,
            CommandQuery {
                limit: params.limit.unwrap_or(50).min(500),
                status: params.status,
            },
        )
        .await?;
    let response = records.into_iter().map(to_command_response).collect();

    Ok(Json(response))
}
