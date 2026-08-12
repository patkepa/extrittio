//! Host-local OpenThread Border Router controls for the hobby appliance.
//!
//! Thread credentials never enter persistence and are intentionally omitted
//! from responses and audit metadata. They are passed once to the local OTBR
//! controller and retained by OpenThread's own operational dataset storage.

use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::State,
    routing::{get, post, put},
};
use extrittio_openthread_runtime::{CreateNetwork, ThreadController, ThreadNetwork, ThreadStatus};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{auth::context::RequestContext, error::AppError, state::AppState};

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadStatusResponse {
    /// Whether this process is the hobby appliance with a controllable OTBR.
    pub available: bool,
    pub connected: bool,
    pub error: Option<String>,
    pub role: Option<String>,
    pub network_name: Option<String>,
    pub channel: Option<u16>,
    pub pan_id: Option<String>,
    pub extended_pan_id: Option<String>,
    pub mesh_local_prefix: Option<String>,
    pub addresses: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateThreadNetworkRequest {
    pub network_name: String,
    pub channel: Option<u16>,
    pub pan_id: Option<String>,
    pub extended_pan_id: Option<String>,
    /// Optional 16-byte Thread network key, as 32 hexadecimal characters.
    /// Omit it to have OpenThread generate a secure random key.
    #[schema(write_only = true)]
    pub network_key: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportThreadDatasetRequest {
    /// Complete hex-encoded Active Operational Dataset TLVs. This value is
    /// write-only because it contains the Thread network key.
    #[schema(write_only = true)]
    pub active_dataset_tlvs: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadNetworkResponse {
    pub pan_id: String,
    pub extended_address: String,
    pub channel: u16,
    pub rssi: i16,
    pub lqi: u8,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadNetworkScanResponse {
    pub networks: Vec<ThreadNetworkResponse>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/thread", get(get_thread_status))
        .route("/api/v1/system/thread/scan", post(scan_thread_networks))
        .route("/api/v1/system/thread/network", post(create_thread_network))
        .route("/api/v1/system/thread/dataset", put(import_thread_dataset))
}

#[utoipa::path(
    post,
    path = "/api/v1/system/thread/scan",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Nearby Thread networks discovered by the local radio", body = ThreadNetworkScanResponse),
        (status = 403, description = "Owner access required"),
        (status = 409, description = "Thread is unavailable"),
    ),
)]
pub(crate) async fn scan_thread_networks(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ThreadNetworkScanResponse>, AppError> {
    require_owner(&ctx)?;
    let controller = controller(&state)?;
    let networks = run_blocking(controller, |controller| controller.scan_networks()).await?;
    Ok(Json(ThreadNetworkScanResponse {
        networks: networks
            .into_iter()
            .map(ThreadNetworkResponse::from)
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/system/thread",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Local Thread border-router status", body = ThreadStatusResponse),
        (status = 403, description = "Owner access required"),
    ),
)]
pub(crate) async fn get_thread_status(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ThreadStatusResponse>, AppError> {
    require_owner(&ctx)?;
    let Some(controller) = state.thread_controller.clone() else {
        return Ok(Json(ThreadStatusResponse::unavailable()));
    };

    let response = match run_blocking(controller, |controller| controller.status()).await {
        Ok(status) => ThreadStatusResponse::connected(status),
        Err(error) => ThreadStatusResponse::failed(error),
    };
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/system/thread/network",
    tag = "system",
    security(("bearer_auth" = [])),
    request_body = CreateThreadNetworkRequest,
    responses(
        (status = 200, description = "New Thread network formed", body = ThreadStatusResponse),
        (status = 400, description = "Invalid network configuration"),
        (status = 403, description = "Owner access required"),
        (status = 409, description = "Thread is unavailable"),
    ),
)]
pub(crate) async fn create_thread_network(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateThreadNetworkRequest>,
) -> Result<Json<ThreadStatusResponse>, AppError> {
    require_owner(&ctx)?;
    let controller = controller(&state)?;
    let network = CreateNetwork {
        network_name: request.network_name,
        channel: request.channel,
        pan_id: request.pan_id,
        extended_pan_id: request.extended_pan_id,
        network_key: request.network_key,
    };
    run_blocking(controller.clone(), move |controller| {
        controller.create_network(&network)
    })
    .await?;
    Ok(Json(status_after_change(controller).await?))
}

#[utoipa::path(
    put,
    path = "/api/v1/system/thread/dataset",
    tag = "system",
    security(("bearer_auth" = [])),
    request_body = ImportThreadDatasetRequest,
    responses(
        (status = 200, description = "Thread dataset imported", body = ThreadStatusResponse),
        (status = 400, description = "Invalid operational dataset"),
        (status = 403, description = "Owner access required"),
        (status = 409, description = "Thread is unavailable"),
    ),
)]
pub(crate) async fn import_thread_dataset(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<ImportThreadDatasetRequest>,
) -> Result<Json<ThreadStatusResponse>, AppError> {
    require_owner(&ctx)?;
    let controller = controller(&state)?;
    run_blocking(controller.clone(), move |controller| {
        controller.import_active_dataset(&request.active_dataset_tlvs)
    })
    .await?;
    Ok(Json(status_after_change(controller).await?))
}

impl ThreadStatusResponse {
    fn unavailable() -> Self {
        Self {
            available: false,
            connected: false,
            error: None,
            role: None,
            network_name: None,
            channel: None,
            pan_id: None,
            extended_pan_id: None,
            mesh_local_prefix: None,
            addresses: Vec::new(),
        }
    }

    fn connected(status: ThreadStatus) -> Self {
        Self {
            available: true,
            connected: true,
            error: None,
            role: status.role,
            network_name: status.network_name,
            channel: status.channel,
            pan_id: status.pan_id,
            extended_pan_id: status.extended_pan_id,
            mesh_local_prefix: status.mesh_local_prefix,
            addresses: status.addresses,
        }
    }

    fn failed(error: AppError) -> Self {
        Self {
            available: true,
            connected: false,
            error: Some(error.to_string()),
            ..Self::unavailable()
        }
    }
}

impl From<ThreadNetwork> for ThreadNetworkResponse {
    fn from(network: ThreadNetwork) -> Self {
        Self {
            pan_id: network.pan_id,
            extended_address: network.extended_address,
            channel: network.channel,
            rssi: network.rssi,
            lqi: network.lqi,
        }
    }
}

fn require_owner(ctx: &RequestContext) -> Result<(), AppError> {
    if ctx.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Only the appliance owner can configure Thread".to_string(),
        ))
    }
}

fn controller(state: &AppState) -> Result<Arc<ThreadController>, AppError> {
    state.thread_controller.clone().ok_or_else(|| {
        AppError::Conflict(
            "The local OpenThread border router is unavailable. Connect an RCP and start the hobby appliance with Thread enabled."
                .to_string(),
        )
    })
}

async fn status_after_change(
    controller: Arc<ThreadController>,
) -> Result<ThreadStatusResponse, AppError> {
    let status = run_blocking(controller, |controller| controller.status()).await?;
    Ok(ThreadStatusResponse::connected(status))
}

async fn run_blocking<T, F>(controller: Arc<ThreadController>, operation: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(Arc<ThreadController>) -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || operation(controller))
        .await
        .map_err(|error| AppError::Internal(format!("Thread control task failed: {error}")))?
        .map_err(|error| AppError::BadRequest(error.to_string()))
}
