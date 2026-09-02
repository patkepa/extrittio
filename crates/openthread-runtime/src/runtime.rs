use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    net::Ipv6Addr,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use crate::{
    controller::ThreadControl,
    dataset::parse_imported_dataset,
    discovery::{
        discover_agent, discover_rcp, serial_candidate_details, thread_interface_ipv6_address,
    },
    error::{OpenThreadError, Result},
    process::{BorderRouter, BorderRouterConfig, ManagedBorderRouter},
    types::{
        CreateNetwork, ThreadActiveDataset, ThreadObservationState, ThreadRcpCandidate,
        ThreadRuntimeConfig, ThreadRuntimePhase, ThreadRuntimeSnapshot, ThreadScan,
        ThreadScanSnapshot, ThreadScanSource, ThreadStatus,
    },
};
use tracing::{info, warn};

const HEALTH_FAILURE_THRESHOLD: u32 = 3;
const INITIAL_RESTART_BACKOFF: Duration = Duration::from_secs(5);
const MAX_RESTART_BACKOFF: Duration = Duration::from_secs(5 * 60);
const RUNTIME_SETTINGS_FILE: &str = "runtime-settings.json";

pub struct ThreadRuntime {
    config: RwLock<ThreadRuntimeConfig>,
    backend: Arc<dyn RuntimeBackend>,
    policy: SupervisorPolicy,
    lifecycle: RwLock<()>,
    state: Mutex<ThreadRuntimeState>,
    scan_state: Mutex<ThreadScanState>,
    scan_changed: Condvar,
    network_generation: AtomicU64,
}

struct ThreadRuntimeState {
    active: Option<ActiveThreadRuntime>,
    snapshot: ThreadRuntimeSnapshot,
    consecutive_health_failures: u32,
    startup_failures: u32,
    retry_not_before: Option<Instant>,
    next_retry_at: Option<SystemTime>,
    restart_count: u64,
    ever_started: bool,
    last_exit: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct PersistedRuntimeSettings {
    rcp_device: Option<PathBuf>,
}

struct ActiveThreadRuntime {
    router: Box<dyn ManagedBorderRouter>,
    controller: Arc<dyn ThreadControl>,
}

#[derive(Clone, Copy)]
struct SupervisorPolicy {
    health_failure_threshold: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
    jitter: bool,
}

impl Default for SupervisorPolicy {
    fn default() -> Self {
        Self {
            health_failure_threshold: HEALTH_FAILURE_THRESHOLD,
            initial_backoff: INITIAL_RESTART_BACKOFF,
            max_backoff: MAX_RESTART_BACKOFF,
            jitter: true,
        }
    }
}

trait RuntimeBackend: Send + Sync {
    fn discover_rcp(&self) -> Result<Option<PathBuf>>;
    fn serial_candidates(&self) -> Result<Vec<ThreadRcpCandidate>>;
    fn discover_agent(&self, explicit: Option<&Path>) -> Result<Option<PathBuf>>;
    fn start_border_router(
        &self,
        config: BorderRouterConfig,
    ) -> Result<(Box<dyn ManagedBorderRouter>, Arc<dyn ThreadControl>)>;
    fn thread_ipv6_address(
        &self,
        interface: &str,
        mesh_local_prefix: Option<&str>,
    ) -> Result<Ipv6Addr>;
}

struct SystemRuntimeBackend;

impl RuntimeBackend for SystemRuntimeBackend {
    fn discover_rcp(&self) -> Result<Option<PathBuf>> {
        discover_rcp()
    }

    fn serial_candidates(&self) -> Result<Vec<ThreadRcpCandidate>> {
        serial_candidate_details()
    }

    fn discover_agent(&self, explicit: Option<&Path>) -> Result<Option<PathBuf>> {
        discover_agent(explicit)
    }

    fn start_border_router(
        &self,
        config: BorderRouterConfig,
    ) -> Result<(Box<dyn ManagedBorderRouter>, Arc<dyn ThreadControl>)> {
        let (router, controller) = BorderRouter::start(config)?;
        Ok((Box::new(router), Arc::new(controller)))
    }

    fn thread_ipv6_address(
        &self,
        interface: &str,
        mesh_local_prefix: Option<&str>,
    ) -> Result<Ipv6Addr> {
        match thread_interface_ipv6_address(interface, mesh_local_prefix) {
            Ok(address) => Ok(address),
            Err(primary_error) => {
                #[cfg(target_os = "macos")]
                if let Some(prefix) = mesh_local_prefix {
                    return crate::discovery::macos_mesh_local_ipv6_address(prefix);
                }
                Err(primary_error)
            }
        }
    }
}

struct ThreadScanState {
    scan: Option<ThreadScan>,
    scanning: bool,
    generation: u64,
    completed_at: Option<SystemTime>,
    completed_at_instant: Option<Instant>,
    last_attempt_at: Option<Instant>,
    error: Option<String>,
}

impl ThreadRuntime {
    #[must_use]
    pub fn new(mut config: ThreadRuntimeConfig) -> Self {
        if config.rcp_device.is_none() {
            match load_runtime_settings(&config.data_path) {
                Ok(settings) => config.rcp_device = settings.rcp_device,
                Err(error) => warn!(%error, "Ignoring unreadable OpenThread runtime settings"),
            }
        }
        Self::new_with_backend(
            config,
            Arc::new(SystemRuntimeBackend),
            SupervisorPolicy::default(),
        )
    }

    fn new_with_backend(
        config: ThreadRuntimeConfig,
        backend: Arc<dyn RuntimeBackend>,
        policy: SupervisorPolicy,
    ) -> Self {
        let rcp_device = config.rcp_device.clone();
        Self {
            config: RwLock::new(config),
            backend,
            policy,
            lifecycle: RwLock::new(()),
            state: Mutex::new(ThreadRuntimeState {
                active: None,
                snapshot: ThreadRuntimeSnapshot {
                    available: false,
                    rcp_device,
                    message: Some("Thread runtime has not checked for an RCP yet".to_string()),
                    phase: ThreadRuntimePhase::Pending,
                    consecutive_failures: 0,
                    restart_count: 0,
                    next_retry_at: None,
                    last_exit: None,
                },
                consecutive_health_failures: 0,
                startup_failures: 0,
                retry_not_before: None,
                next_retry_at: None,
                restart_count: 0,
                ever_started: false,
                last_exit: None,
            }),
            scan_state: Mutex::new(ThreadScanState {
                scan: None,
                scanning: false,
                generation: 0,
                completed_at: None,
                completed_at_instant: None,
                last_attempt_at: None,
                error: None,
            }),
            scan_changed: Condvar::new(),
            network_generation: AtomicU64::new(0),
        }
    }

    /// Rechecks the RCP and both OTBR control endpoints, restarting an
    /// unhealthy process even when its PID and serial path still exist.
    #[must_use]
    pub fn refresh(&self) -> ThreadRuntimeSnapshot {
        let config = self.runtime_config();
        let _lifecycle = self.write_lifecycle();
        let now = Instant::now();
        let previous = {
            let mut state = self.lock_state();
            if state.active.is_none()
                && state
                    .retry_not_before
                    .is_some_and(|retry_not_before| retry_not_before > now)
            {
                return state.snapshot.clone();
            }
            state.retry_not_before = None;
            state.next_retry_at = None;
            state.active.take()
        };
        if let Some(mut active) = previous {
            let rcp_device = active.router.rcp_device().to_path_buf();
            let selected_device_changed = config
                .rcp_device
                .as_deref()
                .is_some_and(|selected| !paths_refer_to_same_device(selected, &rcp_device));
            let mut failure_reason = if selected_device_changed {
                Some(format!(
                    "The configured Thread RCP changed from {}",
                    rcp_device.display()
                ))
            } else if !rcp_device.exists() {
                Some(format!(
                    "The configured Thread RCP was removed: {}",
                    rcp_device.display()
                ))
            } else {
                match active.router.poll_exit() {
                    Ok(Some(status)) => Some(format!(
                        "otbr-agent exited with status {status}: {}",
                        active.router.diagnostic_tail()
                    )),
                    Ok(None) => None,
                    Err(error) => Some(format!("Unable to inspect otbr-agent: {error}")),
                }
            };
            if failure_reason.is_none() {
                match active.controller.health_check() {
                    Ok(()) => {
                        let mut state = self.lock_state();
                        state.consecutive_health_failures = 0;
                        state.startup_failures = 0;
                        state.active = Some(active);
                        let snapshot = state.make_snapshot(
                            true,
                            Some(rcp_device),
                            None,
                            ThreadRuntimePhase::Ready,
                        );
                        state.snapshot = snapshot.clone();
                        return snapshot;
                    }
                    Err(error) => {
                        let mut state = self.lock_state();
                        state.consecutive_health_failures =
                            state.consecutive_health_failures.saturating_add(1);
                        if state.consecutive_health_failures < self.policy.health_failure_threshold
                        {
                            state.active = Some(active);
                            let snapshot = state.make_snapshot(
                                true,
                                Some(rcp_device),
                                Some(format!(
                                    "OpenThread control endpoints are temporarily degraded: {error}"
                                )),
                                ThreadRuntimePhase::Degraded,
                            );
                            state.snapshot = snapshot.clone();
                            return snapshot;
                        }
                        failure_reason = Some(format!(
                            "OpenThread control endpoints failed {} consecutive health checks: {error}",
                            state.consecutive_health_failures
                        ));
                    }
                }
            }
            self.mark_network_changed();
            if let Err(error) = active.router.shutdown() {
                warn!(%error, "Failed to stop an unhealthy OpenThread border router");
                let mut state = self.lock_state();
                state.active = Some(active);
                let snapshot = state.make_snapshot(
                    false,
                    Some(rcp_device),
                    Some(format!(
                        "Unable to stop the unhealthy OpenThread border router: {error}"
                    )),
                    ThreadRuntimePhase::Degraded,
                );
                state.snapshot = snapshot.clone();
                return snapshot;
            }
            let mut state = self.lock_state();
            state.last_exit = failure_reason;
            state.active = None;
        }

        let rcp_device = match config
            .rcp_device
            .clone()
            .map_or_else(|| self.backend.discover_rcp(), |device| Ok(Some(device)))
        {
            Ok(Some(device)) => device,
            Ok(None) => {
                let candidates = self.backend.serial_candidates().unwrap_or_default();
                let message = if candidates.len() > 1 {
                    format!(
                        "Multiple Thread RCP serial devices were detected: {}. Select one explicitly.",
                        candidates
                            .iter()
                            .map(|candidate| format!(
                                "{} ({})",
                                candidate.path.display(),
                                candidate.confidence.as_str()
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                } else {
                    "No Thread RCP serial device was detected. Connect an RCP or select its serial port explicitly."
                        .to_string()
                };
                return self.store_unavailable(None, message, ThreadRuntimePhase::WaitingForRcp);
            }
            Err(error) => {
                return self
                    .schedule_retry(None, format!("Unable to discover a Thread RCP: {error}"));
            }
        };
        let agent_path = match self.backend.discover_agent(config.agent_path.as_deref()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                return self.schedule_retry(
                    Some(rcp_device),
                    "The OpenThread border-router executable is unavailable".to_string(),
                );
            }
            Err(error) => {
                return self.schedule_retry(Some(rcp_device), error.to_string());
            }
        };
        let config = BorderRouterConfig {
            agent_path,
            rcp_device: rcp_device.clone(),
            baud_rate: config.baud_rate,
            thread_interface: config.thread_interface,
            infrastructure_interface: config.infrastructure_interface,
            data_path: config.data_path,
        };
        {
            let mut state = self.lock_state();
            let snapshot = state.make_snapshot(
                false,
                Some(rcp_device.clone()),
                Some("Starting the OpenThread border router".to_string()),
                ThreadRuntimePhase::Starting,
            );
            state.snapshot = snapshot;
        }
        match self.backend.start_border_router(config) {
            Ok((router, controller)) => {
                let active = ActiveThreadRuntime { router, controller };
                self.network_generation.fetch_add(1, Ordering::Relaxed);
                self.invalidate_scan();
                let mut state = self.lock_state();
                if state.ever_started {
                    state.restart_count = state.restart_count.saturating_add(1);
                } else {
                    state.ever_started = true;
                }
                state.active = Some(active);
                state.consecutive_health_failures = 0;
                state.startup_failures = 0;
                state.retry_not_before = None;
                state.next_retry_at = None;
                let snapshot =
                    state.make_snapshot(true, Some(rcp_device), None, ThreadRuntimePhase::Ready);
                state.snapshot = snapshot.clone();
                snapshot
            }
            Err(error) => self.schedule_retry(
                Some(rcp_device),
                format!("Unable to start the OpenThread border router: {error}"),
            ),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> ThreadRuntimeSnapshot {
        self.lock_state().snapshot.clone()
    }

    pub fn available_rcp_devices(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .backend
            .serial_candidates()?
            .into_iter()
            .map(|candidate| candidate.path)
            .collect())
    }

    pub fn available_rcp_candidates(&self) -> Result<Vec<ThreadRcpCandidate>> {
        self.backend.serial_candidates()
    }

    #[must_use]
    pub fn configured_rcp_device(&self) -> Option<PathBuf> {
        self.runtime_config().rcp_device
    }

    /// Selects a detected RCP, or restores automatic selection when omitted.
    /// The choice is host-local and survives subsequent Edge starts.
    pub fn set_rcp_device(&self, requested: Option<PathBuf>) -> Result<ThreadRuntimeSnapshot> {
        let selected = match requested {
            Some(requested) => {
                let candidates = self.backend.serial_candidates()?;
                let selected = candidates
                    .into_iter()
                    .find(|candidate| paths_refer_to_same_device(&candidate.path, &requested))
                    .map(|candidate| candidate.path)
                    .ok_or_else(|| {
                        OpenThreadError::InvalidConfiguration(format!(
                            "Thread RCP is not one of the currently detected serial devices: {}",
                            requested.display()
                        ))
                    })?;
                Some(selected)
            }
            None => None,
        };

        let data_path = self.runtime_config().data_path;
        persist_runtime_settings(
            &data_path,
            &PersistedRuntimeSettings {
                rcp_device: selected.clone(),
            },
        )?;
        self.config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .rcp_device = selected;
        {
            let mut state = self.lock_state();
            state.retry_not_before = None;
            state.next_retry_at = None;
        }
        Ok(self.refresh())
    }

    pub fn status(&self) -> Result<ThreadStatus> {
        let _lifecycle = self.read_lifecycle();
        let controller = self.controller()?;
        self.status_from_controller(controller.as_ref())
    }

    fn status_from_controller(&self, controller: &dyn ThreadControl) -> Result<ThreadStatus> {
        let status = controller.status()?;
        #[cfg(target_os = "macos")]
        let status = {
            let mut status = status;
            if status.addresses.is_empty()
                && let Some(prefix) = status.mesh_local_prefix.as_deref()
                && let Ok(address) = crate::discovery::macos_mesh_local_ipv6_address(prefix)
            {
                status.addresses.push(address.to_string());
            }
            status
        };
        Ok(status)
    }

    pub fn create_network(&self, network: &CreateNetwork) -> Result<ThreadStatus> {
        let _lifecycle = self.read_lifecycle();
        let controller = self.controller()?;
        self.invalidate_scan();
        let result = controller.create_network(network);
        self.mark_network_changed();
        result?;
        self.status_from_controller(controller.as_ref())
    }

    pub fn active_dataset(&self) -> Result<ThreadActiveDataset> {
        let _lifecycle = self.read_lifecycle();
        self.controller()?.active_dataset()
    }

    pub fn import_active_dataset(&self, dataset_tlvs: &str) -> Result<ThreadStatus> {
        let _lifecycle = self.read_lifecycle();
        let controller = self.controller()?;
        let dataset = parse_imported_dataset(dataset_tlvs)?;
        self.invalidate_scan();
        let result = controller.import_active_dataset(dataset);
        self.mark_network_changed();
        result?;
        self.status_from_controller(controller.as_ref())
    }

    pub fn ensure_default_development_network(&self) -> Result<bool> {
        let _lifecycle = self.read_lifecycle();
        let seeded = match self.controller()?.ensure_default_development_network() {
            Ok(seeded) => seeded,
            Err(error) => {
                self.mark_network_changed();
                return Err(error);
            }
        };
        if seeded {
            self.mark_network_changed();
        }
        Ok(seeded)
    }

    #[must_use]
    pub fn scan_snapshot(&self) -> ThreadScanSnapshot {
        let state = self
            .scan_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::scan_snapshot_from_state(&state)
    }

    pub fn refresh_scan(&self, max_age: Duration, force: bool) -> Result<ThreadScanSnapshot> {
        let _lifecycle = self.read_lifecycle();
        let controller = self.controller()?;
        self.refresh_scan_with(max_age, force, move || controller.scan_all())
    }

    fn refresh_scan_with<F>(
        &self,
        max_age: Duration,
        force: bool,
        scan: F,
    ) -> Result<ThreadScanSnapshot>
    where
        F: FnOnce() -> Result<ThreadScan>,
    {
        let requested_at = Instant::now();
        let scan_generation;
        let mut state = self
            .scan_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if state
                .last_attempt_at
                .is_some_and(|completed| completed >= requested_at)
            {
                return Self::scan_result_from_state(&state);
            }
            let fresh = state
                .completed_at_instant
                .is_some_and(|completed| completed.elapsed() < max_age);
            if !force && fresh {
                return Ok(Self::scan_snapshot_from_state(&state));
            }
            if !state.scanning {
                state.scanning = true;
                state.error = None;
                scan_generation = state.generation;
                break;
            }
            state = self
                .scan_changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(state);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(scan));
        let now = Instant::now();
        let mut state = self
            .scan_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.scanning = false;
        state.last_attempt_at = Some(now);
        match result {
            Ok(result) if state.generation == scan_generation => match result {
                Ok(scan) => {
                    let fresh_sources = scan
                        .sources
                        .iter()
                        .filter(|source| source.state == ThreadObservationState::Fresh)
                        .count();
                    let merged = merge_scan_sources(scan, state.scan.as_ref());
                    if fresh_sources == 0 {
                        let error = all_scan_sources_failed(&merged);
                        if state.scan.is_some() {
                            state.scan = Some(merged);
                        }
                        state.error = Some(error);
                    } else {
                        state.scan = Some(merged);
                        state.completed_at = Some(SystemTime::now());
                        state.completed_at_instant = Some(now);
                        state.error = None;
                    }
                }
                Err(error) => {
                    let error = error.to_string();
                    if let Some(scan) = state.scan.as_mut() {
                        mark_scan_stale(scan, &error);
                    }
                    state.error = Some(error);
                }
            },
            Ok(_) => state.error = Some(OpenThreadError::NetworkChanged.to_string()),
            Err(payload) => {
                state.error = Some("The OpenThread scan task panicked".to_string());
                self.scan_changed.notify_all();
                drop(state);
                std::panic::resume_unwind(payload);
            }
        }
        self.scan_changed.notify_all();
        Self::scan_result_from_state(&state)
    }

    fn scan_result_from_state(state: &ThreadScanState) -> Result<ThreadScanSnapshot> {
        if state.scan.is_none()
            && let Some(error) = &state.error
        {
            return Err(OpenThreadError::Control(error.clone()));
        }
        Ok(Self::scan_snapshot_from_state(state))
    }

    fn scan_snapshot_from_state(state: &ThreadScanState) -> ThreadScanSnapshot {
        ThreadScanSnapshot {
            scan: state.scan.clone(),
            scanning: state.scanning,
            completed_at: state.completed_at,
            error: state.error.clone(),
        }
    }

    fn invalidate_scan(&self) {
        let mut state = self
            .scan_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.scan = None;
        state.generation = state.generation.wrapping_add(1);
        state.completed_at = None;
        state.completed_at_instant = None;
        state.error = None;
    }

    fn mark_network_changed(&self) {
        self.network_generation.fetch_add(1, Ordering::Relaxed);
        self.invalidate_scan();
    }

    #[must_use]
    pub fn network_generation(&self) -> u64 {
        self.network_generation.load(Ordering::Relaxed)
    }

    pub fn thread_ipv6_address(&self) -> Result<Ipv6Addr> {
        let _lifecycle = self.read_lifecycle();
        let (thread_interface, controller) = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let active = state.active.as_ref().ok_or_else(|| {
                OpenThreadError::Unavailable("no active OTBR process".to_string())
            })?;
            (
                active.router.thread_interface().to_string(),
                active.controller.clone(),
            )
        };
        let status = controller.status()?;
        let address = self
            .backend
            .thread_ipv6_address(&thread_interface, status.mesh_local_prefix.as_deref())?;
        info!(
            configured_interface = %thread_interface,
            mesh_local_prefix = ?status.mesh_local_prefix,
            address = %address,
            "Selected a mesh-reachable IPv6 address for Thread DNS-SD"
        );
        Ok(address)
    }

    pub fn shutdown(&self) -> Result<()> {
        let _lifecycle = self.write_lifecycle();
        let active = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .take();
        let (remaining, result) = active.map_or((None, Ok(())), |mut active| {
            match active.router.shutdown() {
                Ok(()) => (None, Ok(())),
                Err(error) => {
                    warn!(%error, "OpenThread border-router shutdown failed");
                    (Some(active), Err(error))
                }
            }
        });
        let (rcp_device, message, phase) = match remaining.as_ref() {
            Some(active) => (
                Some(active.router.rcp_device().to_path_buf()),
                format!(
                    "OpenThread runtime shutdown is incomplete: {}",
                    result
                        .as_ref()
                        .expect_err("a retained router implies a shutdown error")
                ),
                ThreadRuntimePhase::Degraded,
            ),
            None => (
                None,
                "Thread runtime is shut down".to_string(),
                ThreadRuntimePhase::Stopped,
            ),
        };
        let mut state = self.lock_state();
        state.active = remaining;
        state.retry_not_before = None;
        state.next_retry_at = None;
        let snapshot = state.make_snapshot(false, rcp_device, Some(message), phase);
        state.snapshot = snapshot;
        drop(state);
        self.mark_network_changed();
        result
    }

    fn controller(&self) -> Result<Arc<dyn ThreadControl>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .as_ref()
            .map(|active| active.controller.clone())
            .ok_or_else(|| OpenThreadError::Unavailable("no active OTBR controller".to_string()))
    }

    fn read_lifecycle(&self) -> std::sync::RwLockReadGuard<'_, ()> {
        self.lifecycle
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn runtime_config(&self) -> ThreadRuntimeConfig {
        self.config
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn write_lifecycle(&self) -> std::sync::RwLockWriteGuard<'_, ()> {
        self.lifecycle
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ThreadRuntimeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn store_unavailable(
        &self,
        rcp_device: Option<PathBuf>,
        message: String,
        phase: ThreadRuntimePhase,
    ) -> ThreadRuntimeSnapshot {
        let mut state = self.lock_state();
        state.active = None;
        state.consecutive_health_failures = 0;
        state.startup_failures = 0;
        state.retry_not_before = None;
        state.next_retry_at = None;
        let snapshot = state.make_snapshot(false, rcp_device, Some(message), phase);
        state.snapshot = snapshot.clone();
        snapshot
    }

    fn schedule_retry(
        &self,
        rcp_device: Option<PathBuf>,
        message: String,
    ) -> ThreadRuntimeSnapshot {
        let mut state = self.lock_state();
        state.active = None;
        state.startup_failures = state.startup_failures.saturating_add(1);
        let delay = self.restart_delay(state.startup_failures);
        state.retry_not_before = Some(Instant::now() + delay);
        state.next_retry_at = SystemTime::now().checked_add(delay);
        let snapshot = state.make_snapshot(
            false,
            rcp_device,
            Some(format!(
                "{message}; retrying in {} seconds",
                delay.as_secs_f32()
            )),
            ThreadRuntimePhase::Backoff,
        );
        state.snapshot = snapshot.clone();
        snapshot
    }

    fn restart_delay(&self, failures: u32) -> Duration {
        let exponent = failures.saturating_sub(1).min(16);
        let factor = 1_u32 << exponent;
        let base = self
            .policy
            .initial_backoff
            .checked_mul(factor)
            .unwrap_or(self.policy.max_backoff)
            .min(self.policy.max_backoff);
        if !self.policy.jitter || base.is_zero() {
            return base;
        }
        let mut random = [0_u8; 2];
        if getrandom::fill(&mut random).is_err() {
            return base;
        }
        let fraction = f64::from(u16::from_ne_bytes(random)) / f64::from(u16::MAX);
        base.saturating_add(base.mul_f64(0.2 * fraction))
            .min(self.policy.max_backoff)
    }
}

impl ThreadRuntimeState {
    fn make_snapshot(
        &self,
        available: bool,
        rcp_device: Option<PathBuf>,
        message: Option<String>,
        phase: ThreadRuntimePhase,
    ) -> ThreadRuntimeSnapshot {
        ThreadRuntimeSnapshot {
            available,
            rcp_device,
            message,
            phase,
            consecutive_failures: self.consecutive_health_failures,
            restart_count: self.restart_count,
            next_retry_at: self.next_retry_at,
            last_exit: self.last_exit.clone(),
        }
    }
}

fn merge_scan_sources(mut current: ThreadScan, previous: Option<&ThreadScan>) -> ThreadScan {
    let Some(previous) = previous else {
        return current;
    };
    let previous_channels = previous
        .channels
        .iter()
        .map(|channel| (channel.channel, channel))
        .collect::<BTreeMap<_, _>>();
    for index in 0..current.sources.len() {
        if current.sources[index].state != ThreadObservationState::Unavailable {
            continue;
        }
        let source = current.sources[index].source;
        let Some(previous_status) = previous
            .sources
            .iter()
            .find(|status| status.source == source && status.observed_at.is_some())
        else {
            continue;
        };
        current.sources[index].state = ThreadObservationState::Stale;
        current.sources[index].observed_at = previous_status.observed_at;
        match source {
            ThreadScanSource::NearbyNetworks => {
                current.networks.clone_from(&previous.networks);
                for channel in &mut current.channels {
                    if let Some(previous) = previous_channels.get(&channel.channel) {
                        channel.network_count = previous.network_count;
                        channel.strongest_network_rssi = previous.strongest_network_rssi;
                    }
                }
            }
            ThreadScanSource::ChannelEnergy => {
                for channel in &mut current.channels {
                    if let Some(previous) = previous_channels.get(&channel.channel) {
                        channel.max_rssi = previous.max_rssi;
                    }
                }
            }
            ThreadScanSource::ChannelUtilization => {
                for channel in &mut current.channels {
                    if let Some(previous) = previous_channels.get(&channel.channel) {
                        channel.occupancy = previous.occupancy;
                    }
                }
            }
            ThreadScanSource::RadioStatistics => {
                current.statistics.clone_from(&previous.statistics);
            }
            ThreadScanSource::MeshDevices => {
                current.devices.clone_from(&previous.devices);
            }
        }
    }
    current
}

fn mark_scan_stale(scan: &mut ThreadScan, error: &str) {
    for source in &mut scan.sources {
        if source.observed_at.is_some() {
            source.state = ThreadObservationState::Stale;
            if source.error.is_none() {
                source.error = Some(error.to_string());
            }
        }
    }
}

fn all_scan_sources_failed(scan: &ThreadScan) -> String {
    let failures = scan
        .sources
        .iter()
        .filter_map(|source| source.error.as_deref())
        .collect::<Vec<_>>();
    if failures.is_empty() {
        "all OpenThread diagnostics sources were unavailable".to_string()
    } else {
        format!(
            "all OpenThread diagnostics sources were unavailable: {}",
            failures.join("; ")
        )
    }
}

fn runtime_settings_path(data_path: &Path) -> PathBuf {
    data_path.join(RUNTIME_SETTINGS_FILE)
}

fn load_runtime_settings(data_path: &Path) -> Result<PersistedRuntimeSettings> {
    let path = runtime_settings_path(data_path);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PersistedRuntimeSettings::default());
        }
        Err(source) => {
            return Err(OpenThreadError::FileSystem {
                operation: "read OpenThread runtime settings",
                path,
                source,
            });
        }
    };
    serde_json::from_slice(&bytes).map_err(|error| {
        OpenThreadError::InvalidConfiguration(format!(
            "unable to parse {}: {error}",
            path.display()
        ))
    })
}

fn persist_runtime_settings(data_path: &Path, settings: &PersistedRuntimeSettings) -> Result<()> {
    fs::create_dir_all(data_path).map_err(|source| OpenThreadError::FileSystem {
        operation: "create OpenThread settings directory",
        path: data_path.to_path_buf(),
        source,
    })?;
    let path = runtime_settings_path(data_path);
    let mut temporary = tempfile::NamedTempFile::new_in(data_path).map_err(|source| {
        OpenThreadError::FileSystem {
            operation: "create temporary OpenThread runtime settings",
            path: path.clone(),
            source,
        }
    })?;
    serde_json::to_writer_pretty(&mut temporary, settings).map_err(|error| {
        OpenThreadError::InvalidConfiguration(format!(
            "unable to serialize OpenThread runtime settings: {error}"
        ))
    })?;
    temporary
        .write_all(b"\n")
        .and_then(|()| temporary.flush())
        .map_err(|source| OpenThreadError::FileSystem {
            operation: "write OpenThread runtime settings",
            path: path.clone(),
            source,
        })?;
    temporary
        .persist(&path)
        .map_err(|error| OpenThreadError::FileSystem {
            operation: "replace OpenThread runtime settings",
            path,
            source: error.error,
        })?;
    Ok(())
}

fn paths_refer_to_same_device(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

impl Drop for ThreadRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            warn!(%error, "Failed to shut down the OpenThread runtime during drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ThreadNetwork, ThreadRadioStatistics, ThreadRole, ThreadScanSourceStatus};
    use std::{
        collections::VecDeque,
        sync::{atomic::AtomicUsize, mpsc},
        thread,
    };
    use zeroize::Zeroizing;

    struct FakeController {
        health_failures: AtomicUsize,
    }

    impl FakeController {
        fn fail_next_health_checks(&self, count: usize) {
            self.health_failures.store(count, Ordering::SeqCst);
        }
    }

    impl ThreadControl for FakeController {
        fn health_check(&self) -> Result<()> {
            if self
                .health_failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                Err(OpenThreadError::Control(
                    "simulated transient health failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }

        fn status(&self) -> Result<ThreadStatus> {
            Ok(ThreadStatus {
                role: ThreadRole::Disabled,
                network_name: None,
                channel: None,
                pan_id: None,
                extended_pan_id: None,
                mesh_local_prefix: None,
                addresses: Vec::new(),
            })
        }

        fn scan_all(&self) -> Result<ThreadScan> {
            Ok(test_scan("fake"))
        }

        fn active_dataset(&self) -> Result<ThreadActiveDataset> {
            crate::dataset::parse_dataset_hex(crate::types::DEFAULT_DEVELOPMENT_DATASET_TLVS)
        }

        fn create_network(&self, _network: &CreateNetwork) -> Result<()> {
            Ok(())
        }

        fn import_active_dataset(&self, _dataset: Zeroizing<Vec<u8>>) -> Result<()> {
            Ok(())
        }

        fn ensure_default_development_network(&self) -> Result<bool> {
            Ok(false)
        }
    }

    struct FakeRouter {
        rcp_device: PathBuf,
        exit: Arc<Mutex<Option<String>>>,
        shutdowns: Arc<AtomicUsize>,
        shutdown_failures: Arc<AtomicUsize>,
    }

    impl ManagedBorderRouter for FakeRouter {
        fn rcp_device(&self) -> &Path {
            &self.rcp_device
        }

        fn thread_interface(&self) -> &str {
            "wpan0"
        }

        fn poll_exit(&mut self) -> Result<Option<String>> {
            Ok(self
                .exit
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone())
        }

        fn shutdown(&mut self) -> Result<()> {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
            if self
                .shutdown_failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(OpenThreadError::Process(
                    "simulated cleanup failure".to_string(),
                ));
            }
            Ok(())
        }

        fn diagnostic_tail(&self) -> String {
            "simulated diagnostics".to_string()
        }
    }

    struct FakeBackend {
        controller: Arc<FakeController>,
        starts: AtomicUsize,
        start_failures: AtomicUsize,
        exit: Arc<Mutex<Option<String>>>,
        shutdowns: Arc<AtomicUsize>,
        shutdown_failures: Arc<AtomicUsize>,
        candidates: Mutex<VecDeque<ThreadRcpCandidate>>,
    }

    impl FakeBackend {
        fn new() -> Self {
            Self {
                controller: Arc::new(FakeController {
                    health_failures: AtomicUsize::new(0),
                }),
                starts: AtomicUsize::new(0),
                start_failures: AtomicUsize::new(0),
                exit: Arc::new(Mutex::new(None)),
                shutdowns: Arc::new(AtomicUsize::new(0)),
                shutdown_failures: Arc::new(AtomicUsize::new(0)),
                candidates: Mutex::new(VecDeque::new()),
            }
        }
    }

    impl RuntimeBackend for FakeBackend {
        fn discover_rcp(&self) -> Result<Option<PathBuf>> {
            Ok(Some(PathBuf::from("/dev/null")))
        }

        fn serial_candidates(&self) -> Result<Vec<ThreadRcpCandidate>> {
            Ok(self
                .candidates
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .cloned()
                .collect())
        }

        fn discover_agent(&self, _explicit: Option<&Path>) -> Result<Option<PathBuf>> {
            Ok(Some(PathBuf::from("/bin/true")))
        }

        fn start_border_router(
            &self,
            config: BorderRouterConfig,
        ) -> Result<(Box<dyn ManagedBorderRouter>, Arc<dyn ThreadControl>)> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            if self
                .start_failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(OpenThreadError::Process(
                    "simulated startup failure".to_string(),
                ));
            }
            *self
                .exit
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            Ok((
                Box::new(FakeRouter {
                    rcp_device: config.rcp_device,
                    exit: self.exit.clone(),
                    shutdowns: self.shutdowns.clone(),
                    shutdown_failures: self.shutdown_failures.clone(),
                }),
                self.controller.clone(),
            ))
        }

        fn thread_ipv6_address(
            &self,
            _interface: &str,
            _mesh_local_prefix: Option<&str>,
        ) -> Result<Ipv6Addr> {
            Ok("fd00::1".parse().unwrap())
        }
    }

    fn fake_runtime() -> (ThreadRuntime, Arc<FakeBackend>) {
        let backend = Arc::new(FakeBackend::new());
        let runtime = ThreadRuntime::new_with_backend(
            ThreadRuntimeConfig {
                rcp_device: Some("/dev/null".into()),
                agent_path: Some("/bin/true".into()),
                baud_rate: 460_800,
                thread_interface: "wpan0".to_string(),
                infrastructure_interface: "en0".to_string(),
                data_path: "/tmp/extrittio-thread-runtime-supervisor-test".into(),
            },
            backend.clone(),
            SupervisorPolicy {
                health_failure_threshold: 3,
                initial_backoff: Duration::from_secs(60),
                max_backoff: Duration::from_secs(60),
                jitter: false,
            },
        );
        (runtime, backend)
    }

    fn test_runtime() -> ThreadRuntime {
        ThreadRuntime::new(ThreadRuntimeConfig {
            rcp_device: None,
            agent_path: None,
            baud_rate: 460_800,
            thread_interface: "wpan0".to_string(),
            infrastructure_interface: "en0".to_string(),
            data_path: "/tmp/extrittio-thread-runtime-test".into(),
        })
    }

    fn test_scan(network_name: &str) -> ThreadScan {
        ThreadScan {
            channels: Vec::new(),
            networks: vec![ThreadNetwork {
                network_name: Some(network_name.to_string()),
                pan_id: "1234".to_string(),
                extended_address: "0011223344556677".to_string(),
                extended_pan_id: Some("8899aabbccddeeff".to_string()),
                channel: 15,
                rssi: -40,
                lqi: 3,
            }],
            devices: Vec::new(),
            statistics: ThreadRadioStatistics::default(),
            sources: vec![ThreadScanSourceStatus {
                source: ThreadScanSource::NearbyNetworks,
                state: ThreadObservationState::Fresh,
                observed_at: Some(SystemTime::now()),
                error: None,
            }],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn transient_health_failures_degrade_without_restarting() {
        let (runtime, backend) = fake_runtime();
        assert_eq!(runtime.refresh().phase, ThreadRuntimePhase::Ready);
        backend.controller.fail_next_health_checks(2);

        let first = runtime.refresh();
        let second = runtime.refresh();
        assert_eq!(first.phase, ThreadRuntimePhase::Degraded);
        assert_eq!(first.consecutive_failures, 1);
        assert_eq!(second.phase, ThreadRuntimePhase::Degraded);
        assert_eq!(second.consecutive_failures, 2);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);

        let recovered = runtime.refresh();
        assert_eq!(recovered.phase, ThreadRuntimePhase::Ready);
        assert_eq!(recovered.consecutive_failures, 0);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn selected_rcp_is_persisted_and_restarts_the_managed_agent() {
        let directory = tempfile::tempdir().unwrap();
        let backend = Arc::new(FakeBackend::new());
        backend
            .candidates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(ThreadRcpCandidate {
                path: "/dev/zero".into(),
                confidence: crate::types::ThreadRcpConfidence::Verified,
                match_reason: "test radio".to_string(),
                usb_vendor_id: None,
                usb_product_id: None,
                manufacturer: None,
                product: None,
                serial_number: None,
            });
        let runtime = ThreadRuntime::new_with_backend(
            ThreadRuntimeConfig {
                rcp_device: Some("/dev/null".into()),
                agent_path: Some("/bin/true".into()),
                baud_rate: 460_800,
                thread_interface: "wpan0".to_string(),
                infrastructure_interface: "en0".to_string(),
                data_path: directory.path().to_path_buf(),
            },
            backend.clone(),
            SupervisorPolicy {
                health_failure_threshold: 3,
                initial_backoff: Duration::from_secs(60),
                max_backoff: Duration::from_secs(60),
                jitter: false,
            },
        );

        assert_eq!(runtime.refresh().rcp_device, Some("/dev/null".into()));
        let selected = runtime.set_rcp_device(Some("/dev/zero".into())).unwrap();

        assert_eq!(selected.rcp_device, Some("/dev/zero".into()));
        assert_eq!(backend.starts.load(Ordering::SeqCst), 2);
        assert_eq!(backend.shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(
            load_runtime_settings(directory.path()).unwrap().rcp_device,
            Some("/dev/zero".into())
        );
    }

    #[test]
    fn persisted_rcp_is_loaded_only_without_a_cli_override() {
        let directory = tempfile::tempdir().unwrap();
        persist_runtime_settings(
            directory.path(),
            &PersistedRuntimeSettings {
                rcp_device: Some("/dev/zero".into()),
            },
        )
        .unwrap();
        let config = |rcp_device| ThreadRuntimeConfig {
            rcp_device,
            agent_path: None,
            baud_rate: 460_800,
            thread_interface: "wpan0".to_string(),
            infrastructure_interface: "en0".to_string(),
            data_path: directory.path().to_path_buf(),
        };

        assert_eq!(
            ThreadRuntime::new(config(None)).configured_rcp_device(),
            Some("/dev/zero".into())
        );
        assert_eq!(
            ThreadRuntime::new(config(Some("/dev/null".into()))).configured_rcp_device(),
            Some("/dev/null".into())
        );
    }

    #[test]
    fn repeated_health_failures_restart_once_threshold_is_reached() {
        let (runtime, backend) = fake_runtime();
        let _ = runtime.refresh();
        backend.controller.fail_next_health_checks(3);

        let _ = runtime.refresh();
        let _ = runtime.refresh();
        let restarted = runtime.refresh();

        assert_eq!(restarted.phase, ThreadRuntimePhase::Ready);
        assert_eq!(restarted.restart_count, 1);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 2);
        assert_eq!(backend.shutdowns.load(Ordering::SeqCst), 1);
        assert!(
            restarted
                .last_exit
                .as_deref()
                .is_some_and(|error| { error.contains("failed 3 consecutive health checks") })
        );
    }

    #[test]
    fn startup_failure_enters_backoff_without_process_churn() {
        let (runtime, backend) = fake_runtime();
        backend.start_failures.store(1, Ordering::SeqCst);

        let failed = runtime.refresh();
        let suppressed = runtime.refresh();

        assert_eq!(failed.phase, ThreadRuntimePhase::Backoff);
        assert!(failed.next_retry_at.is_some());
        assert_eq!(suppressed, failed);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn definitive_process_exit_restarts_immediately_and_keeps_diagnostics() {
        let (runtime, backend) = fake_runtime();
        let _ = runtime.refresh();
        *backend
            .exit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some("exit code 70".to_string());

        let restarted = runtime.refresh();

        assert_eq!(restarted.phase, ThreadRuntimePhase::Ready);
        assert_eq!(restarted.restart_count, 1);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 2);
        assert!(
            restarted
                .last_exit
                .as_deref()
                .is_some_and(|exit| exit.contains("exit code 70"))
        );
    }

    #[test]
    fn cleanup_failure_retains_the_managed_process_and_avoids_double_start() {
        let (runtime, backend) = fake_runtime();
        let _ = runtime.refresh();
        backend.controller.fail_next_health_checks(3);
        backend.shutdown_failures.store(1, Ordering::SeqCst);

        let _ = runtime.refresh();
        let _ = runtime.refresh();
        let failed_cleanup = runtime.refresh();

        assert_eq!(failed_cleanup.phase, ThreadRuntimePhase::Degraded);
        assert!(
            failed_cleanup
                .message
                .as_deref()
                .is_some_and(|message| message.contains("simulated cleanup failure"))
        );
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        assert_eq!(backend.shutdowns.load(Ordering::SeqCst), 1);

        let recovered = runtime.refresh();
        assert_eq!(recovered.phase, ThreadRuntimePhase::Ready);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn explicit_shutdown_reports_incomplete_cleanup_without_claiming_stopped() {
        let (runtime, backend) = fake_runtime();
        let _ = runtime.refresh();
        backend.shutdown_failures.store(1, Ordering::SeqCst);

        let error = runtime.shutdown().unwrap_err();
        let snapshot = runtime.snapshot();

        assert!(error.to_string().contains("simulated cleanup failure"));
        assert_eq!(snapshot.phase, ThreadRuntimePhase::Degraded);
        assert!(snapshot.rcp_device.is_some());
        assert!(
            snapshot
                .message
                .as_deref()
                .is_some_and(|message| message.contains("shutdown is incomplete"))
        );
    }

    #[test]
    fn shared_scan_reuses_fresh_observation() {
        let runtime = test_runtime();
        let calls = AtomicU64::new(0);
        runtime
            .refresh_scan_with(Duration::from_secs(60), false, || {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(test_scan("first"))
            })
            .unwrap();
        let snapshot = runtime
            .refresh_scan_with(Duration::from_secs(60), false, || {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(test_scan("duplicate"))
            })
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            snapshot.scan.unwrap().networks[0].network_name.as_deref(),
            Some("first")
        );
    }

    #[test]
    fn concurrent_forced_scans_join_same_radio_work() {
        let runtime = Arc::new(test_runtime());
        let calls = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first_runtime = runtime.clone();
        let first_calls = calls.clone();
        let first = thread::spawn(move || {
            first_runtime.refresh_scan_with(Duration::ZERO, true, || {
                first_calls.fetch_add(1, Ordering::Relaxed);
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(test_scan("shared"))
            })
        });
        started_rx.recv().unwrap();
        let second_runtime = runtime.clone();
        let second_calls = calls.clone();
        let second = thread::spawn(move || {
            second_runtime.refresh_scan_with(Duration::ZERO, true, || {
                second_calls.fetch_add(1, Ordering::Relaxed);
                Ok(test_scan("duplicate"))
            })
        });
        thread::sleep(Duration::from_millis(25));
        release_tx.send(()).unwrap();
        let first_snapshot = first.join().unwrap().unwrap();
        let second_snapshot = second.join().unwrap().unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(first_snapshot.scan, second_snapshot.scan);
    }

    #[test]
    fn failed_refresh_keeps_last_success() {
        let runtime = test_runtime();
        runtime
            .refresh_scan_with(Duration::ZERO, true, || Ok(test_scan("retained")))
            .unwrap();
        let snapshot = runtime
            .refresh_scan_with(Duration::ZERO, true, || {
                Err(OpenThreadError::Control("radio unavailable".to_string()))
            })
            .unwrap();
        assert_eq!(
            snapshot.scan.unwrap().networks[0].network_name.as_deref(),
            Some("retained")
        );
        assert_eq!(
            snapshot.error.as_deref(),
            Some("OpenThread control request failed: radio unavailable")
        );
    }

    #[test]
    fn unavailable_source_keeps_last_observation_and_marks_it_stale() {
        let runtime = test_runtime();
        runtime
            .refresh_scan_with(Duration::ZERO, true, || Ok(test_scan("retained")))
            .unwrap();
        let snapshot = runtime
            .refresh_scan_with(Duration::ZERO, true, || {
                let mut attempt = test_scan("incorrect-empty-replacement");
                attempt.networks.clear();
                attempt.sources[0].state = ThreadObservationState::Unavailable;
                attempt.sources[0].observed_at = None;
                attempt.sources[0].error = Some("D-Bus timed out".to_string());
                Ok(attempt)
            })
            .unwrap();

        let scan = snapshot.scan.unwrap();
        assert_eq!(scan.networks[0].network_name.as_deref(), Some("retained"));
        assert_eq!(scan.sources[0].state, ThreadObservationState::Stale);
        assert!(scan.sources[0].observed_at.is_some());
        assert!(snapshot.error.as_deref().is_some_and(|error| {
            error.contains("all OpenThread diagnostics sources were unavailable")
        }));
    }

    #[test]
    fn network_change_discards_running_scan() {
        let runtime = Arc::new(test_runtime());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let scan_runtime = runtime.clone();
        let scan = thread::spawn(move || {
            scan_runtime.refresh_scan_with(Duration::ZERO, true, || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(test_scan("stale"))
            })
        });
        started_rx.recv().unwrap();
        runtime.mark_network_changed();
        release_tx.send(()).unwrap();
        let error = scan.join().unwrap().unwrap_err();
        assert!(error.to_string().contains("network changed"));
        assert!(runtime.scan_snapshot().scan.is_none());
    }

    #[test]
    fn scan_panic_does_not_wedge_cache() {
        let runtime = test_runtime();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = runtime.refresh_scan_with(Duration::ZERO, true, || -> Result<ThreadScan> {
                panic!("simulated scanner panic")
            });
        }));
        assert!(panic.is_err());
        assert!(!runtime.scan_snapshot().scanning);
        assert!(
            runtime
                .refresh_scan_with(Duration::ZERO, true, || Ok(test_scan("recovered")))
                .is_ok()
        );
    }
}
