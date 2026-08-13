//! Lifecycle support for a local OpenThread Border Router (OTBR) radio co-processor.
//!
//! The Thread network itself is implemented by OpenThread's `otbr-agent`. This crate
//! deliberately owns only Extrittio-specific concerns: deterministic RCP discovery,
//! safe construction of the radio URL, and child-process lifecycle management. A
//! raw Spinel client is insufficient for a border router because OTBR also configures
//! IPv6 routing, service discovery, and the host network interface.

use std::{
    env,
    ffi::OsString,
    fs,
    net::Ipv6Addr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

/// Public development dataset shared by the hobby border router and the
/// ESP32-C6 OpenThread example. It is intentionally not suitable for a
/// private or production Thread mesh.
pub const DEFAULT_DEVELOPMENT_DATASET_TLVS: &str = "0e080000000000010000000300001935060004001fffe00208ef1398c2fd504b670708fd35344133d1d73e0510fda7c771a27202e232ecd04cf934f476030f4f70656e5468726561642d633634650102c64e04105e9b9b360f80b88be2603fb0135c8d650c0402a0f7f8";

/// Configuration for one local OTBR instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderRouterConfig {
    /// OpenThread's `otbr-agent` executable.
    pub agent_path: PathBuf,
    /// Serial RCP device, such as `/dev/cu.usbmodem…` or `/dev/ttyACM0`.
    pub rcp_device: PathBuf,
    /// UART speed configured in the RCP firmware.
    pub baud_rate: u32,
    /// Name of the host-side Thread interface created by OTBR.
    pub thread_interface: String,
    /// Ethernet or Wi-Fi interface adjacent to the Extrittio host.
    pub infrastructure_interface: String,
    /// Writable OTBR state directory, including its Thread operational dataset.
    pub data_path: PathBuf,
}

impl BorderRouterConfig {
    #[must_use]
    pub fn radio_url(&self) -> String {
        format!(
            "spinel+hdlc+uart://{}?uart-baudrate={}",
            self.rcp_device.display(),
            self.baud_rate
        )
    }

    /// The argument sequence accepted by OpenThread's portable `otbr-agent`.
    #[must_use]
    pub fn agent_args(&self) -> Vec<OsString> {
        vec![
            "-I".into(),
            self.thread_interface.clone().into(),
            "-B".into(),
            self.infrastructure_interface.clone().into(),
            "--vendor-name".into(),
            "Extrittio".into(),
            "--model-name".into(),
            "Hobby Appliance".into(),
            // Keep agent failures with the Extrittio process logs, where they
            // can be diagnosed without opening the platform syslog.
            "--syslog-disable".into(),
            "--data-path".into(),
            self.data_path.clone().into_os_string(),
            self.radio_url().into(),
        ]
    }
}

/// A running `otbr-agent`, stopped together with the Extrittio hobby process.
pub struct BorderRouter {
    child: Child,
    config: BorderRouterConfig,
    dbus_daemon: Option<DbusDaemon>,
}

struct DbusDaemon {
    process_id: u32,
}

/// Controlled access to the local OTBR instance.
///
/// OTBR exposes RCP-mode status over its loopback REST API. Keeping access
/// behind this type prevents the web application from accepting arbitrary
/// commands and avoids `ot-ctl`, whose Unix-socket protocol only supports NCP
/// mode and cannot control a serial RCP border router.
#[derive(Clone)]
pub struct ThreadController {
    rest_endpoint: String,
    dbus_address: Option<String>,
    thread_interface: String,
    command_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadStatus {
    pub role: Option<String>,
    pub network_name: Option<String>,
    pub channel: Option<u16>,
    pub pan_id: Option<String>,
    pub extended_pan_id: Option<String>,
    pub mesh_local_prefix: Option<String>,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateNetwork {
    pub network_name: String,
    pub channel: Option<u16>,
    pub pan_id: Option<String>,
    pub extended_pan_id: Option<String>,
    pub network_key: Option<String>,
}

/// A Thread network discovered by the local radio during an active scan.
///
/// This deliberately contains only the metadata broadcast over the air. A
/// Thread operational dataset (and its network key) is never discoverable via
/// a scan and is not represented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadNetwork {
    pub network_name: Option<String>,
    pub pan_id: String,
    pub extended_address: String,
    pub channel: u16,
    pub rssi: i16,
    pub lqi: u8,
}

/// Radio conditions observed on one IEEE 802.15.4 channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadChannelDiagnostics {
    pub channel: u16,
    /// Maximum RSSI observed during the point-in-time energy scan, in dBm.
    pub max_rssi: Option<i16>,
    /// OpenThread channel-monitor occupancy, where `u16::MAX` represents 100%.
    pub occupancy: Option<u16>,
    pub network_count: usize,
    pub strongest_network_rssi: Option<i16>,
}

/// Cumulative radio statistics reported by the local OpenThread instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRadioStatistics {
    /// OpenThread CCA failure rate, where `u16::MAX` represents 100%.
    pub cca_failure_rate: Option<u16>,
    pub latest_rssi: Option<i16>,
    pub monitor_sample_count: Option<u32>,
    pub tx_total: Option<u32>,
    pub rx_total: Option<u32>,
    pub tx_retries: Option<u32>,
    pub tx_errors: Option<u32>,
    pub rx_errors: Option<u32>,
}

/// A point-in-time scan combining nearby Thread networks, channel energy,
/// long-running channel occupancy, and cumulative radio statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadNetworkDiagnostics {
    pub channels: Vec<ThreadChannelDiagnostics>,
    pub networks: Vec<ThreadNetwork>,
    pub statistics: ThreadRadioStatistics,
    /// Diagnostics that were unavailable on this OTBR/RCP combination.
    pub warnings: Vec<String>,
}

/// One node discovered on the active Thread mesh through OTBR diagnostics.
///
/// These fields intentionally exclude credentials and mutable operational
/// datasets. They are the non-secret inventory attributes exposed by OTBR's
/// device-discovery collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMeshDevice {
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

/// A point-in-time view of nearby Thread networks and the active mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMeshScan {
    pub networks: Vec<ThreadNetwork>,
    pub devices: Vec<ThreadMeshDevice>,
}

/// Configuration used to discover and supervise a local Thread border router
/// after the application has started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRuntimeConfig {
    pub rcp_device: Option<PathBuf>,
    pub agent_path: Option<PathBuf>,
    pub baud_rate: u32,
    pub thread_interface: String,
    pub infrastructure_interface: String,
    pub data_path: PathBuf,
}

/// Non-secret local Thread runtime state, suitable for displaying to an
/// appliance owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRuntimeSnapshot {
    pub available: bool,
    pub rcp_device: Option<PathBuf>,
    pub message: Option<String>,
}

impl ThreadRuntimeSnapshot {
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            rcp_device: None,
            message: Some(message.into()),
        }
    }

    #[must_use]
    pub fn ready(rcp_device: PathBuf) -> Self {
        Self {
            available: true,
            rcp_device: Some(rcp_device),
            message: None,
        }
    }
}

/// Owns an OTBR instance and can discover a newly attached RCP on demand.
///
/// Refreshing is intentionally explicit: it avoids a background process
/// repeatedly opening arbitrary serial devices, while letting the UI recover
/// when an RCP is connected after Extrittio has started.
pub struct ThreadRuntime {
    config: ThreadRuntimeConfig,
    state: Mutex<ThreadRuntimeState>,
    network_generation: AtomicU64,
}

struct ThreadRuntimeState {
    active: Option<ActiveThreadRuntime>,
    snapshot: ThreadRuntimeSnapshot,
}

struct ActiveThreadRuntime {
    router: BorderRouter,
    controller: Arc<ThreadController>,
}

impl ThreadRuntime {
    #[must_use]
    pub fn new(config: ThreadRuntimeConfig) -> Self {
        let rcp_device = config.rcp_device.clone();
        Self {
            config,
            state: Mutex::new(ThreadRuntimeState {
                active: None,
                snapshot: ThreadRuntimeSnapshot {
                    available: false,
                    rcp_device,
                    message: Some("Thread runtime has not checked for an RCP yet".to_string()),
                },
            }),
            network_generation: AtomicU64::new(0),
        }
    }

    /// Recheck the serial RCP and start OTBR when a usable radio is present.
    /// If the radio was unplugged or OTBR exited, its old runtime is stopped
    /// before discovery is retried.
    #[must_use]
    pub fn refresh(&self) -> ThreadRuntimeSnapshot {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(active) = state.active.as_mut() {
            let rcp_present = active.router.config().rcp_device.exists();
            let running = active.router.is_running().unwrap_or(false);
            if rcp_present && running {
                let snapshot =
                    ThreadRuntimeSnapshot::ready(active.router.config().rcp_device.clone());
                state.snapshot = snapshot.clone();
                return snapshot;
            }
        }
        if state.active.is_some() {
            let mut inactive = state
                .active
                .take()
                .expect("active runtime was checked above");
            if let Err(error) = inactive.router.shutdown() {
                warn!(%error, "Failed to stop an unavailable OpenThread border router");
            }
        }

        let rcp_device = match self
            .config
            .rcp_device
            .clone()
            .map_or_else(discover_rcp, |device| Ok(Some(device)))
        {
            Ok(Some(device)) => device,
            Ok(None) => {
                return Self::set_snapshot(
                    &mut state,
                    ThreadRuntimeSnapshot::unavailable(
                        "No unique Thread RCP serial device was detected. Connect an RCP or select its serial port explicitly.",
                    ),
                );
            }
            Err(error) => {
                return Self::set_snapshot(
                    &mut state,
                    ThreadRuntimeSnapshot::unavailable(format!(
                        "Unable to discover a Thread RCP: {error}"
                    )),
                );
            }
        };

        let agent_path = match discover_agent(self.config.agent_path.as_deref()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                return Self::set_snapshot(
                    &mut state,
                    ThreadRuntimeSnapshot {
                        available: false,
                        rcp_device: Some(rcp_device),
                        message: Some(
                            "The OpenThread border-router executable is unavailable".to_string(),
                        ),
                    },
                );
            }
            Err(error) => {
                return Self::set_snapshot(
                    &mut state,
                    ThreadRuntimeSnapshot {
                        available: false,
                        rcp_device: Some(rcp_device),
                        message: Some(format!(
                            "Unable to locate the OpenThread border-router executable: {error}"
                        )),
                    },
                );
            }
        };

        let controller = match ThreadController::discover(&agent_path, None) {
            Ok(Some(controller)) => controller,
            Ok(None) => {
                return Self::set_snapshot(
                    &mut state,
                    ThreadRuntimeSnapshot {
                        available: false,
                        rcp_device: Some(rcp_device),
                        message: Some(
                            "The OpenThread controller tool (ot-ctl) is unavailable".to_string(),
                        ),
                    },
                );
            }
            Err(error) => {
                return Self::set_snapshot(
                    &mut state,
                    ThreadRuntimeSnapshot {
                        available: false,
                        rcp_device: Some(rcp_device),
                        message: Some(format!(
                            "Unable to locate the OpenThread controller tool: {error}"
                        )),
                    },
                );
            }
        };

        let config = BorderRouterConfig {
            agent_path,
            rcp_device: rcp_device.clone(),
            baud_rate: self.config.baud_rate,
            thread_interface: self.config.thread_interface.clone(),
            infrastructure_interface: self.config.infrastructure_interface.clone(),
            data_path: self.config.data_path.clone(),
        };
        match BorderRouter::start_with_controller(config, controller) {
            Ok((router, controller)) => {
                state.active = Some(ActiveThreadRuntime {
                    router,
                    controller: Arc::new(controller),
                });
                self.network_generation.fetch_add(1, Ordering::Relaxed);
                Self::set_snapshot(&mut state, ThreadRuntimeSnapshot::ready(rcp_device))
            }
            Err(error) => Self::set_snapshot(
                &mut state,
                ThreadRuntimeSnapshot {
                    available: false,
                    rcp_device: Some(rcp_device),
                    message: Some(format!(
                        "Unable to start the OpenThread border router: {error}"
                    )),
                },
            ),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> ThreadRuntimeSnapshot {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .snapshot
            .clone()
    }

    /// Lists serial devices that could be directly connected Thread RCPs.
    ///
    /// This is deliberately diagnostic-only: automatic startup still requires
    /// exactly one candidate so the runtime never guesses between radios.
    pub fn available_rcp_devices(&self) -> Result<Vec<PathBuf>> {
        serial_candidates()
    }

    #[must_use]
    pub fn controller(&self) -> Option<Arc<ThreadController>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .as_ref()
            .map(|active| active.controller.clone())
    }

    /// Seeds the shared public development mesh only when OTBR has no active
    /// dataset. User-created and imported datasets are always left intact.
    pub fn ensure_default_development_network(&self) -> Result<bool> {
        let controller = self
            .controller()
            .context("The local OpenThread border router is unavailable")?;
        let seeded = controller.ensure_default_development_network()?;
        if seeded {
            self.mark_network_changed();
        }
        Ok(seeded)
    }

    pub fn shutdown(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(mut active) = state.active.take()
            && let Err(error) = active.router.shutdown()
        {
            warn!(%error, "OpenThread border-router shutdown failed");
        }
    }

    /// Increments the mesh generation after replacing the active dataset.
    /// Callers use this to re-register the backend service on the new mesh.
    pub fn mark_network_changed(&self) {
        self.network_generation.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn network_generation(&self) -> u64 {
        self.network_generation.load(Ordering::Relaxed)
    }

    /// Returns a non-link-local IPv6 address assigned to OTBR's Thread
    /// interface. This is deliberately not derived from a Zenoh wildcard
    /// listener; DNS-SD clients must receive a concrete, mesh-reachable host.
    pub fn thread_ipv6_address(&self) -> Result<Ipv6Addr> {
        let (thread_interface, controller) = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .as_ref()
            .map(|active| {
                (
                    active.router.config().thread_interface.clone(),
                    active.controller.clone(),
                )
            })
            .context("The local OpenThread border router is unavailable")?;
        match thread_interface_ipv6_address(&thread_interface) {
            Ok(address) => Ok(address),
            Err(_primary_error) => {
                #[cfg(target_os = "macos")]
                {
                    // macOS assigns OTBR's TUN device a dynamic `utunN` name
                    // instead of the Linux-style `wpan0` requested from the
                    // agent. Find it by OTBR's authoritative mesh-local prefix.
                    let mesh_local_prefix = controller
                        .status()?
                        .mesh_local_prefix
                        .context("OTBR did not report a mesh-local IPv6 prefix")?;
                    let address = macos_mesh_local_ipv6_address(&mesh_local_prefix)
                        .with_context(|| {
                            format!(
                                "Failed to find the macOS OTBR interface for mesh-local prefix {mesh_local_prefix}"
                            )
                        })?;
                    info!(
                        configured_interface = %thread_interface,
                        mesh_local_prefix,
                        address = %address,
                        "Using the macOS OTBR tunnel interface for Thread DNS-SD"
                    );
                    Ok(address)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(_primary_error)
                }
            }
        }
    }

    fn set_snapshot(
        state: &mut ThreadRuntimeState,
        snapshot: ThreadRuntimeSnapshot,
    ) -> ThreadRuntimeSnapshot {
        state.snapshot = snapshot.clone();
        snapshot
    }
}

impl Drop for ThreadRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl ThreadController {
    /// Creates a controller for the OTBR companion binary installed beside the
    /// agent. The binary may also be supplied explicitly for development.
    pub fn discover(_agent_path: &Path, _explicit: Option<&Path>) -> Result<Option<Self>> {
        Ok(Some(Self {
            rest_endpoint: "http://127.0.0.1:8081".to_string(),
            dbus_address: None,
            thread_interface: "wpan0".to_string(),
            command_lock: Arc::new(Mutex::new(())),
        }))
    }

    fn with_dbus_control(mut self, address: String, thread_interface: String) -> Self {
        self.dbus_address = Some(address);
        self.thread_interface = thread_interface;
        self
    }

    pub fn status(&self) -> Result<ThreadStatus> {
        let role = Some(self.rest_json_string("/node/state")?);
        let dataset = self.rest_json("/node/dataset/active").ok();

        Ok(ThreadStatus {
            role,
            network_name: dataset
                .as_ref()
                .and_then(|dataset| dataset_string(dataset, "networkName")),
            channel: dataset
                .as_ref()
                .and_then(|dataset| dataset_u16(dataset, "channel")),
            pan_id: dataset
                .as_ref()
                .and_then(|dataset| dataset_string(dataset, "panId")),
            extended_pan_id: dataset
                .as_ref()
                .and_then(|dataset| dataset_string(dataset, "extendedPanId")),
            mesh_local_prefix: dataset
                .as_ref()
                .and_then(|dataset| dataset_string(dataset, "meshLocalPrefix")),
            addresses: Vec::new(),
        })
    }

    /// Performs an active scan using the local radio and returns nearby Thread
    /// networks. This does not alter the active operational dataset.
    pub fn scan_networks(&self) -> Result<Vec<ThreadNetwork>> {
        let _guard = self.lock();
        self.scan_networks_locked()
    }

    /// Combines an active Thread scan with channel energy, occupancy, and
    /// cumulative radio statistics from the local OTBR instance.
    pub fn scan_network_diagnostics(&self) -> Result<ThreadNetworkDiagnostics> {
        let _guard = self.lock();
        let networks = self.scan_networks_locked()?;
        let mut warnings = Vec::new();

        let energy = match self.energy_scan_locked(100) {
            Ok(energy) => energy,
            Err(error) => {
                warnings.push(format!("Channel energy scan is unavailable: {error}"));
                Vec::new()
            }
        };
        let occupancy = match self.dbus_property("ChannelMonitorAllChannelQualities") {
            Ok(output) => parse_dbus_channel_occupancy(&output),
            Err(error) => {
                warnings.push(format!("Channel utilization is unavailable: {error}"));
                Vec::new()
            }
        };
        let monitor_sample_count = self
            .dbus_property("ChannelMonitorSampleCount")
            .ok()
            .and_then(|output| parse_dbus_scalar::<u32>(&output, "uint32"));
        let cca_failure_rate = self
            .dbus_property("CcaFailureRate")
            .ok()
            .and_then(|output| parse_dbus_scalar::<u16>(&output, "uint16"));
        let latest_rssi = self
            .dbus_property("InstantRssi")
            .ok()
            .and_then(|output| parse_dbus_byte(&output));
        let mac_counters = self
            .dbus_property("LinkCounters")
            .ok()
            .and_then(|output| parse_dbus_mac_counters(&output));

        let channels = (11..=26)
            .map(|channel| ThreadChannelDiagnostics {
                channel,
                max_rssi: energy
                    .iter()
                    .find(|result| result.channel == channel)
                    .map(|result| result.max_rssi),
                occupancy: occupancy
                    .iter()
                    .find(|result| result.channel == channel)
                    .map(|result| result.occupancy),
                network_count: networks
                    .iter()
                    .filter(|network| network.channel == channel)
                    .count(),
                strongest_network_rssi: networks
                    .iter()
                    .filter(|network| network.channel == channel)
                    .map(|network| network.rssi)
                    .max(),
            })
            .collect();

        Ok(ThreadNetworkDiagnostics {
            channels,
            networks,
            statistics: ThreadRadioStatistics {
                cca_failure_rate,
                latest_rssi,
                monitor_sample_count,
                tx_total: mac_counters.as_ref().map(|counters| counters.tx_total),
                rx_total: mac_counters.as_ref().map(|counters| counters.rx_total),
                tx_retries: mac_counters.as_ref().map(|counters| counters.tx_retries),
                tx_errors: mac_counters.as_ref().map(|counters| counters.tx_errors),
                rx_errors: mac_counters.as_ref().map(|counters| counters.rx_errors),
            },
            warnings,
        })
    }

    fn scan_networks_locked(&self) -> Result<Vec<ThreadNetwork>> {
        let output = self.dbus_method("Scan", &[], 35_000)?;
        Ok(parse_dbus_scan(&output))
    }

    fn energy_scan_locked(&self, duration_ms: u32) -> Result<Vec<ThreadEnergyResult>> {
        let duration = format!("uint32:{duration_ms}");
        let output = self.dbus_method("EnergyScan", &[duration], 35_000)?;
        Ok(parse_dbus_energy_scan(&output))
    }

    fn dbus_property(&self, property: &str) -> Result<String> {
        self.dbus_call(
            "org.freedesktop.DBus.Properties.Get",
            &[
                "string:io.openthread.BorderRouter".to_string(),
                format!("string:{property}"),
            ],
            5_000,
        )
    }

    fn dbus_method(&self, method: &str, arguments: &[String], timeout_ms: u32) -> Result<String> {
        self.dbus_call(
            &format!("io.openthread.BorderRouter.{method}"),
            arguments,
            timeout_ms,
        )
    }

    fn dbus_call(&self, member: &str, arguments: &[String], timeout_ms: u32) -> Result<String> {
        let address = self
            .dbus_address
            .as_deref()
            .context("The local OTBR RCP control bus is unavailable")?;
        let service = format!("io.openthread.BorderRouter.{}", self.thread_interface);
        let object = format!("/io/openthread/BorderRouter/{}", self.thread_interface);
        let mut command = Command::new("dbus-send");
        command
            // OTBR connects with `dbus_bus_get(DBUS_BUS_SYSTEM)`. Point that
            // lookup at our private daemon so the client uses the identical
            // system-bus handshake instead of a direct peer connection.
            .env("DBUS_SYSTEM_BUS_ADDRESS", address)
            .args([
                "--system".to_string(),
                "--print-reply".to_string(),
                format!("--reply-timeout={timeout_ms}"),
                format!("--dest={service}"),
                object,
                member.to_string(),
            ])
            .args(arguments);
        command
            .output()
            .context("Failed to run the OTBR RCP controller")
            .and_then(|output| {
                if !output.status.success() {
                    bail!(
                        "OpenThread RCP request failed: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    );
                }
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            })
    }

    /// Discovers nearby Thread networks and refreshes OTBR's inventory for the
    /// active mesh. The diagnostics collection is an ephemeral OTBR cache, so
    /// it is cleared before discovery to avoid showing nodes from a previous
    /// operational dataset.
    pub fn scan_mesh(&self) -> Result<ThreadMeshScan> {
        let networks = self.scan_networks()?;
        let _guard = self.lock();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .context("Failed to initialize the OTBR mesh diagnostics client")?;

        let devices_url = self.url("/api/devices");
        client
            .delete(&devices_url)
            .send()
            .with_context(|| format!("Failed to clear stale OTBR mesh inventory at {devices_url}"))?
            .error_for_status()
            .with_context(|| {
                format!("OTBR rejected clearing its mesh inventory at {devices_url}")
            })?;

        let actions_url = self.url("/api/actions");
        let response = client
            .post(&actions_url)
            .header(reqwest::header::CONTENT_TYPE, "application/vnd.api+json")
            .json(&serde_json::json!({
                "data": [{
                    "type": "updateDeviceCollectionTask",
                    "attributes": {
                        "maxAge": 0,
                        "maxRetries": 1,
                        "deviceCount": 200,
                        "timeout": 8
                    }
                }]
            }))
            .send()
            .with_context(|| format!("Failed to start OTBR mesh discovery at {actions_url}"))?
            .error_for_status()
            .with_context(|| format!("OTBR rejected mesh discovery at {actions_url}"))?
            .json::<serde_json::Value>()
            .with_context(|| format!("OTBR returned an invalid mesh action from {actions_url}"))?;
        let action_id = response
            .pointer("/data/0/id")
            .and_then(serde_json::Value::as_str)
            .context("OTBR mesh discovery did not return an action identifier")?;
        self.wait_for_mesh_discovery(&client, action_id)?;

        let devices = client
            .get(&devices_url)
            .header(reqwest::header::ACCEPT, "application/vnd.api+json")
            .send()
            .with_context(|| format!("Failed to read OTBR mesh inventory at {devices_url}"))?
            .error_for_status()
            .with_context(|| format!("OTBR rejected its mesh inventory request at {devices_url}"))?
            .json::<serde_json::Value>()
            .with_context(|| format!("OTBR returned invalid mesh inventory from {devices_url}"))?;

        Ok(ThreadMeshScan {
            networks,
            devices: parse_thread_mesh_devices(&devices)?,
        })
    }

    fn wait_for_mesh_discovery(
        &self,
        client: &reqwest::blocking::Client,
        action_id: &str,
    ) -> Result<()> {
        let action_url = self.url(&format!("/api/actions/{action_id}"));
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            let response = client
                .get(&action_url)
                .header(reqwest::header::ACCEPT, "application/vnd.api+json")
                .send()
                .with_context(|| format!("Failed to inspect OTBR mesh discovery at {action_url}"))?
                .error_for_status()
                .with_context(|| format!("OTBR rejected mesh discovery status at {action_url}"))?
                .json::<serde_json::Value>()
                .with_context(|| {
                    format!("OTBR returned an invalid mesh discovery status from {action_url}")
                })?;
            let status = response
                .pointer("/data/attributes/status")
                .and_then(serde_json::Value::as_str)
                .context("OTBR mesh discovery status is missing")?;

            match status {
                "completed" | "stopped" => return Ok(()),
                "failed" => bail!("OTBR mesh discovery failed"),
                "pending" | "active" if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(250));
                }
                "pending" | "active" => bail!("OTBR mesh discovery timed out"),
                other => bail!("OTBR returned an unknown mesh discovery status: {other}"),
            }
        }
    }

    /// Forms a new Thread mesh. Existing devices will be detached, so callers
    /// must obtain explicit confirmation before invoking this operation.
    pub fn create_network(&self, network: &CreateNetwork) -> Result<()> {
        validate_create_network(network)?;
        let _guard = self.lock();
        self.rest_put_json(
            "/node/state",
            &serde_json::Value::String("disable".to_string()),
        )?;
        let mut dataset = serde_json::Map::new();
        dataset.insert(
            "networkName".to_string(),
            serde_json::Value::String(network.network_name.clone()),
        );
        if let Some(channel) = network.channel {
            dataset.insert("channel".to_string(), serde_json::Value::from(channel));
        }
        if let Some(pan_id) = &network.pan_id {
            dataset.insert(
                "panId".to_string(),
                serde_json::Value::from(parse_pan_id(pan_id)?),
            );
        }
        if let Some(extended_pan_id) = &network.extended_pan_id {
            dataset.insert(
                "extPanId".to_string(),
                serde_json::Value::String(strip_hex_prefix(extended_pan_id).to_string()),
            );
        }
        if let Some(network_key) = &network.network_key {
            dataset.insert(
                "networkKey".to_string(),
                serde_json::Value::String(strip_hex_prefix(network_key).to_string()),
            );
        }
        self.rest_put_json("/node/dataset/active", &serde_json::Value::Object(dataset))?;
        self.rest_put_json(
            "/node/state",
            &serde_json::Value::String("enable".to_string()),
        )
    }

    /// Imports the bundled development mesh when the RCP has no active
    /// network. The ESP32-C6 example carries the same dataset by default.
    pub fn ensure_default_development_network(&self) -> Result<bool> {
        let _guard = self.lock();
        // OTBR returns Active Operational Datasets as raw TLV hex rather than
        // JSON. A non-empty dataset is an existing user or OTBR network and
        // must never be replaced by the development mesh.
        if !self.rest_text("/node/dataset/active")?.trim().is_empty() {
            return Ok(false);
        }

        self.rest_put_json(
            "/node/state",
            &serde_json::Value::String("disable".to_string()),
        )?;
        self.rest_put_plain("/node/dataset/active", DEFAULT_DEVELOPMENT_DATASET_TLVS)?;
        self.rest_put_json(
            "/node/state",
            &serde_json::Value::String("enable".to_string()),
        )?;
        Ok(true)
    }

    /// Replaces the active dataset with a complete, hex-encoded Thread
    /// operational dataset. The dataset is write-only and is never returned by
    /// this API because it includes the mesh network key.
    pub fn import_active_dataset(&self, dataset_tlvs: &str) -> Result<()> {
        let dataset_tlvs = dataset_tlvs.trim();
        if dataset_tlvs.len() < 4
            || !dataset_tlvs.len().is_multiple_of(2)
            || !dataset_tlvs.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            bail!("Thread operational dataset must be an even-length hexadecimal value");
        }

        let _guard = self.lock();
        self.rest_put_json(
            "/node/state",
            &serde_json::Value::String("disable".to_string()),
        )?;
        self.rest_put_plain("/node/dataset/active", dataset_tlvs)?;
        self.rest_put_json(
            "/node/state",
            &serde_json::Value::String("enable".to_string()),
        )
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.command_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn rest_json_string(&self, path: &str) -> Result<String> {
        self.rest_json(path)?
            .as_str()
            .map(ToOwned::to_owned)
            .context("OTBR returned an invalid JSON string")
    }

    fn rest_json(&self, path: &str) -> Result<serde_json::Value> {
        let url = self.url(path);
        let response = self
            .client()?
            .get(&url)
            .send()
            .with_context(|| format!("Failed to reach local OTBR REST endpoint at {url}"))?
            .error_for_status()
            .with_context(|| format!("OTBR REST endpoint rejected {url}"))?;
        response
            .json()
            .with_context(|| format!("OTBR REST endpoint returned invalid JSON for {url}"))
    }

    fn rest_text(&self, path: &str) -> Result<String> {
        let url = self.url(path);
        self.client()?
            .get(&url)
            .header(reqwest::header::ACCEPT, "text/plain")
            .send()
            .with_context(|| format!("Failed to reach local OTBR REST endpoint at {url}"))?
            .error_for_status()
            .with_context(|| format!("OTBR REST endpoint rejected {url}"))?
            .text()
            .with_context(|| format!("OTBR REST endpoint returned invalid text for {url}"))
    }

    fn rest_put_json(&self, path: &str, body: &serde_json::Value) -> Result<()> {
        let url = self.url(path);
        self.client()?
            .put(&url)
            .json(body)
            .send()
            .with_context(|| format!("Failed to reach local OTBR REST endpoint at {url}"))?
            .error_for_status()
            .with_context(|| format!("OTBR REST endpoint rejected {url}"))?;
        Ok(())
    }

    fn rest_put_plain(&self, path: &str, body: &str) -> Result<()> {
        let url = self.url(path);
        self.client()?
            .put(&url)
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(body.to_string())
            .send()
            .with_context(|| format!("Failed to reach local OTBR REST endpoint at {url}"))?
            .error_for_status()
            .with_context(|| format!("OTBR REST endpoint rejected {url}"))?;
        Ok(())
    }

    fn client(&self) -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("Failed to initialize the OTBR REST client")
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.rest_endpoint)
    }
}

impl BorderRouter {
    /// Start OTBR and confirm that it did not fail immediately.
    pub fn start(config: BorderRouterConfig) -> Result<Self> {
        Self::start_inner(config, None, None)
    }

    /// Starts OTBR with a private D-Bus bus required by the agent's RCP mode,
    /// then returns its loopback REST controller.
    pub fn start_with_controller(
        config: BorderRouterConfig,
        controller: ThreadController,
    ) -> Result<(Self, ThreadController)> {
        let (daemon, address) = DbusDaemon::start()?;
        let thread_interface = config.thread_interface.clone();
        let router = Self::start_inner(config, Some(address.clone()), Some(daemon))?;
        Ok((
            router,
            controller.with_dbus_control(address, thread_interface),
        ))
    }

    fn start_inner(
        config: BorderRouterConfig,
        dbus_address: Option<String>,
        dbus_daemon: Option<DbusDaemon>,
    ) -> Result<Self> {
        validate_config(&config)?;
        let args = config.agent_args();
        info!(
            rcp = %config.rcp_device.display(),
            interface = %config.thread_interface,
            infrastructure_interface = %config.infrastructure_interface,
            "Starting OpenThread border router"
        );
        let log_path = config.data_path.join("otbr-agent.log");
        let log_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
            .with_context(|| format!("Failed to create OTBR log file: {}", log_path.display()))?;
        let stdout = log_file
            .try_clone()
            .context("Failed to duplicate OTBR log file handle")?;
        let mut command = Command::new(&config.agent_path);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(log_file);
        if let Some(address) = dbus_address {
            command.env("DBUS_SYSTEM_BUS_ADDRESS", address);
        }
        let mut child = command.spawn().with_context(|| {
            format!(
                "Failed to start OpenThread border router at {}",
                config.agent_path.display()
            )
        })?;

        // A number of agent configuration failures are reported just after
        // spawn rather than synchronously. Briefly observe the child so the
        // status endpoint does not claim a dead router is controllable.
        std::thread::sleep(Duration::from_millis(250));

        if let Some(status) = child
            .try_wait()
            .context("Failed to inspect OpenThread border router startup")?
        {
            bail!(
                "OpenThread border router exited immediately with status {status}: {}",
                startup_log_tail(&log_path)
            );
        }

        Ok(Self {
            child,
            config,
            dbus_daemon,
        })
    }

    #[must_use]
    pub fn config(&self) -> &BorderRouterConfig {
        &self.config
    }

    /// Returns whether the OTBR child process is still running.
    pub fn is_running(&mut self) -> Result<bool> {
        Ok(self
            .child
            .try_wait()
            .context("Failed to inspect OpenThread border router")?
            .is_none())
    }

    /// Stop OTBR when Extrittio exits. OTBR is a child of the hobby process and
    /// must not be left behind with routes pointing to a disconnected RCP.
    pub fn shutdown(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .context("Failed to inspect OpenThread border router")?
            .is_none()
        {
            self.child
                .kill()
                .context("Failed to stop OpenThread border router")?;
            self.child
                .wait()
                .context("Failed to wait for OpenThread border router shutdown")?;
        }
        self.stop_dbus_daemon();
        Ok(())
    }

    fn stop_dbus_daemon(&mut self) {
        if let Some(daemon) = self.dbus_daemon.take() {
            drop(daemon);
        }
    }
}

fn startup_log_tail(log_path: &Path) -> String {
    let Ok(log) = fs::read_to_string(log_path) else {
        return "no agent diagnostics were captured".to_string();
    };
    let tail = log
        .lines()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    if tail.is_empty() {
        "no agent diagnostics were captured".to_string()
    } else {
        tail
    }
}

impl DbusDaemon {
    fn start() -> Result<(Self, String)> {
        let output = Command::new("dbus-daemon")
            .args([
                "--session",
                // Homebrew's session configuration uses a launchd-provided
                // socket on macOS. OTBR needs an isolated bus instead, so
                // explicitly create a private Unix-domain socket on every
                // supported host.
                "--address=unix:tmpdir=/tmp",
                "--fork",
                "--print-address=1",
                "--print-pid=1",
                "--nopidfile",
            ])
            .output()
            .context("Failed to start the local D-Bus daemon required for OpenThread control")?;
        if !output.status.success() {
            bail!(
                "Failed to start the local D-Bus daemon required for OpenThread control: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let output = String::from_utf8_lossy(&output.stdout);
        let mut lines = output.lines();
        let address = lines
            .next()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .context("D-Bus daemon did not provide a bus address")?;
        let process_id = lines
            .next()
            .context("D-Bus daemon did not provide a process ID")?
            .trim()
            .parse()
            .context("D-Bus daemon returned an invalid process ID")?;
        Ok((Self { process_id }, address))
    }
}

impl Drop for DbusDaemon {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(self.process_id.to_string())
            .status();
    }
}

impl Drop for BorderRouter {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            warn!(%error, "Failed to stop OpenThread border router during shutdown");
        }
    }
}

/// Finds a directly connected serial RCP when there is exactly one plausible
/// candidate. Returns `None` instead of guessing when several devices exist.
pub fn discover_rcp() -> Result<Option<PathBuf>> {
    let candidates = serial_candidates()?;
    match candidates.as_slice() {
        [] => Ok(None),
        [candidate] => Ok(Some(candidate.clone())),
        _ => {
            warn!(
                candidates = ?candidates,
                "Multiple serial devices could be Thread RCPs; set --thread-rcp explicitly"
            );
            Ok(None)
        }
    }
}

/// Resolves the OTBR executable. Packaged hobby releases place it beside the
/// application; a user override and a PATH-installed copy remain useful during
/// local development.
pub fn discover_agent(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return executable_if_present(path).map(Some);
    }
    if let Some(path) = env::var_os("EXTRITTIO_OTBR_AGENT") {
        return executable_if_present(Path::new(&path)).map(Some);
    }

    for path in bundled_agent_paths() {
        if path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(find_in_path("otbr-agent"))
}

/// Selects the usual primary network interface for a standalone hobby host.
#[must_use]
pub fn default_infrastructure_interface() -> String {
    if cfg!(target_os = "macos") {
        Command::new("route")
            .args(["-n", "get", "default"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|output| {
                output.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix("interface:")
                        .map(str::trim)
                        .filter(|interface| !interface.is_empty())
                        .map(ToOwned::to_owned)
                })
            })
            .unwrap_or_else(|| "en0".to_string())
    } else {
        "eth0".to_string()
    }
}

fn validate_config(config: &BorderRouterConfig) -> Result<()> {
    if config.baud_rate == 0 {
        bail!("OpenThread RCP baud rate must be greater than zero");
    }
    if config.thread_interface.trim().is_empty()
        || config.infrastructure_interface.trim().is_empty()
    {
        bail!("OpenThread interface names must not be empty");
    }
    executable_if_present(&config.agent_path)?;
    fs::create_dir_all(&config.data_path).with_context(|| {
        format!(
            "Failed to create OpenThread data directory: {}",
            config.data_path.display()
        )
    })?;
    if !config.rcp_device.exists() {
        bail!(
            "OpenThread RCP device does not exist: {}",
            config.rcp_device.display()
        );
    }
    Ok(())
}

fn executable_if_present(path: &Path) -> Result<PathBuf> {
    if !path.is_file() {
        bail!(
            "OpenThread border-router executable is missing: {}",
            path.display()
        );
    }
    Ok(path.to_path_buf())
}

fn bundled_agent_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = env::current_exe()
        && let Some(bin_dir) = executable.parent()
    {
        paths.push(bin_dir.join("../libexec/extrittio/otbr-agent"));
        paths.push(bin_dir.join("otbr-agent"));
    }
    paths
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(program))
            .find(|candidate| candidate.is_file())
    })
}

fn thread_interface_ipv6_address(interface: &str) -> Result<Ipv6Addr> {
    let ip_output = Command::new("ip")
        .args(["-6", "address", "show", "dev", interface])
        .output();
    if let Ok(output) = ip_output
        && output.status.success()
        && let Some(address) = parse_thread_ipv6_addresses(&String::from_utf8_lossy(&output.stdout))
            .into_iter()
            .next()
    {
        return Ok(address);
    }

    let ifconfig_output = Command::new("ifconfig")
        .arg(interface)
        .output()
        .with_context(|| format!("Failed to inspect IPv6 addresses on {interface}"))?;
    anyhow::ensure!(
        ifconfig_output.status.success(),
        "Failed to inspect IPv6 addresses on {interface}: {}",
        String::from_utf8_lossy(&ifconfig_output.stderr).trim()
    );
    parse_thread_ipv6_addresses(&String::from_utf8_lossy(&ifconfig_output.stdout))
        .into_iter()
        .next()
        .context("OTBR has no usable IPv6 address on its Thread interface")
}

#[cfg(target_os = "macos")]
fn macos_mesh_local_ipv6_address(mesh_local_prefix: &str) -> Result<Ipv6Addr> {
    let prefix = mesh_local_prefix
        .trim()
        .split_once('/')
        .map_or(mesh_local_prefix.trim(), |(address, _)| address)
        .parse::<Ipv6Addr>()
        .context("OTBR returned an invalid mesh-local IPv6 prefix")?;
    let interfaces = Command::new("ifconfig")
        .arg("-l")
        .output()
        .context("Failed to list macOS network interfaces")?;
    anyhow::ensure!(
        interfaces.status.success(),
        "Failed to list macOS network interfaces: {}",
        String::from_utf8_lossy(&interfaces.stderr).trim()
    );

    for interface in String::from_utf8_lossy(&interfaces.stdout).split_whitespace() {
        if !interface.starts_with("utun") {
            continue;
        }
        let Ok(addresses) = thread_interface_ipv6_addresses(interface) else {
            continue;
        };
        if let Some(address) = addresses
            .into_iter()
            .find(|address| same_ipv6_prefix_64(*address, prefix))
        {
            return Ok(address);
        }
    }
    bail!("No macOS utun interface has an address in the OTBR mesh-local prefix")
}

fn thread_interface_ipv6_addresses(interface: &str) -> Result<Vec<Ipv6Addr>> {
    let output = Command::new("ifconfig")
        .arg(interface)
        .output()
        .with_context(|| format!("Failed to inspect IPv6 addresses on {interface}"))?;
    anyhow::ensure!(
        output.status.success(),
        "Failed to inspect IPv6 addresses on {interface}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(parse_thread_ipv6_addresses(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn same_ipv6_prefix_64(left: Ipv6Addr, right: Ipv6Addr) -> bool {
    left.octets()[..8] == right.octets()[..8]
}

fn parse_thread_ipv6_addresses(output: &str) -> Vec<Ipv6Addr> {
    output
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            while let Some(word) = words.next() {
                if word == "inet6" {
                    let value = words.next()?.split('%').next()?.split('/').next()?;
                    return value.parse::<Ipv6Addr>().ok();
                }
            }
            None
        })
        .filter(|address| {
            !address.is_unspecified()
                && !address.is_loopback()
                && !address.is_multicast()
                && !address.is_unicast_link_local()
        })
        .collect()
}

fn dataset_string(dataset: &serde_json::Value, key: &str) -> Option<String> {
    dataset.get(key)?.as_str().map(ToOwned::to_owned)
}

fn dataset_u16(dataset: &serde_json::Value, key: &str) -> Option<u16> {
    dataset
        .get(key)?
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThreadEnergyResult {
    channel: u16,
    max_rssi: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThreadChannelOccupancy {
    channel: u16,
    occupancy: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThreadMacCounters {
    tx_total: u32,
    rx_total: u32,
    tx_retries: u32,
    tx_errors: u32,
    rx_errors: u32,
}

fn parse_dbus_energy_scan(output: &str) -> Vec<ThreadEnergyResult> {
    output
        .split("struct {")
        .skip(1)
        .filter_map(|entry| entry.split('}').next())
        .filter_map(|entry| {
            let bytes = dbus_values::<u8>(entry, "byte");
            Some(ThreadEnergyResult {
                channel: u16::from(*bytes.first()?),
                max_rssi: i16::from(*bytes.get(1)? as i8),
            })
        })
        .collect()
}

fn parse_dbus_channel_occupancy(output: &str) -> Vec<ThreadChannelOccupancy> {
    output
        .split("struct {")
        .skip(1)
        .filter_map(|entry| entry.split('}').next())
        .filter_map(|entry| {
            Some(ThreadChannelOccupancy {
                channel: u16::from(*dbus_values::<u8>(entry, "byte").first()?),
                occupancy: *dbus_values::<u16>(entry, "uint16").first()?,
            })
        })
        .collect()
}

fn parse_dbus_scalar<T>(output: &str, kind: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    dbus_values(output, kind).into_iter().next()
}

fn parse_dbus_byte(output: &str) -> Option<i16> {
    parse_dbus_scalar::<u8>(output, "byte").map(|value| i16::from(value as i8))
}

fn parse_dbus_mac_counters(output: &str) -> Option<ThreadMacCounters> {
    let values = dbus_values::<u32>(output, "uint32");
    Some(ThreadMacCounters {
        tx_total: *values.first()?,
        tx_retries: *values.get(11)?,
        tx_errors: values
            .get(12..=14)?
            .iter()
            .copied()
            .fold(0, u32::saturating_add),
        rx_total: *values.get(15)?,
        rx_errors: values
            .get(26..=31)?
            .iter()
            .copied()
            .fold(0, u32::saturating_add),
    })
}

fn dbus_values<T>(output: &str, kind: &str) -> Vec<T>
where
    T: std::str::FromStr,
{
    output
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix(kind))
        .filter_map(|value| value.trim().parse().ok())
        .collect()
}

fn parse_dbus_scan(output: &str) -> Vec<ThreadNetwork> {
    output
        .split("struct {")
        .skip(1)
        .filter_map(|entry| entry.split('}').next())
        .filter_map(parse_dbus_scan_entry)
        .collect()
}

fn parse_dbus_scan_entry(entry: &str) -> Option<ThreadNetwork> {
    let mut ext_address = None;
    let mut network_name = None;
    let mut pan_id = None;
    let mut channel = None;
    let mut rssi = None;
    let mut lqi = None;

    for line in entry.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("uint64 ") {
            ext_address.get_or_insert_with(|| value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("string ") {
            if network_name.is_none() {
                network_name = serde_json::from_str(value.trim()).ok();
            }
        } else if let Some(value) = line.strip_prefix("uint16 ") {
            if pan_id.is_none() {
                pan_id = value
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .map(|value| format!("{value:04x}"));
            }
        } else if let Some(value) = line.strip_prefix("byte ") {
            if channel.is_none() {
                channel = value.trim().parse().ok();
            } else if lqi.is_none() {
                lqi = value.trim().parse().ok();
            }
        } else if let Some(value) = line.strip_prefix("int16 ") {
            rssi = value.trim().parse().ok();
        }
    }

    Some(ThreadNetwork {
        network_name,
        pan_id: pan_id?,
        extended_address: format!("{:016x}", ext_address?.parse::<u64>().ok()?),
        channel: channel?,
        rssi: rssi?,
        lqi: lqi?,
    })
}

fn parse_thread_mesh_devices(value: &serde_json::Value) -> Result<Vec<ThreadMeshDevice>> {
    let items = value
        .get("data")
        .and_then(serde_json::Value::as_array)
        .context("OTBR mesh inventory is missing its data collection")?;

    items
        .iter()
        .map(|item| {
            let id = item
                .get("id")
                .and_then(serde_json::Value::as_str)
                .context("OTBR mesh device is missing its identifier")?
                .to_string();
            let item_type = item
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("threadDevice");
            let attributes = item
                .get("attributes")
                .and_then(serde_json::Value::as_object)
                .context("OTBR mesh device is missing its attributes")?;
            let mode = attributes
                .get("mode")
                .and_then(serde_json::Value::as_object);
            let leader_data = attributes
                .get("leaderData")
                .and_then(serde_json::Value::as_object);

            Ok(ThreadMeshDevice {
                id,
                is_border_router: item_type == "threadBorderRouter",
                extended_address: json_object_string(attributes, "extAddress"),
                mesh_local_eid_iid: json_object_string(attributes, "mlEidIid"),
                omr_ipv6_addresses: json_object_strings(attributes, "omrIpv6Address"),
                hostname: json_object_string(attributes, "hostname"),
                eui64: json_object_string(attributes, "eui"),
                role: json_object_string(attributes, "role")
                    .or_else(|| json_object_string(attributes, "state")),
                full_thread_device: mode
                    .and_then(|mode| json_object_bool(mode, "fullThreadDevice")),
                rx_on_when_idle: mode.and_then(|mode| json_object_bool(mode, "rxOnWhenIdle")),
                full_network_data: mode.and_then(|mode| json_object_bool(mode, "fullNetworkData")),
                rloc16: json_object_string(attributes, "rloc16"),
                rloc_address: json_object_string(attributes, "rlocAddress"),
                router_id: json_object_u16(attributes, "routerId"),
                router_count: json_object_u16(attributes, "routerCount"),
                network_name: json_object_string(attributes, "networkName"),
                extended_pan_id: json_object_string(attributes, "extPanId"),
                border_agent_id: json_object_string(attributes, "baId"),
                border_agent_state: json_object_string(attributes, "baState"),
                partition_id: leader_data.and_then(|data| json_object_u32(data, "partitionId")),
                leader_router_id: leader_data
                    .and_then(|data| json_object_u16(data, "leaderRouterId")),
                data_version: leader_data.and_then(|data| json_object_u16(data, "dataVersion")),
                stable_data_version: leader_data
                    .and_then(|data| json_object_u16(data, "stableDataVersion")),
                created_at: json_object_string(attributes, "created"),
                updated_at: json_object_string(attributes, "updated"),
            })
        })
        .collect()
}

fn json_object_string(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    object
        .get(key)?
        .as_str()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn json_object_strings(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Vec<String> {
    match object.get(key) {
        Some(serde_json::Value::String(value)) if !value.is_empty() => vec![value.clone()],
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn json_object_bool(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<bool> {
    object.get(key)?.as_bool()
}

fn json_object_u16(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<u16> {
    object
        .get(key)?
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
}

fn json_object_u32(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<u32> {
    object
        .get(key)?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
}

fn strip_hex_prefix(value: &str) -> &str {
    value.trim().trim_start_matches("0x")
}

fn parse_pan_id(value: &str) -> Result<u16> {
    u16::from_str_radix(strip_hex_prefix(value), 16).context("Thread PAN ID must be hexadecimal")
}

fn validate_create_network(network: &CreateNetwork) -> Result<()> {
    let name = network.network_name.trim();
    if name.is_empty() || name.len() > 16 || !name.is_ascii() {
        bail!("Thread network name must be 1 to 16 ASCII characters");
    }
    if let Some(channel) = network.channel
        && !(11..=26).contains(&channel)
    {
        bail!("Thread channel must be between 11 and 26");
    }
    validate_hex("Thread PAN ID", network.pan_id.as_deref(), 4)?;
    validate_hex(
        "Thread extended PAN ID",
        network.extended_pan_id.as_deref(),
        16,
    )?;
    validate_hex("Thread network key", network.network_key.as_deref(), 32)?;
    Ok(())
}

fn validate_hex(label: &str, value: Option<&str>, length: usize) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let value = strip_hex_prefix(value);
    if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("{label} must be {length} hexadecimal characters");
    }
    Ok(())
}

fn serial_candidates() -> Result<Vec<PathBuf>> {
    #[cfg(target_os = "macos")]
    let roots = ["/dev"];
    #[cfg(target_os = "linux")]
    let roots = ["/dev/serial/by-id", "/dev"];
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let roots: [&str; 0] = [];

    let mut candidates = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries {
            let entry = entry.context("Failed to inspect serial device")?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_rcp_candidate_name(&name) {
                candidates.push(entry.path());
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn is_rcp_candidate_name(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    return name.starts_with("cu.usbmodem") || name.starts_with("cu.usbserial");
    #[cfg(target_os = "linux")]
    return name.contains("Nordic")
        || name.contains("nRF")
        || name.starts_with("ttyACM")
        || name.starts_with("ttyUSB");
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    false
}

#[cfg(test)]
mod tests {
    use super::{
        BorderRouterConfig, CreateNetwork, DEFAULT_DEVELOPMENT_DATASET_TLVS,
        default_infrastructure_interface, parse_thread_ipv6_addresses, validate_create_network,
    };
    use std::{net::Ipv6Addr, path::PathBuf};

    #[test]
    fn builds_the_standard_spinel_uart_url_and_otbr_arguments() {
        let config = BorderRouterConfig {
            agent_path: "/opt/extrittio/libexec/otbr-agent".into(),
            rcp_device: PathBuf::from("/dev/cu.usbmodem14101"),
            baud_rate: 460_800,
            thread_interface: "wpan0".into(),
            infrastructure_interface: "en0".into(),
            data_path: "/tmp/extrittio-thread-test".into(),
        };

        assert_eq!(
            config.radio_url(),
            "spinel+hdlc+uart:///dev/cu.usbmodem14101?uart-baudrate=460800"
        );
        assert_eq!(
            config.agent_args(),
            vec![
                "-I",
                "wpan0",
                "-B",
                "en0",
                "--vendor-name",
                "Extrittio",
                "--model-name",
                "Hobby Appliance",
                "--syslog-disable",
                "--data-path",
                "/tmp/extrittio-thread-test",
                "spinel+hdlc+uart:///dev/cu.usbmodem14101?uart-baudrate=460800"
            ]
        );
    }

    #[test]
    fn chooses_a_sensible_default_infrastructure_interface() {
        assert!(!default_infrastructure_interface().is_empty());
    }

    #[test]
    fn validates_create_network_inputs_without_accepting_malformed_credentials() {
        let valid = CreateNetwork {
            network_name: "Extrittio-Thread".into(),
            channel: Some(15),
            pan_id: Some("1234".into()),
            extended_pan_id: Some("0011223344556677".into()),
            network_key: Some("00112233445566778899aabbccddeeff".into()),
        };
        assert!(validate_create_network(&valid).is_ok());

        let invalid_channel = CreateNetwork {
            channel: Some(27),
            ..valid.clone()
        };
        assert!(validate_create_network(&invalid_channel).is_err());

        let invalid_key = CreateNetwork {
            network_key: Some("not-a-network-key".into()),
            ..valid
        };
        assert!(validate_create_network(&invalid_key).is_err());
    }

    #[test]
    fn bundled_development_dataset_is_valid_hex() {
        assert!(DEFAULT_DEVELOPMENT_DATASET_TLVS.len().is_multiple_of(2));
        assert!(
            DEFAULT_DEVELOPMENT_DATASET_TLVS
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
    }

    #[test]
    fn parses_rcp_dbus_scan_results_without_network_credentials() {
        let output = r#"
array [
   struct {
      uint64 4822678189205111
      string "Example"
      uint64 0
      array [ ]
      uint16 4660
      uint16 0
      byte 15
      int16 -28
      byte 3
      byte 4
      boolean false
      boolean false
   }
]
"#;

        assert_eq!(
            super::parse_dbus_scan(output),
            vec![super::ThreadNetwork {
                network_name: Some("Example".into()),
                pan_id: "1234".into(),
                extended_address: "0011223344556677".into(),
                channel: 15,
                rssi: -28,
                lqi: 3,
            }]
        );
    }

    #[test]
    fn parses_rcp_energy_scan_signed_rssi_values() {
        let output = r#"
array [
   struct {
      byte 11
      byte 197
   }
   struct {
      byte 12
      byte 169
   }
]
"#;

        assert_eq!(
            super::parse_dbus_energy_scan(output),
            vec![
                super::ThreadEnergyResult {
                    channel: 11,
                    max_rssi: -59,
                },
                super::ThreadEnergyResult {
                    channel: 12,
                    max_rssi: -87,
                },
            ]
        );
    }

    #[test]
    fn parses_channel_monitor_occupancy_and_radio_counters() {
        let occupancy = r#"
variant array [
   struct {
      byte 11
      uint16 8192
   }
   struct {
      byte 12
      uint16 49151
   }
]
"#;
        assert_eq!(
            super::parse_dbus_channel_occupancy(occupancy),
            vec![
                super::ThreadChannelOccupancy {
                    channel: 11,
                    occupancy: 8192,
                },
                super::ThreadChannelOccupancy {
                    channel: 12,
                    occupancy: 49151,
                },
            ]
        );

        let counters = (0..32)
            .map(|value| format!("uint32 {value}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            super::parse_dbus_mac_counters(&counters),
            Some(super::ThreadMacCounters {
                tx_total: 0,
                tx_retries: 11,
                tx_errors: 39,
                rx_total: 15,
                rx_errors: 171,
            })
        );
    }

    #[test]
    fn parses_otbr_mesh_inventory_without_operational_credentials() {
        let inventory = serde_json::json!({
            "data": [
                {
                    "id": "96518e5497d5b9f3",
                    "type": "threadBorderRouter",
                    "attributes": {
                        "extAddress": "96518e5497d5b9f3",
                        "mlEidIid": "731f529f1266a17d",
                        "omrIpv6Address": "fd11:22::1",
                        "hostname": "extrittio.local",
                        "role": "leader",
                        "mode": {
                            "fullThreadDevice": true,
                            "rxOnWhenIdle": true,
                            "fullNetworkData": true
                        },
                        "rloc16": "0xf000",
                        "routerId": 60,
                        "routerCount": 2,
                        "rlocAddress": "fd35:3441:33d1:d73e:0:ff:fe00:f000",
                        "networkName": "Extrittio-Thread",
                        "extPanId": "ef1398c2fd504b67",
                        "baId": "e11e23c164311ce642f93297b095b2f8",
                        "baState": "active",
                        "leaderData": {
                            "partitionId": 1794764107,
                            "dataVersion": 64,
                            "stableDataVersion": 63,
                            "leaderRouterId": 60
                        },
                        "created": "2026-08-13T12:00:00Z"
                    }
                },
                {
                    "id": "2a55d952bc7b4008",
                    "type": "threadDevice",
                    "attributes": {
                        "extAddress": "2a55d952bc7b4008",
                        "omrIpv6Address": ["fd11:22::2", "fd11:22::3"],
                        "role": "child",
                        "mode": {
                            "fullThreadDevice": false,
                            "rxOnWhenIdle": false,
                            "fullNetworkData": true
                        }
                    }
                }
            ]
        });

        let devices = super::parse_thread_mesh_devices(&inventory).unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices[0].is_border_router);
        assert_eq!(devices[0].network_name.as_deref(), Some("Extrittio-Thread"));
        assert_eq!(devices[0].router_count, Some(2));
        assert_eq!(devices[0].partition_id, Some(1_794_764_107));
        assert_eq!(
            devices[1].omr_ipv6_addresses,
            vec!["fd11:22::2", "fd11:22::3"]
        );
        assert_eq!(devices[1].rx_on_when_idle, Some(false));
    }

    #[test]
    fn selects_only_mesh_reachable_ipv6_addresses_for_dns_sd() {
        let addresses = parse_thread_ipv6_addresses(
            "inet6 ::1 prefixlen 128\ninet6 fe80::1234%wpan0 prefixlen 64\ninet6 fd12:3456::20/64 scope global\n",
        );
        assert_eq!(
            addresses,
            vec!["fd12:3456::20".parse::<Ipv6Addr>().unwrap()]
        );
    }

    #[test]
    fn compares_mesh_local_ipv6_prefixes_at_sixty_four_bits() {
        assert!(super::same_ipv6_prefix_64(
            "fd35:3441:33d1:d73e::1".parse().unwrap(),
            "fd35:3441:33d1:d73e::".parse().unwrap(),
        ));
        assert!(!super::same_ipv6_prefix_64(
            "fd35:3441:33d1:d73f::1".parse().unwrap(),
            "fd35:3441:33d1:d73e::".parse().unwrap(),
        ));
    }
}
