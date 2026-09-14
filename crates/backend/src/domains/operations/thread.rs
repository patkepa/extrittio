//! Host-local OpenThread Border Router controls for Extrittio Edge.
//!
//! Thread credentials never enter persistence or audit metadata. The active
//! dataset is returned only from an owner-authenticated, explicitly requested
//! endpoint and is retained by OpenThread's own operational dataset storage.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    routing::{get, post},
};
use chrono::{DateTime, SecondsFormat, Utc};
use extrittio_openthread_runtime::{
    CreateNetwork, OpenThreadError, ThreadActiveDataset, ThreadChannelDiagnostics,
    ThreadMeshDevice, ThreadNetwork, ThreadRadioStatistics, ThreadRcpCandidate, ThreadRuntime,
    ThreadRuntimeSnapshot, ThreadScan, ThreadScanSnapshot, ThreadScanSourceStatus, ThreadStatus,
};
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;
use zeroize::Zeroizing;

use crate::{auth::context::RequestContext, error::AppError, state::AppState};

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadStatusResponse {
    /// Whether this process is Extrittio Edge with a controllable OTBR.
    pub available: bool,
    pub connected: bool,
    pub error: Option<String>,
    pub rcp_device: Option<String>,
    /// Persisted host-local RCP selection. Omitted when automatic discovery is enabled.
    pub configured_rcp_device: Option<String>,
    /// Serial devices currently detected as plausible Thread RCPs.
    pub available_rcp_devices: Vec<String>,
    pub available_rcp_candidates: Vec<ThreadRcpCandidateResponse>,
    pub runtime_phase: String,
    pub consecutive_failures: u32,
    pub restart_count: u64,
    pub next_retry_at: Option<String>,
    pub last_exit: Option<String>,
    pub role: Option<String>,
    pub network_name: Option<String>,
    pub channel: Option<u16>,
    pub pan_id: Option<String>,
    pub extended_pan_id: Option<String>,
    pub mesh_local_prefix: Option<String>,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ThreadRcpCandidateResponse {
    pub path: String,
    pub confidence: String,
    pub match_reason: String,
    pub usb_vendor_id: Option<u16>,
    pub usb_product_id: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateThreadNetworkRequest {
    pub network_name: String,
    pub channel: Option<u16>,
    pub pan_id: Option<String>,
    pub extended_pan_id: Option<String>,
    /// Optional 16-byte Thread network key, as 32 hexadecimal characters.
    /// Omit it to have OpenThread generate a secure random key.
    #[schema(value_type = Option<String>, write_only = true)]
    pub network_key: Option<Zeroizing<String>>,
}

#[derive(Deserialize, ToSchema)]
pub struct ImportThreadDatasetRequest {
    /// Complete hex-encoded Active Operational Dataset TLVs. This value is
    /// write-only because it contains the Thread network key.
    #[schema(value_type = String, write_only = true)]
    pub active_dataset_tlvs: Zeroizing<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigureThreadRuntimeRequest {
    /// Detected serial device to persist, or null to restore automatic discovery.
    pub rcp_device: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ThreadDatasetResponse {
    /// Complete hex-encoded Active Operational Dataset TLVs. Scanning or
    /// importing this value can grant a device access to the Thread network.
    #[schema(value_type = String)]
    pub active_dataset_tlvs: Zeroizing<String>,
    /// Thread Network Key extracted from the active dataset, when present.
    #[schema(value_type = Option<String>)]
    pub network_key: Option<Zeroizing<String>>,
    /// Pre-Shared Key for the Commissioner extracted from the active dataset,
    /// when present.
    #[schema(value_type = Option<String>)]
    pub pskc: Option<Zeroizing<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadNetworkResponse {
    pub network_name: Option<String>,
    pub pan_id: String,
    pub extended_address: String,
    pub extended_pan_id: Option<String>,
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
    pub sources: Vec<ThreadScanSourceResponse>,
    /// Measurements unsupported by the current OTBR/RCP combination.
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ThreadScanSourceResponse {
    pub source: String,
    pub state: String,
    pub observed_at: Option<String>,
    pub error: Option<String>,
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
            "/api/v1/system/thread/configuration",
            axum::routing::put(configure_thread_runtime),
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
    let Some(runtime) = state.runtime().thread_runtime().as_ref() else {
        return Ok(Json(ThreadNetworkDiagnosticsResponse::empty(Some(
            "The local OpenThread border router is unavailable".to_string(),
        ))));
    };
    let snapshot = runtime.snapshot();
    if !snapshot.available {
        return Ok(Json(ThreadNetworkDiagnosticsResponse::empty(
            snapshot.message,
        )));
    }
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
    let runtime = thread_runtime(&state)?;
    let snapshot = run_blocking(runtime, |runtime| {
        runtime.refresh_scan(std::time::Duration::from_secs(60), true)
    })
    .await?;
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
    let Some(runtime) = state.runtime().thread_runtime().clone() else {
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
    let Some(runtime) = state.runtime().thread_runtime().clone() else {
        return Ok(Json(ThreadStatusResponse::unavailable()));
    };
    let refreshed_runtime = runtime.clone();
    tokio::task::spawn_blocking(move || refreshed_runtime.refresh())
        .await
        .map_err(|error| AppError::Internal(format!("Thread refresh task failed: {error}")))?;
    Ok(Json(thread_status(runtime).await))
}

#[utoipa::path(
    put,
    path = "/api/v1/system/thread/configuration",
    tag = "system",
    security(("bearer_auth" = [])),
    request_body = ConfigureThreadRuntimeRequest,
    responses(
        (status = 200, description = "Updated host-local OpenThread runtime configuration", body = ThreadStatusResponse),
        (status = 400, description = "The selected RCP is not currently detected"),
        (status = 403, description = "Owner access required"),
        (status = 409, description = "Thread is unavailable on this deployment"),
    ),
)]
pub(crate) async fn configure_thread_runtime(
    Extension(ctx): Extension<RequestContext>,
    State(state): State<Arc<AppState>>,
    Json(request): Json<ConfigureThreadRuntimeRequest>,
) -> Result<Json<ThreadStatusResponse>, AppError> {
    require_owner(&ctx)?;
    let runtime = state.runtime().thread_runtime().clone().ok_or_else(|| {
        AppError::Conflict("OpenThread is unavailable on this deployment".to_string())
    })?;
    let rcp_device = request.rcp_device.map(PathBuf::from);
    run_blocking(runtime.clone(), move |runtime| {
        runtime.set_rcp_device(rcp_device)
    })
    .await?;
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
    let runtime = thread_runtime(&state)?;
    let network = CreateNetwork::new(
        request.network_name,
        request.channel,
        request.pan_id,
        request.extended_pan_id,
        request.network_key.map(|mut key| std::mem::take(&mut *key)),
    )
    .map_err(map_thread_error)?;
    let status = run_blocking(runtime.clone(), move |runtime| {
        runtime.create_network(&network)
    })
    .await?;
    Ok(Json(status_after_change(runtime, status).await))
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
    let dataset = run_blocking(thread_runtime(&state)?, |runtime| runtime.active_dataset()).await?;

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
    let runtime = thread_runtime(&state)?;
    let status = run_blocking(runtime.clone(), move |runtime| {
        runtime.import_active_dataset(&request.active_dataset_tlvs)
    })
    .await?;
    Ok(Json(status_after_change(runtime, status).await))
}

impl ThreadStatusResponse {
    fn unavailable() -> Self {
        Self {
            available: false,
            connected: false,
            error: None,
            rcp_device: None,
            configured_rcp_device: None,
            available_rcp_devices: Vec::new(),
            available_rcp_candidates: Vec::new(),
            runtime_phase: "unavailable".to_string(),
            consecutive_failures: 0,
            restart_count: 0,
            next_retry_at: None,
            last_exit: None,
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
        configured_rcp_device: Option<String>,
        available_rcp_candidates: Vec<ThreadRcpCandidateResponse>,
    ) -> Self {
        let available_rcp_devices = available_rcp_candidates
            .iter()
            .map(|candidate| candidate.path.clone())
            .collect();
        Self {
            error: snapshot.message,
            rcp_device: snapshot
                .rcp_device
                .map(|device| device.display().to_string()),
            configured_rcp_device,
            available_rcp_devices,
            available_rcp_candidates,
            runtime_phase: snapshot.phase.as_str().to_string(),
            consecutive_failures: snapshot.consecutive_failures,
            restart_count: snapshot.restart_count,
            next_retry_at: snapshot.next_retry_at.map(format_system_time),
            last_exit: snapshot.last_exit,
            ..Self::unavailable()
        }
    }

    fn connected(
        status: ThreadStatus,
        snapshot: ThreadRuntimeSnapshot,
        configured_rcp_device: Option<String>,
        available_rcp_candidates: Vec<ThreadRcpCandidateResponse>,
    ) -> Self {
        let connected = status.is_attached();
        let available_rcp_devices = available_rcp_candidates
            .iter()
            .map(|candidate| candidate.path.clone())
            .collect();
        Self {
            available: true,
            connected,
            error: None,
            rcp_device: snapshot
                .rcp_device
                .map(|device| device.display().to_string()),
            configured_rcp_device,
            available_rcp_devices,
            available_rcp_candidates,
            runtime_phase: snapshot.phase.as_str().to_string(),
            consecutive_failures: snapshot.consecutive_failures,
            restart_count: snapshot.restart_count,
            next_retry_at: snapshot.next_retry_at.map(format_system_time),
            last_exit: snapshot.last_exit,
            role: Some(status.role.as_str().to_owned()),
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
        snapshot: ThreadRuntimeSnapshot,
        configured_rcp_device: Option<String>,
        available_rcp_candidates: Vec<ThreadRcpCandidateResponse>,
    ) -> Self {
        let available_rcp_devices = available_rcp_candidates
            .iter()
            .map(|candidate| candidate.path.clone())
            .collect();
        Self {
            available: true,
            connected: false,
            error: Some(error.to_string()),
            rcp_device: snapshot
                .rcp_device
                .map(|device| device.display().to_string()),
            configured_rcp_device,
            available_rcp_devices,
            available_rcp_candidates,
            runtime_phase: snapshot.phase.as_str().to_string(),
            consecutive_failures: snapshot.consecutive_failures,
            restart_count: snapshot.restart_count,
            next_retry_at: snapshot.next_retry_at.map(format_system_time),
            last_exit: snapshot.last_exit,
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
            extended_pan_id: network.extended_pan_id,
            channel: network.channel,
            rssi: network.rssi,
            lqi: network.lqi,
        }
    }
}

impl From<ThreadActiveDataset> for ThreadDatasetResponse {
    fn from(dataset: ThreadActiveDataset) -> Self {
        Self {
            active_dataset_tlvs: Zeroizing::new(dataset.expose_active_dataset_tlvs().to_owned()),
            network_key: dataset
                .expose_network_key()
                .map(|value| Zeroizing::new(value.to_owned())),
            pskc: dataset
                .expose_pskc()
                .map(|value| Zeroizing::new(value.to_owned())),
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
            sources: Vec::new(),
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
            sources: scan
                .sources
                .into_iter()
                .map(ThreadScanSourceResponse::from)
                .collect(),
            warnings: scan.warnings,
        }
    }
}

impl From<ThreadScanSourceStatus> for ThreadScanSourceResponse {
    fn from(status: ThreadScanSourceStatus) -> Self {
        Self {
            source: status.source.as_str().to_string(),
            state: status.state.as_str().to_string(),
            observed_at: status.observed_at.map(format_system_time),
            error: status.error,
        }
    }
}

impl From<ThreadRcpCandidate> for ThreadRcpCandidateResponse {
    fn from(candidate: ThreadRcpCandidate) -> Self {
        Self {
            path: candidate.path.display().to_string(),
            confidence: candidate.confidence.as_str().to_string(),
            match_reason: candidate.match_reason,
            usb_vendor_id: candidate.usb_vendor_id,
            usb_product_id: candidate.usb_product_id,
            manufacturer: candidate.manufacturer,
            product: candidate.product,
            serial_number: candidate.serial_number,
        }
    }
}

fn format_system_time(time: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339_opts(SecondsFormat::Millis, true)
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

fn thread_runtime(state: &AppState) -> Result<Arc<ThreadRuntime>, AppError> {
    let Some(runtime) = state.runtime().thread_runtime().clone() else {
        return Err(AppError::Conflict(
            "The local OpenThread border router is unavailable. Connect an RCP and refresh Thread settings."
                .to_string(),
        ));
    };
    if runtime.snapshot().available {
        Ok(runtime)
    } else {
        let snapshot = runtime.snapshot();
        Err(AppError::Conflict(
            snapshot.message.unwrap_or_else(|| {
                "The local OpenThread border router is unavailable. Connect an RCP and refresh Thread settings."
                    .to_string()
            }),
        ))
    }
}

async fn thread_status(runtime: Arc<ThreadRuntime>) -> ThreadStatusResponse {
    let snapshot = runtime.snapshot();
    let configured_rcp_device = runtime
        .configured_rcp_device()
        .map(|device| device.display().to_string());
    let available_rcp_candidates = rcp_candidates(runtime.clone()).await;
    if !snapshot.available {
        return ThreadStatusResponse::unavailable_runtime(
            snapshot,
            configured_rcp_device,
            available_rcp_candidates,
        );
    }
    match run_blocking(runtime, |runtime| runtime.status()).await {
        Ok(status) => ThreadStatusResponse::connected(
            status,
            snapshot,
            configured_rcp_device,
            available_rcp_candidates,
        ),
        Err(error) => ThreadStatusResponse::failed(
            error,
            snapshot,
            configured_rcp_device,
            available_rcp_candidates,
        ),
    }
}

async fn status_after_change(
    runtime: Arc<ThreadRuntime>,
    status: ThreadStatus,
) -> ThreadStatusResponse {
    let snapshot = runtime.snapshot();
    let configured_rcp_device = runtime
        .configured_rcp_device()
        .map(|device| device.display().to_string());
    ThreadStatusResponse::connected(
        status,
        snapshot,
        configured_rcp_device,
        rcp_candidates(runtime).await,
    )
}

async fn rcp_candidates(runtime: Arc<ThreadRuntime>) -> Vec<ThreadRcpCandidateResponse> {
    match tokio::task::spawn_blocking(move || runtime.available_rcp_candidates()).await {
        Ok(Ok(candidates)) => candidates
            .into_iter()
            .map(ThreadRcpCandidateResponse::from)
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

async fn run_blocking<T, F>(runtime: Arc<ThreadRuntime>, operation: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce(Arc<ThreadRuntime>) -> extrittio_openthread_runtime::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || operation(runtime))
        .await
        .map_err(|error| AppError::Internal(format!("Thread control task failed: {error}")))?
        .map_err(map_thread_error)
}

fn map_thread_error(error: OpenThreadError) -> AppError {
    match error {
        error @ (OpenThreadError::InvalidConfiguration(_) | OpenThreadError::InvalidDataset(_)) => {
            AppError::BadRequest(error.to_string())
        }
        error @ (OpenThreadError::Unavailable(_) | OpenThreadError::NetworkChanged) => {
            AppError::Conflict(error.to_string())
        }
        error => AppError::Internal(error.to_string()),
    }
}
