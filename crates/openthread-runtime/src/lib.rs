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
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
/// `ot-ctl` is the supported OpenThread CLI client for an OTBR agent. Keeping
/// all calls behind this type prevents the web application from ever accepting
/// arbitrary CLI input and serializes state-changing dataset operations.
#[derive(Clone)]
pub struct ThreadController {
    ot_ctl_path: PathBuf,
    command_lock: Arc<Mutex<()>>,
    dbus_address: Option<String>,
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
    pub fn discover(agent_path: &Path, explicit: Option<&Path>) -> Result<Option<Self>> {
        let path = match explicit {
            Some(path) => executable_if_present(path)?,
            None => match discover_ctl(agent_path) {
                Some(path) => path,
                None => return Ok(None),
            },
        };
        Ok(Some(Self {
            ot_ctl_path: path,
            command_lock: Arc::new(Mutex::new(())),
            dbus_address: None,
        }))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.ot_ctl_path
    }

    fn with_dbus_address(mut self, address: String) -> Self {
        self.dbus_address = Some(address);
        self
    }

    pub fn status(&self) -> Result<ThreadStatus> {
        let role = single_value(&self.run(&["state"])?);
        let dataset = self.run(&["dataset", "active"])?;
        let addresses = self
            .run(&["ipaddr"])?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != "Done")
            .map(ToOwned::to_owned)
            .collect();

        Ok(ThreadStatus {
            role,
            network_name: dataset_value(&dataset, "Network Name"),
            channel: dataset_value(&dataset, "Channel").and_then(|value| value.parse().ok()),
            pan_id: dataset_value(&dataset, "PAN ID"),
            extended_pan_id: dataset_value(&dataset, "Ext PAN ID"),
            mesh_local_prefix: dataset_value(&dataset, "Mesh Local Prefix"),
            addresses,
        })
    }

    /// Performs an active scan using the local radio and returns nearby Thread
    /// networks. This does not alter the active operational dataset.
    pub fn scan_networks(&self) -> Result<Vec<ThreadNetwork>> {
        Ok(parse_network_scan(&self.run(&["scan"])?))
    }

    /// Forms a new Thread mesh. Existing devices will be detached, so callers
    /// must obtain explicit confirmation before invoking this operation.
    pub fn create_network(&self, network: &CreateNetwork) -> Result<()> {
        validate_create_network(network)?;
        let _guard = self.lock();
        self.run_locked(&["thread", "stop"])?;
        self.run_locked(&["ifconfig", "down"])?;
        self.run_locked(&["dataset", "init", "new"])?;
        self.run_locked(&["dataset", "networkname", &network.network_name])?;
        if let Some(channel) = network.channel {
            self.run_locked(&["dataset", "channel", &channel.to_string()])?;
        }
        if let Some(pan_id) = &network.pan_id {
            self.run_locked(&["dataset", "panid", pan_id])?;
        }
        if let Some(extended_pan_id) = &network.extended_pan_id {
            self.run_locked(&["dataset", "extpanid", extended_pan_id])?;
        }
        if let Some(network_key) = &network.network_key {
            self.run_locked(&["dataset", "networkkey", network_key])?;
        }
        self.activate_locked()
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
        self.run_locked(&["thread", "stop"])?;
        self.run_locked(&["ifconfig", "down"])?;
        self.run_locked(&["dataset", "set", "active", dataset_tlvs])?;
        self.activate_locked()
    }

    fn activate_locked(&self) -> Result<()> {
        self.run_locked(&["dataset", "commit", "active"])?;
        self.run_locked(&["ifconfig", "up"])?;
        self.run_locked(&["thread", "start"])?;
        Ok(())
    }

    fn run(&self, arguments: &[&str]) -> Result<String> {
        let _guard = self.lock();
        self.run_locked(arguments)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.command_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn run_locked(&self, arguments: &[&str]) -> Result<String> {
        let mut command = Command::new(&self.ot_ctl_path);
        command
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(address) = &self.dbus_address {
            command.env("DBUS_SYSTEM_BUS_ADDRESS", address);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("Failed to run {}", self.ot_ctl_path.display()))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child
                .try_wait()
                .context("Failed to inspect OpenThread command")?
                .is_some()
            {
                break;
            }
            if Instant::now() >= deadline {
                child
                    .kill()
                    .context("Failed to stop a timed-out OpenThread command")?;
                let _ = child.wait();
                bail!("OpenThread command timed out after 5 seconds");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let output = child
            .wait_with_output()
            .context("Failed to collect OpenThread command output")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("OpenThread command failed: {}", stderr.trim());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl BorderRouter {
    /// Start OTBR and confirm that it did not fail immediately.
    pub fn start(config: BorderRouterConfig) -> Result<Self> {
        Self::start_inner(config, None, None)
    }

    /// Starts OTBR with a private D-Bus control bus and returns the paired
    /// `ot-ctl` controller. This avoids exposing or depending on the host's
    /// system D-Bus service.
    pub fn start_with_controller(
        config: BorderRouterConfig,
        controller: ThreadController,
    ) -> Result<(Self, ThreadController)> {
        let (daemon, address) = DbusDaemon::start()?;
        let router = Self::start_inner(config, Some(address.clone()), Some(daemon))?;
        Ok((router, controller.with_dbus_address(address)))
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
        let mut command = Command::new(&config.agent_path);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
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
            bail!("OpenThread border router exited immediately with status {status}");
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
pub fn default_infrastructure_interface() -> &'static str {
    if cfg!(target_os = "macos") {
        "en0"
    } else {
        "eth0"
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
    if let Ok(executable) = env::current_exe() {
        if let Some(bin_dir) = executable.parent() {
            paths.push(bin_dir.join("../libexec/extrittio/otbr-agent"));
            paths.push(bin_dir.join("otbr-agent"));
        }
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

fn discover_ctl(agent_path: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(directory) = agent_path.parent() {
        candidates.push(directory.join("ot-ctl"));
    }
    candidates.extend(bundled_ctl_paths());
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .or_else(|| find_in_path("ot-ctl"))
}

fn bundled_ctl_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(executable) = env::current_exe()
        && let Some(bin_dir) = executable.parent()
    {
        paths.push(bin_dir.join("../libexec/extrittio/ot-ctl"));
        paths.push(bin_dir.join("ot-ctl"));
    }
    paths
}

fn single_value(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && *line != "Done")
        .map(ToOwned::to_owned)
}

fn dataset_value(dataset: &str, key: &str) -> Option<String> {
    dataset.lines().find_map(|line| {
        line.trim()
            .strip_prefix(key)
            .and_then(|value| value.strip_prefix(':'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn parse_network_scan(output: &str) -> Vec<ThreadNetwork> {
    let mut header = None;
    let mut networks = Vec::new();

    for line in output.lines().map(str::trim) {
        if !line.starts_with('|') {
            continue;
        }
        let columns = scan_columns(line);
        if columns.is_empty() {
            continue;
        }
        if header.is_none() {
            if columns
                .iter()
                .any(|column| *column == "PAN" || *column == "PAN ID")
            {
                header = Some(columns);
            }
            continue;
        }

        let Some(network) = parse_scan_row(header.as_deref().unwrap_or_default(), &columns) else {
            continue;
        };
        networks.push(network);
    }

    networks
}

fn scan_columns(line: &str) -> Vec<&str> {
    line.trim_matches('|').split('|').map(str::trim).collect()
}

fn parse_scan_row(header: &[&str], row: &[&str]) -> Option<ThreadNetwork> {
    if row.len() != header.len() || row.iter().all(|column| column.chars().all(|ch| ch == '-')) {
        return None;
    }

    let value = |name| {
        header
            .iter()
            .position(|column| *column == name)
            .and_then(|index| row.get(index).copied())
            .filter(|value| !value.is_empty())
    };

    Some(ThreadNetwork {
        pan_id: value("PAN").or_else(|| value("PAN ID"))?.to_owned(),
        extended_address: value("MAC Address")
            .or_else(|| value("Extended Address"))?
            .to_owned(),
        channel: value("Ch")?.parse().ok()?,
        rssi: value("dBm")?.parse().ok()?,
        lqi: value("LQI")?.parse().ok()?,
    })
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
    let value = value.trim().trim_start_matches("0x");
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
        BorderRouterConfig, CreateNetwork, dataset_value, default_infrastructure_interface,
        parse_network_scan, validate_create_network,
    };
    use std::path::PathBuf;

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
        assert!(matches!(default_infrastructure_interface(), "en0" | "eth0"));
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
    fn parses_only_requested_non_secret_dataset_values() {
        let dataset = "Network Name: Extrittio-Thread\nNetwork Key: secret\nPAN ID: 0x1234\nDone\n";
        assert_eq!(
            dataset_value(dataset, "Network Name"),
            Some("Extrittio-Thread".into())
        );
        assert_eq!(dataset_value(dataset, "PAN ID"), Some("0x1234".into()));
    }

    #[test]
    fn parses_active_scan_results_without_credentials() {
        let scan = "\
| PAN  | MAC Address      | Ch | dBm | LQI |\n\
+------+------------------+----+-----+-----+\n\
| 1234 | 0011223344556677 | 15 | -28 | 3   |\n\
| abcd | 8899aabbccddeeff | 20 | -74 | 1   |\n\
Done\n";

        assert_eq!(
            parse_network_scan(scan),
            vec![
                super::ThreadNetwork {
                    pan_id: "1234".into(),
                    extended_address: "0011223344556677".into(),
                    channel: 15,
                    rssi: -28,
                    lqi: 3,
                },
                super::ThreadNetwork {
                    pan_id: "abcd".into(),
                    extended_address: "8899aabbccddeeff".into(),
                    channel: 20,
                    rssi: -74,
                    lqi: 1,
                },
            ]
        );
    }
}
