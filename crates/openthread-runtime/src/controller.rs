use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::{Duration, Instant, SystemTime},
};
use tracing::{debug, info};
use zeroize::Zeroizing;

use crate::{
    dataset::{parse_active_dataset_bytes, parse_imported_dataset},
    dbus::OtbrDbus,
    discovery::thread_interface_ipv6_addresses,
    error::{OpenThreadError, Result},
    rest::OtbrRest,
    types::{
        CreateNetwork, DEFAULT_DEVELOPMENT_DATASET_TLVS, ThreadActiveDataset,
        ThreadChannelDiagnostics, ThreadMeshDevice, ThreadNetwork, ThreadObservationState,
        ThreadRadioStatistics, ThreadRole, ThreadScan, ThreadScanSource, ThreadScanSourceStatus,
        ThreadStatus,
    },
};

const DIAGNOSTICS_DEADLINE: Duration = Duration::from_secs(60);

pub(crate) struct ThreadController {
    dbus: Arc<dyn OtbrDbusControl>,
    rest: Arc<dyn OtbrRestControl>,
    thread_interface: String,
    scan_lock: Mutex<()>,
    dataset_lock: RwLock<()>,
}

trait OtbrDbusControl: Send + Sync {
    fn health_check(&self) -> Result<()>;
    fn verify_capabilities(&self) -> Result<()>;
    fn status(&self, addresses: Vec<String>) -> Result<ThreadStatus>;
    fn scan_networks(&self) -> Result<Vec<ThreadNetwork>>;
    fn energy_scan(&self, duration_ms: u32) -> Result<Vec<(u16, i16)>>;
    fn channel_qualities(&self) -> Result<Vec<(u16, u16)>>;
    fn radio_statistics(&self) -> (ThreadRadioStatistics, Vec<String>, bool);
    fn active_dataset_tlvs(&self) -> Result<Zeroizing<Vec<u8>>>;
    fn active_dataset_tlvs_if_present(&self) -> Result<Option<Zeroizing<Vec<u8>>>>;
    fn set_active_dataset_tlvs(&self, dataset: &[u8]) -> Result<()>;
    fn attach_current_dataset(&self) -> Result<()>;
    fn detach(&self) -> Result<()>;
    fn factory_reset(&self) -> Result<()>;
    fn create_network(&self, network: &CreateNetwork) -> Result<()>;
}

impl OtbrDbusControl for OtbrDbus {
    fn health_check(&self) -> Result<()> {
        Self::health_check(self)
    }

    fn verify_capabilities(&self) -> Result<()> {
        Self::verify_capabilities(self)
    }

    fn status(&self, addresses: Vec<String>) -> Result<ThreadStatus> {
        Self::status(self, addresses)
    }

    fn scan_networks(&self) -> Result<Vec<ThreadNetwork>> {
        Self::scan_networks(self)
    }

    fn energy_scan(&self, duration_ms: u32) -> Result<Vec<(u16, i16)>> {
        Self::energy_scan(self, duration_ms)
    }

    fn channel_qualities(&self) -> Result<Vec<(u16, u16)>> {
        Self::channel_qualities(self)
    }

    fn radio_statistics(&self) -> (ThreadRadioStatistics, Vec<String>, bool) {
        Self::radio_statistics(self)
    }

    fn active_dataset_tlvs(&self) -> Result<Zeroizing<Vec<u8>>> {
        Self::active_dataset_tlvs(self)
    }

    fn active_dataset_tlvs_if_present(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        Self::active_dataset_tlvs_if_present(self)
    }

    fn set_active_dataset_tlvs(&self, dataset: &[u8]) -> Result<()> {
        Self::set_active_dataset_tlvs(self, dataset)
    }

    fn attach_current_dataset(&self) -> Result<()> {
        Self::attach_current_dataset(self)
    }

    fn detach(&self) -> Result<()> {
        Self::detach(self)
    }

    fn factory_reset(&self) -> Result<()> {
        Self::factory_reset(self)
    }

    fn create_network(&self, network: &CreateNetwork) -> Result<()> {
        Self::create_network(self, network)
    }
}

trait OtbrRestControl: Send + Sync {
    fn health_check(&self) -> Result<()>;
    fn verify_authentication(&self) -> Result<()>;
    fn scan_mesh_devices(&self) -> Result<Vec<ThreadMeshDevice>>;
}

impl OtbrRestControl for OtbrRest {
    fn health_check(&self) -> Result<()> {
        Self::health_check(self)
    }

    fn verify_authentication(&self) -> Result<()> {
        Self::verify_authentication(self)
    }

    fn scan_mesh_devices(&self) -> Result<Vec<ThreadMeshDevice>> {
        Self::scan_mesh_devices(self)
    }
}

pub(crate) trait ThreadControl: Send + Sync {
    fn health_check(&self) -> Result<()>;
    fn status(&self) -> Result<ThreadStatus>;
    fn scan_all(&self) -> Result<ThreadScan>;
    fn active_dataset(&self) -> Result<ThreadActiveDataset>;
    fn create_network(&self, network: &CreateNetwork) -> Result<()>;
    fn import_active_dataset(&self, dataset: Zeroizing<Vec<u8>>) -> Result<()>;
    fn ensure_default_development_network(&self) -> Result<bool>;
}

impl ThreadControl for ThreadController {
    fn health_check(&self) -> Result<()> {
        Self::health_check(self)
    }

    fn status(&self) -> Result<ThreadStatus> {
        Self::status(self)
    }

    fn scan_all(&self) -> Result<ThreadScan> {
        Self::scan_all(self)
    }

    fn active_dataset(&self) -> Result<ThreadActiveDataset> {
        Self::active_dataset(self)
    }

    fn create_network(&self, network: &CreateNetwork) -> Result<()> {
        Self::create_network(self, network)
    }

    fn import_active_dataset(&self, dataset: Zeroizing<Vec<u8>>) -> Result<()> {
        Self::import_active_dataset(self, dataset)
    }

    fn ensure_default_development_network(&self) -> Result<bool> {
        Self::ensure_default_development_network(self)
    }
}

impl ThreadController {
    pub(crate) fn new(dbus: OtbrDbus, rest: OtbrRest, thread_interface: String) -> Self {
        Self::new_with_transports(Arc::new(dbus), Arc::new(rest), thread_interface)
    }

    fn new_with_transports(
        dbus: Arc<dyn OtbrDbusControl>,
        rest: Arc<dyn OtbrRestControl>,
        thread_interface: String,
    ) -> Self {
        Self {
            dbus,
            rest,
            thread_interface,
            scan_lock: Mutex::new(()),
            dataset_lock: RwLock::new(()),
        }
    }

    pub(crate) fn health_check(&self) -> Result<()> {
        self.dbus.health_check()?;
        self.rest.health_check()
    }

    pub(crate) fn startup_health_check(&self) -> Result<()> {
        self.health_check()?;
        self.dbus.verify_capabilities()?;
        self.rest.verify_authentication()
    }

    pub(crate) fn status(&self) -> Result<ThreadStatus> {
        let _dataset = self.read_dataset();
        self.status_unlocked()
    }

    fn status_unlocked(&self) -> Result<ThreadStatus> {
        let addresses = thread_interface_ipv6_addresses(&self.thread_interface)
            .unwrap_or_default()
            .into_iter()
            .map(|address| address.to_string())
            .collect();
        self.dbus.status(addresses)
    }

    pub(crate) fn scan_all(&self) -> Result<ThreadScan> {
        let started = Instant::now();
        let _scan = self.lock_scan();
        let _dataset = self.read_dataset();
        let mut warnings = Vec::new();
        let mut sources = Vec::new();
        let mut successful_sources = 0_u8;
        let networks =
            match before_diagnostics_deadline(started, "nearby-network discovery", || {
                self.dbus.scan_networks()
            }) {
                Ok(networks) => {
                    successful_sources += 1;
                    sources.push(fresh_source(ThreadScanSource::NearbyNetworks));
                    deduplicate_networks(networks)
                }
                Err(error) => {
                    let error = format!("Nearby-network discovery is unavailable: {error}");
                    warnings.push(error.clone());
                    sources.push(unavailable_source(ThreadScanSource::NearbyNetworks, error));
                    Vec::new()
                }
            };
        let energy = match before_diagnostics_deadline(started, "channel energy scan", || {
            self.dbus.energy_scan(100)
        }) {
            Ok(energy) => {
                successful_sources += 1;
                sources.push(fresh_source(ThreadScanSource::ChannelEnergy));
                energy
            }
            Err(error) => {
                let error = format!("Channel energy scan is unavailable: {error}");
                warnings.push(error.clone());
                sources.push(unavailable_source(ThreadScanSource::ChannelEnergy, error));
                Vec::new()
            }
        };
        let occupancy = match before_diagnostics_deadline(started, "channel utilization", || {
            self.dbus.channel_qualities()
        }) {
            Ok(occupancy) => {
                successful_sources += 1;
                sources.push(fresh_source(ThreadScanSource::ChannelUtilization));
                occupancy
            }
            Err(error) => {
                let error = format!("Channel utilization is unavailable: {error}");
                warnings.push(error.clone());
                sources.push(unavailable_source(
                    ThreadScanSource::ChannelUtilization,
                    error,
                ));
                Vec::new()
            }
        };
        let (statistics, statistics_warnings, statistics_available) = if started.elapsed()
            >= DIAGNOSTICS_DEADLINE
        {
            (
                ThreadRadioStatistics::default(),
                vec![
                    "The overall diagnostics deadline elapsed before radio statistics".to_string(),
                ],
                false,
            )
        } else {
            self.dbus.radio_statistics()
        };
        successful_sources += u8::from(statistics_available);
        if statistics_available {
            sources.push(fresh_source(ThreadScanSource::RadioStatistics));
        } else {
            sources.push(unavailable_source(
                ThreadScanSource::RadioStatistics,
                if statistics_warnings.is_empty() {
                    "Radio statistics are unavailable".to_string()
                } else {
                    statistics_warnings.join("; ")
                },
            ));
        }
        warnings.extend(statistics_warnings);
        let devices = match before_diagnostics_deadline(started, "mesh discovery", || {
            self.rest.scan_mesh_devices()
        }) {
            Ok(devices) => {
                successful_sources += 1;
                sources.push(fresh_source(ThreadScanSource::MeshDevices));
                devices
            }
            Err(error) => {
                let error = format!("Mesh discovery is unavailable: {error}");
                warnings.push(error.clone());
                sources.push(unavailable_source(ThreadScanSource::MeshDevices, error));
                Vec::new()
            }
        };
        let channels = (11..=26)
            .map(|channel| ThreadChannelDiagnostics {
                channel,
                max_rssi: energy
                    .iter()
                    .find(|result| result.0 == channel)
                    .map(|result| result.1),
                occupancy: occupancy
                    .iter()
                    .find(|result| result.0 == channel)
                    .map(|result| result.1),
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
        debug!(
            duration_ms = started.elapsed().as_millis(),
            successful_sources,
            network_count = networks.len(),
            device_count = devices.len(),
            warning_count = warnings.len(),
            "Completed OpenThread diagnostics scan"
        );
        Ok(ThreadScan {
            channels,
            networks,
            devices,
            statistics,
            sources,
            warnings,
        })
    }

    pub(crate) fn active_dataset(&self) -> Result<ThreadActiveDataset> {
        let _dataset = self.read_dataset();
        parse_active_dataset_bytes(&self.dbus.active_dataset_tlvs()?)
    }

    pub(crate) fn create_network(&self, network: &CreateNetwork) -> Result<()> {
        let _dataset = self.write_dataset();
        let previous_dataset = self.dbus.active_dataset_tlvs_if_present()?;
        let was_enabled = self.was_enabled()?;
        if let Err(error) = self
            .dbus
            .create_network(network)
            .and_then(|()| self.verify_created_network(network))
        {
            return Err(self.rollback_error(error, previous_dataset, was_enabled));
        }
        info!(
            network_name = %network.network_name(),
            channel = ?network.channel(),
            "Created a new Thread network"
        );
        Ok(())
    }

    pub(crate) fn import_active_dataset(&self, dataset: Zeroizing<Vec<u8>>) -> Result<()> {
        let _dataset_guard = self.write_dataset();
        self.import_active_dataset_unlocked(dataset)
    }

    fn import_active_dataset_unlocked(&self, dataset: Zeroizing<Vec<u8>>) -> Result<()> {
        let previous_dataset = self.dbus.active_dataset_tlvs_if_present()?;
        let was_enabled = self.was_enabled()?;
        let result = self.dbus.detach().and_then(|()| {
            self.dbus.set_active_dataset_tlvs(&dataset)?;
            self.dbus.attach_current_dataset()?;
            let active = self.dbus.active_dataset_tlvs()?;
            if active.as_slice() != dataset.as_slice() {
                return Err(OpenThreadError::InvalidResponse(
                    "OTBR did not activate the imported operational dataset".to_string(),
                ));
            }
            Ok(())
        });
        if let Err(error) = result {
            return Err(self.rollback_error(error, previous_dataset, was_enabled));
        }
        info!("Imported and activated a Thread operational dataset");
        Ok(())
    }

    pub(crate) fn ensure_default_development_network(&self) -> Result<bool> {
        let _dataset_guard = self.write_dataset();
        if self.dbus.active_dataset_tlvs_if_present()?.is_some() {
            return Ok(false);
        }
        self.import_active_dataset_unlocked(parse_imported_dataset(
            DEFAULT_DEVELOPMENT_DATASET_TLVS,
        )?)?;
        Ok(true)
    }

    fn verify_created_network(&self, expected: &CreateNetwork) -> Result<()> {
        let status = self.dbus.status(Vec::new())?;
        if status.network_name.as_deref() != Some(expected.network_name()) {
            return Err(OpenThreadError::InvalidResponse(
                "OTBR created a network with an unexpected name".to_string(),
            ));
        }
        if expected.channel().is_some() && status.channel != expected.channel() {
            return Err(OpenThreadError::InvalidResponse(
                "OTBR created a network on an unexpected channel".to_string(),
            ));
        }
        if let Some(pan_id) = expected.pan_id()
            && status.pan_id.as_deref() != Some(pan_id)
        {
            return Err(OpenThreadError::InvalidResponse(
                "OTBR created a network with an unexpected PAN ID".to_string(),
            ));
        }
        if let Some(extended_pan_id) = expected.extended_pan_id()
            && status.extended_pan_id.as_deref() != Some(extended_pan_id)
        {
            return Err(OpenThreadError::InvalidResponse(
                "OTBR created a network with an unexpected Extended PAN ID".to_string(),
            ));
        }
        let dataset = parse_active_dataset_bytes(&self.dbus.active_dataset_tlvs()?)?;
        if let Some(key) = expected.network_key()
            && dataset.expose_network_key() != Some(key)
        {
            return Err(OpenThreadError::InvalidResponse(
                "OTBR created a network with an unexpected network key".to_string(),
            ));
        }
        Ok(())
    }

    fn was_enabled(&self) -> Result<bool> {
        Ok(self.dbus.status(Vec::new())?.role != ThreadRole::Disabled)
    }

    fn rollback_error(
        &self,
        original: OpenThreadError,
        previous_dataset: Option<Zeroizing<Vec<u8>>>,
        was_enabled: bool,
    ) -> OpenThreadError {
        match self.restore_dataset(previous_dataset, was_enabled) {
            Ok(()) => OpenThreadError::Control(format!(
                "{original}; the previous Thread network was restored"
            )),
            Err(rollback) => OpenThreadError::Control(format!(
                "{original}; restoring the previous Thread network also failed: {rollback}"
            )),
        }
    }

    fn restore_dataset(
        &self,
        previous_dataset: Option<Zeroizing<Vec<u8>>>,
        was_enabled: bool,
    ) -> Result<()> {
        self.dbus.detach()?;
        match previous_dataset {
            Some(dataset) if !dataset.is_empty() => {
                self.dbus.set_active_dataset_tlvs(&dataset)?;
                if was_enabled {
                    self.dbus.attach_current_dataset()?;
                }
            }
            _ => self.dbus.factory_reset()?,
        }
        Ok(())
    }

    fn lock_scan(&self) -> MutexGuard<'_, ()> {
        self.scan_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn read_dataset(&self) -> RwLockReadGuard<'_, ()> {
        self.dataset_lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_dataset(&self) -> RwLockWriteGuard<'_, ()> {
        self.dataset_lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn deduplicate_networks(networks: Vec<ThreadNetwork>) -> Vec<ThreadNetwork> {
    let mut unique = BTreeMap::<String, ThreadNetwork>::new();
    for network in networks {
        let key = network.extended_pan_id.clone().unwrap_or_else(|| {
            format!(
                "{}:{}:{}",
                network.extended_address, network.pan_id, network.channel
            )
        });
        unique
            .entry(key)
            .and_modify(|current| {
                if network.rssi > current.rssi {
                    *current = network.clone();
                } else if current.network_name.is_none() && network.network_name.is_some() {
                    current.network_name = network.network_name.clone();
                }
            })
            .or_insert(network);
    }
    unique.into_values().collect()
}

fn before_diagnostics_deadline<T>(
    started: Instant,
    source: &str,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if started.elapsed() >= DIAGNOSTICS_DEADLINE {
        Err(OpenThreadError::Control(format!(
            "the overall diagnostics deadline elapsed before {source}"
        )))
    } else {
        operation()
    }
}

fn fresh_source(source: ThreadScanSource) -> ThreadScanSourceStatus {
    ThreadScanSourceStatus {
        source,
        state: ThreadObservationState::Fresh,
        observed_at: Some(SystemTime::now()),
        error: None,
    }
}

fn unavailable_source(source: ThreadScanSource, error: String) -> ThreadScanSourceStatus {
    ThreadScanSourceStatus {
        source,
        state: ThreadObservationState::Unavailable,
        observed_at: None,
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct FakeDbus {
        state: Mutex<FakeDbusState>,
    }

    struct FakeDbusState {
        active_dataset: Option<Vec<u8>>,
        active_dataset_response: Option<Vec<u8>>,
        statuses: VecDeque<ThreadStatus>,
        calls: Vec<&'static str>,
        set_dataset_failures: usize,
    }

    impl FakeDbus {
        fn new(active_dataset: Vec<u8>, statuses: Vec<ThreadStatus>) -> Self {
            Self {
                state: Mutex::new(FakeDbusState {
                    active_dataset: Some(active_dataset),
                    active_dataset_response: None,
                    statuses: statuses.into(),
                    calls: Vec::new(),
                    set_dataset_failures: 0,
                }),
            }
        }

        fn lock(&self) -> MutexGuard<'_, FakeDbusState> {
            self.state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }
    }

    impl OtbrDbusControl for FakeDbus {
        fn health_check(&self) -> Result<()> {
            Ok(())
        }

        fn verify_capabilities(&self) -> Result<()> {
            Ok(())
        }

        fn status(&self, _addresses: Vec<String>) -> Result<ThreadStatus> {
            let mut state = self.lock();
            if state.statuses.len() > 1 {
                Ok(state.statuses.pop_front().expect("status was present"))
            } else {
                state.statuses.front().cloned().ok_or_else(|| {
                    OpenThreadError::Control("fake status was not configured".to_string())
                })
            }
        }

        fn scan_networks(&self) -> Result<Vec<ThreadNetwork>> {
            Ok(Vec::new())
        }

        fn energy_scan(&self, _duration_ms: u32) -> Result<Vec<(u16, i16)>> {
            Ok(Vec::new())
        }

        fn channel_qualities(&self) -> Result<Vec<(u16, u16)>> {
            Ok(Vec::new())
        }

        fn radio_statistics(&self) -> (ThreadRadioStatistics, Vec<String>, bool) {
            (ThreadRadioStatistics::default(), Vec::new(), false)
        }

        fn active_dataset_tlvs(&self) -> Result<Zeroizing<Vec<u8>>> {
            let state = self.lock();
            state
                .active_dataset_response
                .as_ref()
                .or(state.active_dataset.as_ref())
                .cloned()
                .map(Zeroizing::new)
                .ok_or_else(|| {
                    OpenThreadError::InvalidDataset("fake dataset was not configured".to_string())
                })
        }

        fn active_dataset_tlvs_if_present(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
            Ok(self.lock().active_dataset.clone().map(Zeroizing::new))
        }

        fn set_active_dataset_tlvs(&self, dataset: &[u8]) -> Result<()> {
            let mut state = self.lock();
            state.calls.push("set_dataset");
            if state.set_dataset_failures > 0 {
                state.set_dataset_failures -= 1;
                return Err(OpenThreadError::Control(
                    "simulated dataset restore failure".to_string(),
                ));
            }
            state.active_dataset = Some(dataset.to_vec());
            Ok(())
        }

        fn attach_current_dataset(&self) -> Result<()> {
            self.lock().calls.push("attach");
            Ok(())
        }

        fn detach(&self) -> Result<()> {
            self.lock().calls.push("detach");
            Ok(())
        }

        fn factory_reset(&self) -> Result<()> {
            let mut state = self.lock();
            state.calls.push("factory_reset");
            state.active_dataset = None;
            Ok(())
        }

        fn create_network(&self, _network: &CreateNetwork) -> Result<()> {
            self.lock().calls.push("create_network");
            Ok(())
        }
    }

    struct FakeRest;

    impl OtbrRestControl for FakeRest {
        fn health_check(&self) -> Result<()> {
            Ok(())
        }

        fn verify_authentication(&self) -> Result<()> {
            Ok(())
        }

        fn scan_mesh_devices(&self) -> Result<Vec<ThreadMeshDevice>> {
            Ok(Vec::new())
        }
    }

    fn attached_status(network_name: &str) -> ThreadStatus {
        ThreadStatus {
            role: ThreadRole::Leader,
            network_name: Some(network_name.to_string()),
            channel: Some(15),
            pan_id: Some("1234".to_string()),
            extended_pan_id: Some("0011223344556677".to_string()),
            mesh_local_prefix: Some("fd00:db8::/64".to_string()),
            addresses: Vec::new(),
        }
    }

    fn fake_controller(dbus: Arc<FakeDbus>) -> ThreadController {
        ThreadController::new_with_transports(dbus, Arc::new(FakeRest), "wpan0".to_string())
    }

    #[test]
    fn scan_deduplicates_same_mesh_by_extended_pan_id() {
        let networks = vec![
            ThreadNetwork {
                network_name: Some("Mesh".to_string()),
                pan_id: "1234".to_string(),
                extended_address: "0011223344556677".to_string(),
                extended_pan_id: Some("8899aabbccddeeff".to_string()),
                channel: 15,
                rssi: -70,
                lqi: 2,
            },
            ThreadNetwork {
                network_name: Some("Mesh".to_string()),
                pan_id: "1234".to_string(),
                extended_address: "1021324354657687".to_string(),
                extended_pan_id: Some("8899aabbccddeeff".to_string()),
                channel: 15,
                rssi: -40,
                lqi: 3,
            },
        ];
        let unique = deduplicate_networks(networks);
        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].rssi, -40);
    }

    #[test]
    fn source_failures_are_typed_for_stale_data_merging() {
        let status = unavailable_source(
            ThreadScanSource::MeshDevices,
            "REST unavailable".to_string(),
        );
        assert_eq!(status.state, ThreadObservationState::Unavailable);
        assert_eq!(status.source, ThreadScanSource::MeshDevices);
        assert!(status.observed_at.is_none());
    }

    #[test]
    fn default_development_network_is_seeded_only_when_no_dataset_exists() {
        let dbus = Arc::new(FakeDbus::new(
            Vec::new(),
            vec![attached_status("existing or newly seeded network")],
        ));
        dbus.lock().active_dataset = None;
        let controller = fake_controller(dbus.clone());

        assert!(controller.ensure_default_development_network().unwrap());
        let expected = parse_imported_dataset(DEFAULT_DEVELOPMENT_DATASET_TLVS).unwrap();
        {
            let state = dbus.lock();
            assert_eq!(state.active_dataset.as_deref(), Some(expected.as_slice()));
            assert_eq!(state.calls, ["detach", "set_dataset", "attach"]);
        }

        assert!(!controller.ensure_default_development_network().unwrap());
        assert_eq!(dbus.lock().calls, ["detach", "set_dataset", "attach"]);
    }

    #[test]
    fn create_network_restores_previous_dataset_when_verification_fails() {
        let previous_dataset = parse_imported_dataset(DEFAULT_DEVELOPMENT_DATASET_TLVS).unwrap();
        let expected = CreateNetwork::new(
            "Expected".to_string(),
            Some(15),
            Some("1234".to_string()),
            Some("0011223344556677".to_string()),
            None,
        )
        .unwrap();
        let dbus = Arc::new(FakeDbus::new(
            previous_dataset.to_vec(),
            vec![attached_status("Previous"), attached_status("Unexpected")],
        ));
        let controller = fake_controller(dbus.clone());

        let error = controller.create_network(&expected).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("previous Thread network was restored")
        );
        let state = dbus.lock();
        assert_eq!(
            state.active_dataset.as_deref(),
            Some(previous_dataset.as_slice())
        );
        assert_eq!(
            state.calls,
            ["create_network", "detach", "set_dataset", "attach"]
        );
    }

    #[test]
    fn import_restores_previous_dataset_when_activation_is_not_observed() {
        let previous_dataset = parse_imported_dataset(DEFAULT_DEVELOPMENT_DATASET_TLVS).unwrap();
        let imported_dataset = Zeroizing::new(
            previous_dataset
                .iter()
                .enumerate()
                .map(|(index, byte)| if index == 5 { byte ^ 1 } else { *byte })
                .collect::<Vec<_>>(),
        );
        let dbus = Arc::new(FakeDbus::new(
            previous_dataset.to_vec(),
            vec![attached_status("Previous")],
        ));
        dbus.lock().active_dataset_response = Some(previous_dataset.to_vec());
        let controller = fake_controller(dbus.clone());

        let error = controller
            .import_active_dataset(imported_dataset)
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("previous Thread network was restored")
        );
        let state = dbus.lock();
        assert_eq!(
            state.active_dataset.as_deref(),
            Some(previous_dataset.as_slice())
        );
        assert_eq!(
            state.calls,
            [
                "detach",
                "set_dataset",
                "attach",
                "detach",
                "set_dataset",
                "attach"
            ]
        );
    }

    #[test]
    fn rollback_failure_preserves_both_error_contexts() {
        let previous_dataset = parse_imported_dataset(DEFAULT_DEVELOPMENT_DATASET_TLVS).unwrap();
        let expected =
            CreateNetwork::new("Expected".to_string(), Some(15), None, None, None).unwrap();
        let dbus = Arc::new(FakeDbus::new(
            previous_dataset.to_vec(),
            vec![attached_status("Previous"), attached_status("Unexpected")],
        ));
        dbus.lock().set_dataset_failures = 1;
        let controller = fake_controller(dbus);

        let error = controller
            .create_network(&expected)
            .unwrap_err()
            .to_string();

        assert!(error.contains("unexpected name"));
        assert!(error.contains("restoring the previous Thread network also failed"));
        assert!(error.contains("simulated dataset restore failure"));
    }
}
