//! Host-local OpenThread Border Router controls for the hobby appliance.
//!
//! Thread credentials never enter persistence or audit metadata. The active
//! dataset is returned only from an owner-authenticated, explicitly requested
//! endpoint and is retained by OpenThread's own operational dataset storage.

use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    routing::{get, post},
};
use chrono::{DateTime, SecondsFormat, Utc};
use extrittio_openthread_runtime::{
    CreateNetwork, ThreadActiveDataset, ThreadChannelDiagnostics, ThreadController,
    ThreadMeshDevice, ThreadNetwork, ThreadRadioStatistics, ThreadRuntime, ThreadRuntimeSnapshot,
    ThreadScan, ThreadScanSnapshot, ThreadStatus,
};
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;

use crate::{auth::context::RequestContext, error::AppError, state::AppState};

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadStatusResponse {
    /// Whether this process is the hobby appliance with a controllable OTBR.
    pub available: bool,
    pub connected: bool,
    pub error: Option<String>,
    pub rcp_device: Option<String>,
    /// Serial devices currently detected as plausible Thread RCPs.
    pub available_rcp_devices: Vec<String>,
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
pub struct ThreadDatasetResponse {
    /// Complete hex-encoded Active Operational Dataset TLVs. Scanning or
    /// importing this value can grant a device access to the Thread network.
    pub active_dataset_tlvs: String,
    /// Thread Network Key extracted from the active dataset, when present.
    pub network_key: Option<String>,
    /// Pre-Shared Key for the Commissioner extracted from the active dataset,
    /// when present.
    pub pskc: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadNetworkResponse {
    pub network_name: Option<String>,
    pub pan_id: String,
    pub extended_address: String,
    pub channel: u16,
    pub rssi: i16,
    pub lqi: u8,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadChannelDiagnosticsResponse {
    pub channel: u16,
    /// Percentage of channel-monitor RSSI samples above OpenThread's noise threshold.
    pub utilization_percent: Option<f64>,
    /// Maximum energy observed during this scan, in dBm.
    pub max_rssi_dbm: Option<i16>,
    pub network_count: usize,
    pub strongest_network_rssi_dbm: Option<i16>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadRadioStatisticsResponse {
    pub cca_failure_rate_percent: Option<f64>,
    pub latest_rssi_dbm: Option<i16>,
    pub monitor_sample_count: Option<u32>,
    pub tx_total: Option<u32>,
    pub rx_total: Option<u32>,
    pub tx_retries: Option<u32>,
    pub tx_errors: Option<u32>,
    pub rx_errors: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadNetworkDiagnosticsResponse {
    pub scanning: bool,
    pub scanned_at: Option<String>,
    pub error: Option<String>,
    pub channels: Vec<ThreadChannelDiagnosticsResponse>,
    pub networks: Vec<ThreadNetworkResponse>,
    pub devices: Vec<ThreadMeshDeviceResponse>,
    pub statistics: ThreadRadioStatisticsResponse,
    /// Measurements unsupported by the current OTBR/RCP combination.
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadMeshDeviceResponse {
    pub id: String,
    pub is_border_router: bool,
    pub extended_address: Option<String>,
    pub mesh_local_eid_iid: Option<String>,
    pub omr_ipv6_addresses: Vec<String>,
    pub hostname: Option<String>,
    pub eui64: Option<String>,
    pub role: Option<String>,
    pub full_thread_device: Option<bool>,
    pub rx_on_when_idle: Option<bool>,
    pub full_network_data: Option<bool>,
    pub rloc16: Option<String>,
    pub rloc_address: Option<String>,
    pub router_id: Option<u16>,
    pub router_count: Option<u16>,
    pub network_name: Option<String>,
    pub extended_pan_id: Option<String>,
    pub border_agent_id: Option<String>,
    pub border_agent_state: Option<String>,
    pub partition_id: Option<u32>,
    pub leader_router_id: Option<u16>,
    pub data_version: Option<u16>,
    pub stable_data_version: Option<u16>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/system/thread", get(get_thread_status))
        .route(
            "/api/v1/system/thread/refresh",
            post(refresh_thread_runtime),
        )
        .route(
            "/api/v1/system/thread/scan",
            get(get_thread_scan).post(force_thread_scan),
        )
        .route("/api/v1/system/thread/radio/scan", post(force_thread_scan))
        .route("/api/v1/system/thread/mesh/scan", post(force_thread_scan))
        .route("/api/v1/system/thread/network", post(create_thread_network))
        .route(
            "/api/v1/system/thread/dataset",
            get(get_thread_dataset).put(import_thread_dataset),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/system/thread/scan",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Latest shared OpenThread topology and radio scan", body = ThreadNetworkDiagnosticsResponse),
        (status = 403, description = "Owner access required"),
    ),
)]
pub(crate) async fn get_thread_scan(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ThreadNetworkDiagnosticsResponse>, AppError> {
    require_owner(&ctx)?;
    let Some(runtime) = state.thread_runtime.as_ref() else {
        return Ok(Json(ThreadNetworkDiagnosticsResponse::empty(Some(
            "The local OpenThread border router is unavailable".to_string(),
        ))));
    };
    Ok(Json(ThreadNetworkDiagnosticsResponse::from(
        runtime.scan_snapshot(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/v1/system/thread/scan",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Refreshed shared OpenThread topology and radio scan", body = ThreadNetworkDiagnosticsResponse),
        (status = 403, description = "Owner access required"),
        (status = 409, description = "Thread is unavailable"),
    ),
)]
pub(crate) async fn force_thread_scan(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ThreadNetworkDiagnosticsResponse>, AppError> {
    require_owner(&ctx)?;
    controller(&state)?;
    let runtime = state
        .thread_runtime
        .clone()
        .expect("a Thread controller requires a Thread runtime");
    let snapshot = tokio::task::spawn_blocking(move || {
        runtime.refresh_scan(std::time::Duration::from_secs(60), true)
    })
    .await
    .map_err(|error| AppError::Internal(format!("Thread scan task failed: {error}")))?
    .map_err(|error| AppError::BadRequest(error.to_string()))?;
    Ok(Json(ThreadNetworkDiagnosticsResponse::from(snapshot)))
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
    let Some(runtime) = state.thread_runtime.clone() else {
        return Ok(Json(ThreadStatusResponse::unavailable()));
    };

    Ok(Json(thread_status(runtime).await))
}

#[utoipa::path(
    post,
    path = "/api/v1/system/thread/refresh",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Refreshed local Thread border-router status", body = ThreadStatusResponse),
        (status = 403, description = "Owner access required"),
    ),
)]
pub(crate) async fn refresh_thread_runtime(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ThreadStatusResponse>, AppError> {
    require_owner(&ctx)?;
    let Some(runtime) = state.thread_runtime.clone() else {
        return Ok(Json(ThreadStatusResponse::unavailable()));
    };
    let refreshed_runtime = runtime.clone();
    tokio::task::spawn_blocking(move || refreshed_runtime.refresh())
        .await
        .map_err(|error| AppError::Internal(format!("Thread refresh task failed: {error}")))?;
    Ok(Json(thread_status(runtime).await))
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
    let runtime = state
        .thread_runtime
        .clone()
        .expect("a Thread controller requires a Thread runtime");
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
    runtime.mark_network_changed();
    Ok(Json(status_after_change(runtime, controller).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/system/thread/dataset",
    tag = "system",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Active Thread operational dataset and credentials", body = ThreadDatasetResponse),
        (status = 403, description = "Owner access required"),
        (status = 409, description = "Thread is unavailable"),
    ),
)]
pub(crate) async fn get_thread_dataset(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
) -> Result<(HeaderMap, Json<ThreadDatasetResponse>), AppError> {
    require_owner(&ctx)?;
    let dataset = run_blocking(controller(&state)?, |controller| {
        controller.active_dataset()
    })
    .await?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    Ok((headers, Json(ThreadDatasetResponse::from(dataset))))
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
    let runtime = state
        .thread_runtime
        .clone()
        .expect("a Thread controller requires a Thread runtime");
    run_blocking(controller.clone(), move |controller| {
        controller.import_active_dataset(&request.active_dataset_tlvs)
    })
    .await?;
    runtime.mark_network_changed();
    Ok(Json(status_after_change(runtime, controller).await?))
}

impl ThreadStatusResponse {
    fn unavailable() -> Self {
        Self {
            available: false,
            connected: false,
            error: None,
            rcp_device: None,
            available_rcp_devices: Vec::new(),
            role: None,
            network_name: None,
            channel: None,
            pan_id: None,
            extended_pan_id: None,
            mesh_local_prefix: None,
            addresses: Vec::new(),
        }
    }

    fn unavailable_runtime(
        snapshot: ThreadRuntimeSnapshot,
        available_rcp_devices: Vec<String>,
    ) -> Self {
        Self {
            error: snapshot.message,
            rcp_device: snapshot
                .rcp_device
                .map(|device| device.display().to_string()),
            available_rcp_devices,
            ..Self::unavailable()
        }
    }

    fn connected(
        status: ThreadStatus,
        rcp_device: Option<String>,
        available_rcp_devices: Vec<String>,
    ) -> Self {
        Self {
            available: true,
            connected: true,
            error: None,
            rcp_device,
            available_rcp_devices,
            role: status.role,
            network_name: status.network_name,
            channel: status.channel,
            pan_id: status.pan_id,
            extended_pan_id: status.extended_pan_id,
            mesh_local_prefix: status.mesh_local_prefix,
            addresses: status.addresses,
        }
    }

    fn failed(
        error: AppError,
        rcp_device: Option<String>,
        available_rcp_devices: Vec<String>,
    ) -> Self {
        Self {
            available: true,
            connected: false,
            error: Some(error.to_string()),
            rcp_device,
            available_rcp_devices,
            ..Self::unavailable()
        }
    }
}

impl From<ThreadNetwork> for ThreadNetworkResponse {
    fn from(network: ThreadNetwork) -> Self {
        Self {
            network_name: network.network_name,
            pan_id: network.pan_id,
            extended_address: network.extended_address,
            channel: network.channel,
            rssi: network.rssi,
            lqi: network.lqi,
        }
    }
}

impl From<ThreadActiveDataset> for ThreadDatasetResponse {
    fn from(dataset: ThreadActiveDataset) -> Self {
        Self {
            active_dataset_tlvs: dataset.active_dataset_tlvs,
            network_key: dataset.network_key,
            pskc: dataset.pskc,
        }
    }
}

impl From<ThreadChannelDiagnostics> for ThreadChannelDiagnosticsResponse {
    fn from(channel: ThreadChannelDiagnostics) -> Self {
        Self {
            channel: channel.channel,
            utilization_percent: channel.occupancy.map(thread_ratio_percent),
            max_rssi_dbm: channel.max_rssi,
            network_count: channel.network_count,
            strongest_network_rssi_dbm: channel.strongest_network_rssi,
        }
    }
}

impl From<ThreadRadioStatistics> for ThreadRadioStatisticsResponse {
    fn from(statistics: ThreadRadioStatistics) -> Self {
        Self {
            cca_failure_rate_percent: statistics.cca_failure_rate.map(thread_ratio_percent),
            latest_rssi_dbm: statistics.latest_rssi,
            monitor_sample_count: statistics.monitor_sample_count,
            tx_total: statistics.tx_total,
            rx_total: statistics.rx_total,
            tx_retries: statistics.tx_retries,
            tx_errors: statistics.tx_errors,
            rx_errors: statistics.rx_errors,
        }
    }
}

impl ThreadRadioStatisticsResponse {
    fn empty() -> Self {
        Self {
            cca_failure_rate_percent: None,
            latest_rssi_dbm: None,
            monitor_sample_count: None,
            tx_total: None,
            rx_total: None,
            tx_retries: None,
            tx_errors: None,
            rx_errors: None,
        }
    }
}

impl ThreadNetworkDiagnosticsResponse {
    fn empty(error: Option<String>) -> Self {
        Self {
            scanning: false,
            scanned_at: None,
            error,
            channels: Vec::new(),
            networks: Vec::new(),
            devices: Vec::new(),
            statistics: ThreadRadioStatisticsResponse::empty(),
            warnings: Vec::new(),
        }
    }

    fn from_scan(scan: ThreadScan) -> Self {
        Self {
            scanning: false,
            scanned_at: None,
            error: None,
            channels: scan
                .channels
                .into_iter()
                .map(ThreadChannelDiagnosticsResponse::from)
                .collect(),
            networks: scan
                .networks
                .into_iter()
                .map(ThreadNetworkResponse::from)
                .collect(),
            devices: scan
                .devices
                .into_iter()
                .map(ThreadMeshDeviceResponse::from)
                .collect(),
            statistics: ThreadRadioStatisticsResponse::from(scan.statistics),
            warnings: scan.warnings,
        }
    }
}

impl From<ThreadScanSnapshot> for ThreadNetworkDiagnosticsResponse {
    fn from(snapshot: ThreadScanSnapshot) -> Self {
        let ThreadScanSnapshot {
            scan,
            scanning,
            completed_at,
            error,
        } = snapshot;
        let mut response = scan.map_or_else(|| Self::empty(error.clone()), Self::from_scan);
        response.scanning = scanning;
        response.error = error;
        response.scanned_at = completed_at.map(|completed_at| {
            DateTime::<Utc>::from(completed_at).to_rfc3339_opts(SecondsFormat::Millis, true)
        });
        response
    }
}

fn thread_ratio_percent(value: u16) -> f64 {
    (f64::from(value) * 1_000.0 / f64::from(u16::MAX)).round() / 10.0
}

impl From<ThreadMeshDevice> for ThreadMeshDeviceResponse {
    fn from(device: ThreadMeshDevice) -> Self {
        Self {
            id: device.id,
            is_border_router: device.is_border_router,
            extended_address: device.extended_address,
            mesh_local_eid_iid: device.mesh_local_eid_iid,
            omr_ipv6_addresses: device.omr_ipv6_addresses,
            hostname: device.hostname,
            eui64: device.eui64,
            role: device.role,
            full_thread_device: device.full_thread_device,
            rx_on_when_idle: device.rx_on_when_idle,
            full_network_data: device.full_network_data,
            rloc16: device.rloc16,
            rloc_address: device.rloc_address,
            router_id: device.router_id,
            router_count: device.router_count,
            network_name: device.network_name,
            extended_pan_id: device.extended_pan_id,
            border_agent_id: device.border_agent_id,
            border_agent_state: device.border_agent_state,
            partition_id: device.partition_id,
            leader_router_id: device.leader_router_id,
            data_version: device.data_version,
            stable_data_version: device.stable_data_version,
            created_at: device.created_at,
            updated_at: device.updated_at,
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
    let Some(runtime) = state.thread_runtime.as_ref() else {
        return Err(AppError::Conflict(
            "The local OpenThread border router is unavailable. Connect an RCP and refresh Thread settings."
                .to_string(),
        ));
    };
    runtime.controller().ok_or_else(|| {
        let snapshot = runtime.snapshot();
        AppError::Conflict(
            snapshot.message.unwrap_or_else(|| {
                "The local OpenThread border router is unavailable. Connect an RCP and refresh Thread settings."
                    .to_string()
            }),
        )
    })
}

async fn thread_status(runtime: Arc<ThreadRuntime>) -> ThreadStatusResponse {
    let snapshot = runtime.snapshot();
    let available_rcp_devices = rcp_devices(runtime.clone()).await;
    let Some(controller) = runtime.controller() else {
        return ThreadStatusResponse::unavailable_runtime(snapshot, available_rcp_devices);
    };
    let rcp_device = snapshot
        .rcp_device
        .map(|device| device.display().to_string());
    match run_blocking(controller, |controller| controller.status()).await {
        Ok(status) => ThreadStatusResponse::connected(status, rcp_device, available_rcp_devices),
        Err(error) => ThreadStatusResponse::failed(error, rcp_device, available_rcp_devices),
    }
}

async fn status_after_change(
    runtime: Arc<ThreadRuntime>,
    controller: Arc<ThreadController>,
) -> Result<ThreadStatusResponse, AppError> {
    let status = run_blocking(controller, |controller| controller.status()).await?;
    let rcp_device = runtime
        .snapshot()
        .rcp_device
        .map(|device| device.display().to_string());
    Ok(ThreadStatusResponse::connected(
        status,
        rcp_device,
        rcp_devices(runtime).await,
    ))
}

async fn rcp_devices(runtime: Arc<ThreadRuntime>) -> Vec<String> {
    match tokio::task::spawn_blocking(move || runtime.available_rcp_devices()).await {
        Ok(Ok(devices)) => devices
            .into_iter()
            .map(|device| device.display().to_string())
            .collect(),
        Ok(Err(error)) => {
            warn!(%error, "Unable to inspect available Thread RCP serial devices");
            Vec::new()
        }
        Err(error) => {
            warn!(%error, "Thread RCP serial-device inspection task failed");
            Vec::new()
        }
    }
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
