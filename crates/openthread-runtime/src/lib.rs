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
    time::Duration,
};

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

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

    #[must_use]
    pub fn controller(&self) -> Option<Arc<ThreadController>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .as_ref()
            .map(|active| active.controller.clone())
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
        let thread_interface = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .as_ref()
            .map(|active| active.router.config().thread_interface.clone())
            .context("The local OpenThread border router is unavailable")?;
        thread_interface_ipv6_address(&thread_interface)
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
        let address = self
            .dbus_address
            .as_deref()
            .context("The local OTBR RCP control bus is unavailable")?;
        let service = format!("io.openthread.BorderRouter.{}", self.thread_interface);
        let object = format!("/io/openthread/BorderRouter/{}", self.thread_interface);
        Command::new("dbus-send")
            // OTBR connects with `dbus_bus_get(DBUS_BUS_SYSTEM)`. Point that
            // lookup at our private daemon so the client uses the identical
            // system-bus handshake instead of a direct peer connection.
            .env("DBUS_SYSTEM_BUS_ADDRESS", address)
            .args([
                "--system".to_string(),
                "--print-reply".to_string(),
                "--reply-timeout=35000".to_string(),
                format!("--dest={service}"),
                object,
                "io.openthread.BorderRouter.Scan".to_string(),
            ])
            .output()
            .context("Failed to run the OTBR RCP scan controller")
            .and_then(|output| {
                if !output.status.success() {
                    bail!(
                        "OpenThread RCP scan failed: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    );
                }
                Ok(parse_dbus_scan(&String::from_utf8_lossy(&output.stdout)))
            })
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
        BorderRouterConfig, CreateNetwork, default_infrastructure_interface,
        parse_thread_ipv6_addresses, validate_create_network,
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
    fn selects_only_mesh_reachable_ipv6_addresses_for_dns_sd() {
        let addresses = parse_thread_ipv6_addresses(
            "inet6 ::1 prefixlen 128\ninet6 fe80::1234%wpan0 prefixlen 64\ninet6 fd12:3456::20/64 scope global\n",
        );
        assert_eq!(
            addresses,
            vec!["fd12:3456::20".parse::<Ipv6Addr>().unwrap()]
        );
    }
}
