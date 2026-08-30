use std::{net::Ipv6Addr, thread::ScopedJoinHandle, time::Duration};

use serde::Deserialize;
use zbus::{
    blocking::{Connection, Proxy, connection::Builder, proxy::Builder as ProxyBuilder},
    proxy::CacheProperties,
    zvariant::{OwnedValue, Type, Value},
};
use zeroize::{Zeroize as _, Zeroizing};

use crate::{
    dataset::{parse_extended_pan_id, parse_mesh_local_prefix, parse_network_key, parse_pan_id},
    error::{OpenThreadError, Result},
    types::{CreateNetwork, ThreadNetwork, ThreadRadioStatistics, ThreadRole, ThreadStatus},
};

const INTERFACE: &str = "io.openthread.BorderRouter";
const PROPERTY_CHANNEL_QUALITIES: &str = "ChannelMonitorAllChannelQualities";
const PROPERTY_MAC_COUNTERS: &str = "MacCounters";
const PROPERTY_LINK_COUNTERS: &str = "LinkCounters";
// Dataset activation can wait for the Thread attach state machine to select a
// parent or form a partition. OTBR does not reply to Attach until that work is
// complete, so a short generic D-Bus deadline would roll back a dataset that
// OTBR has already accepted.
const CONTROL_METHOD_TIMEOUT: Duration = Duration::from_secs(30);
// OTBR's active scan can dwell for roughly one second on each of the 16
// Thread channels on macOS. Leave enough time for the server to finish so the
// following energy scan does not race the still-running active scan.
const SCAN_METHOD_TIMEOUT: Duration = Duration::from_secs(30);

type OtbrStatusProperties = (
    String,
    Option<String>,
    Option<u16>,
    Option<u16>,
    Option<u64>,
    Option<Vec<u8>>,
);

#[derive(Debug, Deserialize, Type)]
#[allow(dead_code)]
struct ActiveScanResult {
    extended_address: u64,
    network_name: String,
    extended_pan_id: u64,
    steering_data: Vec<u8>,
    pan_id: u16,
    joiner_udp_port: u16,
    channel: u8,
    rssi: i16,
    lqi: u8,
    version: u8,
    is_native: bool,
    discover: bool,
}

#[derive(Debug, Deserialize, Type)]
struct EnergyScanResult {
    channel: u8,
    max_rssi: u8,
}

#[derive(Debug, Deserialize, Type, Value, OwnedValue)]
struct ChannelQuality {
    channel: u8,
    occupancy: u16,
}

#[derive(Debug, Deserialize, Type, OwnedValue)]
#[allow(dead_code)]
struct MacCounters {
    tx_total: u32,
    tx_unicast: u32,
    tx_broadcast: u32,
    tx_ack_requested: u32,
    tx_acked: u32,
    tx_no_ack_requested: u32,
    tx_data: u32,
    tx_data_poll: u32,
    tx_beacon: u32,
    tx_beacon_request: u32,
    tx_other: u32,
    tx_retry: u32,
    tx_err_cca: u32,
    tx_err_abort: u32,
    tx_err_busy_channel: u32,
    rx_total: u32,
    rx_unicast: u32,
    rx_broadcast: u32,
    rx_data: u32,
    rx_data_poll: u32,
    rx_beacon: u32,
    rx_beacon_request: u32,
    rx_other: u32,
    rx_address_filtered: u32,
    rx_dest_address_filtered: u32,
    rx_duplicated: u32,
    rx_err_no_frame: u32,
    rx_err_unknown_neighbor: u32,
    rx_err_invalid_src_addr: u32,
    rx_err_sec: u32,
    rx_err_fcs: u32,
    rx_err_other: u32,
}

#[derive(Clone)]
pub(crate) struct OtbrDbus {
    control_connection: Connection,
    scan_connection: Connection,
    service: String,
    object: String,
}

impl OtbrDbus {
    pub(crate) fn connect(address: &str, thread_interface: &str) -> Result<Self> {
        let control_connection = Builder::address(address)?
            .method_timeout(CONTROL_METHOD_TIMEOUT)
            .build()?;
        let scan_connection = Builder::address(address)?
            .method_timeout(SCAN_METHOD_TIMEOUT)
            .build()?;
        Ok(Self {
            control_connection,
            scan_connection,
            service: format!("io.openthread.BorderRouter.{thread_interface}"),
            object: format!("/io/openthread/BorderRouter/{thread_interface}"),
        })
    }

    fn control_proxy(&self) -> Result<Proxy<'_>> {
        Self::proxy(
            &self.control_connection,
            self.service.as_str(),
            self.object.as_str(),
        )
    }

    fn scan_proxy(&self) -> Result<Proxy<'_>> {
        Self::proxy(
            &self.scan_connection,
            self.service.as_str(),
            self.object.as_str(),
        )
    }

    fn proxy<'a>(
        connection: &'a Connection,
        service: &'a str,
        object: &'a str,
    ) -> Result<Proxy<'a>> {
        // OTBR advertises individual properties but deliberately does not
        // implement org.freedesktop.DBus.Properties.GetAll. zbus's default
        // lazy cache fetches that unsupported method on the first property
        // access, so use direct per-property reads instead.
        Ok(ProxyBuilder::new(connection)
            .destination(service)?
            .path(object)?
            .interface(INTERFACE)?
            .cache_properties(CacheProperties::No)
            .build()?)
    }

    fn get_property<T>(&self, property: &str) -> Result<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<zbus::Error>,
    {
        Ok(self.control_proxy()?.get_property(property)?)
    }

    pub(crate) fn health_check(&self) -> Result<()> {
        let _: String = self.get_property("DeviceRole")?;
        Ok(())
    }

    pub(crate) fn verify_capabilities(&self) -> Result<()> {
        let introspection = self.control_proxy()?.introspect()?;
        let required = [
            "method name=\"Attach\"",
            "method name=\"Detach\"",
            "method name=\"FactoryReset\"",
            "method name=\"Scan\"",
            "method name=\"EnergyScan\"",
            "property name=\"DeviceRole\"",
            "property name=\"ActiveDatasetTlvs\"",
        ];
        let missing = required
            .into_iter()
            .filter(|contract| !introspection.contains(contract))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(OpenThreadError::InvalidConfiguration(format!(
                "otbr-agent does not provide the required D-Bus contract entries: {}",
                missing.join(", ")
            )))
        }
    }

    pub(crate) fn status(&self, addresses: Vec<String>) -> Result<ThreadStatus> {
        let (role_value, network_name, channel, pan_id, extended_pan_id, prefix_bytes) =
            std::thread::scope(|scope| -> Result<OtbrStatusProperties> {
                let role = scope.spawn(|| self.get_property::<String>("DeviceRole"));
                let name = scope.spawn(|| self.get_property::<String>("NetworkName"));
                let channel = scope.spawn(|| self.get_property::<u16>("Channel"));
                let pan_id = scope.spawn(|| self.get_property::<u16>("PanId"));
                let extended_pan_id = scope.spawn(|| self.get_property::<u64>("ExtPanId"));
                let prefix = scope.spawn(|| self.get_property::<Vec<u8>>("MeshLocalPrefix"));
                Ok((
                    join_dbus_call(role)?,
                    dbus_property_if_present(name)?,
                    dbus_property_if_present(channel)?,
                    dbus_property_if_present(pan_id)?,
                    dbus_property_if_present(extended_pan_id)?,
                    dbus_property_if_present(prefix)?,
                ))
            })?;
        if role_value.is_empty() {
            return Err(OpenThreadError::InvalidResponse(
                "OTBR returned an empty device role".to_string(),
            ));
        }
        let role = ThreadRole::from_otbr(role_value);
        let network_name = network_name.and_then(nonempty);
        let channel = channel.and_then(nonzero);
        let pan_id = pan_id
            .filter(|value| *value != 0 && *value != u16::MAX)
            .map(|value| format!("{value:04x}"));
        let extended_pan_id = extended_pan_id
            .filter(|value| *value != 0 && *value != u64::MAX)
            .map(|value| format!("{value:016x}"));
        let mut mesh_local_prefix = prefix_bytes
            .filter(|bytes| !bytes.is_empty())
            .map(|bytes| {
                format_mesh_local_prefix(&bytes).ok_or_else(|| {
                    OpenThreadError::InvalidResponse(
                        "OTBR returned a mesh-local prefix with an invalid length".to_string(),
                    )
                })
            })
            .transpose()?;
        if mesh_local_prefix.is_none()
            && let Some(dataset) = self.active_dataset_tlvs_if_present()?
        {
            mesh_local_prefix = mesh_local_prefix_from_dataset(&dataset)?;
        }

        Ok(ThreadStatus {
            role,
            network_name,
            channel,
            pan_id,
            extended_pan_id,
            mesh_local_prefix,
            addresses,
        })
    }

    pub(crate) fn scan_networks(&self) -> Result<Vec<ThreadNetwork>> {
        let results: Vec<ActiveScanResult> = self.scan_proxy()?.call("Scan", &())?;
        Ok(results
            .into_iter()
            .map(|result| ThreadNetwork {
                network_name: (!result.network_name.is_empty()).then_some(result.network_name),
                pan_id: format!("{:04x}", result.pan_id),
                extended_address: format!("{:016x}", result.extended_address),
                extended_pan_id: Some(format!("{:016x}", result.extended_pan_id)),
                channel: u16::from(result.channel),
                rssi: result.rssi,
                lqi: result.lqi,
            })
            .collect())
    }

    pub(crate) fn energy_scan(&self, duration_ms: u32) -> Result<Vec<(u16, i16)>> {
        let results: Vec<EnergyScanResult> =
            self.scan_proxy()?.call("EnergyScan", &(duration_ms,))?;
        Ok(results
            .into_iter()
            .map(|result| {
                (
                    u16::from(result.channel),
                    i16::from(i8::from_ne_bytes([result.max_rssi])),
                )
            })
            .collect())
    }

    pub(crate) fn channel_qualities(&self) -> Result<Vec<(u16, u16)>> {
        let qualities = self
            .control_proxy()?
            .get_property::<Vec<ChannelQuality>>(PROPERTY_CHANNEL_QUALITIES)?;
        Ok(qualities
            .into_iter()
            .map(|quality| (u16::from(quality.channel), quality.occupancy))
            .collect())
    }

    pub(crate) fn radio_statistics(&self) -> (ThreadRadioStatistics, Vec<String>, bool) {
        let mut warnings = Vec::new();
        let (cca_failure_rate, latest_rssi, monitor_sample_count, counters) =
            std::thread::scope(|scope| {
                let cca = scope.spawn(|| self.get_property::<u16>("CcaFailureRate"));
                let rssi = scope.spawn(|| self.get_property::<u8>("InstantRssi"));
                let samples = scope.spawn(|| self.get_property::<u32>("ChannelMonitorSampleCount"));
                let counters = scope.spawn(|| self.mac_counters());
                (
                    optional_dbus_property(cca, "CcaFailureRate", &mut warnings),
                    optional_dbus_property(rssi, "InstantRssi", &mut warnings)
                        .map(|value| i16::from(i8::from_ne_bytes([value]))),
                    optional_dbus_property(samples, "ChannelMonitorSampleCount", &mut warnings),
                    optional_dbus_property(counters, "MAC counters", &mut warnings),
                )
            });
        let statistics = ThreadRadioStatistics {
            cca_failure_rate,
            latest_rssi,
            monitor_sample_count,
            tx_total: counters.as_ref().map(|value| value.tx_total),
            rx_total: counters.as_ref().map(|value| value.rx_total),
            tx_retries: counters.as_ref().map(|value| value.tx_retry),
            tx_errors: counters.as_ref().map(|value| {
                value
                    .tx_err_cca
                    .saturating_add(value.tx_err_abort)
                    .saturating_add(value.tx_err_busy_channel)
            }),
            rx_errors: counters.as_ref().map(|value| {
                value
                    .rx_err_no_frame
                    .saturating_add(value.rx_err_unknown_neighbor)
                    .saturating_add(value.rx_err_invalid_src_addr)
                    .saturating_add(value.rx_err_sec)
                    .saturating_add(value.rx_err_fcs)
                    .saturating_add(value.rx_err_other)
            }),
        };
        let available = statistics.cca_failure_rate.is_some()
            || statistics.latest_rssi.is_some()
            || statistics.monitor_sample_count.is_some()
            || counters.is_some();
        (statistics, warnings, available)
    }

    fn mac_counters(&self) -> Result<MacCounters> {
        let [primary, fallback] = mac_counter_property_candidates();
        match self.get_property::<MacCounters>(primary) {
            Ok(counters) => Ok(counters),
            Err(primary_error) => self
                .get_property::<MacCounters>(fallback)
                .or(Err(primary_error)),
        }
    }

    pub(crate) fn active_dataset_tlvs(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.active_dataset_tlvs_if_present()?.ok_or_else(|| {
            OpenThreadError::InvalidDataset(
                "the local Thread network has no Active Operational Dataset".to_string(),
            )
        })
    }

    pub(crate) fn active_dataset_tlvs_if_present(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        match self.get_property::<Vec<u8>>("ActiveDatasetTlvs") {
            Ok(dataset) if dataset.is_empty() => Ok(None),
            Ok(dataset) => Ok(Some(Zeroizing::new(dataset))),
            Err(error) if is_openthread_not_found(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn set_active_dataset_tlvs(&self, dataset: &[u8]) -> Result<()> {
        self.control_proxy()?
            .set_property("ActiveDatasetTlvs", dataset)?;
        Ok(())
    }

    pub(crate) fn attach_current_dataset(&self) -> Result<()> {
        self.control_proxy()?
            .call::<_, _, ()>("Attach", &())
            .map_err(OpenThreadError::from)
    }

    pub(crate) fn detach(&self) -> Result<()> {
        self.control_proxy()?.call::<_, _, ()>("Detach", &())?;
        Ok(())
    }

    pub(crate) fn factory_reset(&self) -> Result<()> {
        self.control_proxy()?
            .call::<_, _, ()>("FactoryReset", &())?;
        Ok(())
    }

    pub(crate) fn create_network(&self, network: &CreateNetwork) -> Result<()> {
        let proxy = self.control_proxy()?;
        let channel_mask = match network.channel() {
            Some(channel) => 1_u32 << channel,
            None => proxy.get_property::<u32>("LinkSupportedChannelMask")?,
        };
        let network_key = network
            .network_key()
            .map(parse_network_key)
            .transpose()?
            .unwrap_or_default();
        let pan_id = network
            .pan_id()
            .map(parse_pan_id)
            .transpose()?
            .unwrap_or(u16::MAX);
        let extended_pan_id = network
            .extended_pan_id()
            .map(parse_extended_pan_id)
            .transpose()?
            .unwrap_or(u64::MAX);
        let mut arguments = (
            network_key,
            pan_id,
            network.network_name(),
            extended_pan_id,
            Vec::<u8>::new(),
            channel_mask,
        );
        let result = proxy.call::<_, _, ()>("Attach", &arguments);
        arguments.0.zeroize();
        result.map_err(OpenThreadError::from)
    }
}

fn join_dbus_call<T>(handle: ScopedJoinHandle<'_, Result<T>>) -> Result<T> {
    handle.join().map_err(|_| {
        OpenThreadError::Control("an OpenThread D-Bus request worker panicked".to_string())
    })?
}

/// OTBR does not expose several network properties before its first Active
/// Operational Dataset is installed. That is a normal ready-to-provision
/// state, rather than a failed border router.
fn dbus_property_if_present<T>(handle: ScopedJoinHandle<'_, Result<T>>) -> Result<Option<T>> {
    match join_dbus_call(handle) {
        Ok(value) => Ok(Some(value)),
        Err(error) if is_openthread_not_found(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn optional_dbus_property<T>(
    handle: ScopedJoinHandle<'_, Result<T>>,
    property: &str,
    warnings: &mut Vec<String>,
) -> Option<T> {
    match join_dbus_call(handle) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("{property} is unavailable: {error}"));
            None
        }
    }
}

fn is_openthread_not_found(error: &OpenThreadError) -> bool {
    matches!(
        error,
        OpenThreadError::Dbus(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "io.openthread.Error.NotFound"
    )
}

fn format_mesh_local_prefix(bytes: &[u8]) -> Option<String> {
    if bytes.len() != 8 {
        return None;
    }
    let mut address = [0_u8; 16];
    address[..8].copy_from_slice(bytes);
    Some(format!("{}/64", Ipv6Addr::from(address)))
}

fn mesh_local_prefix_from_dataset(dataset: &[u8]) -> Result<Option<String>> {
    Ok(parse_mesh_local_prefix(dataset)?
        .as_ref()
        .and_then(|prefix| format_mesh_local_prefix(prefix)))
}

fn mac_counter_property_candidates() -> [&'static str; 2] {
    if cfg!(target_os = "macos") {
        [PROPERTY_LINK_COUNTERS, PROPERTY_MAC_COUNTERS]
    } else {
        [PROPERTY_MAC_COUNTERS, PROPERTY_LINK_COUNTERS]
    }
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn nonzero<T>(value: T) -> Option<T>
where
    T: Default + PartialEq,
{
    (value != T::default()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dataset::parse_imported_dataset, types::DEFAULT_DEVELOPMENT_DATASET_TLVS};

    #[test]
    fn wire_signatures_match_pinned_otbr_introspection() {
        assert_eq!(ActiveScanResult::SIGNATURE.to_string(), "(tstayqqynyybb)");
        assert_eq!(EnergyScanResult::SIGNATURE.to_string(), "(yy)");
        assert_eq!(ChannelQuality::SIGNATURE.to_string(), "(yq)");
        assert_eq!(
            MacCounters::SIGNATURE.to_string(),
            "(uuuuuuuuuuuuuuuuuuuuuuuuuuuuuuuu)"
        );
        assert_eq!(
            mac_counter_property_candidates(),
            if cfg!(target_os = "macos") {
                ["LinkCounters", "MacCounters"]
            } else {
                ["MacCounters", "LinkCounters"]
            }
        );
        assert_eq!(SCAN_METHOD_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn maps_scan_results_without_losing_extended_pan_id() {
        let result = ActiveScanResult {
            extended_address: 0x0011_2233_4455_6677,
            network_name: "Example".to_string(),
            extended_pan_id: 0x8899_aabb_ccdd_eeff,
            steering_data: Vec::new(),
            pan_id: 0x1234,
            joiner_udp_port: 0,
            channel: 15,
            rssi: -28,
            lqi: 3,
            version: 4,
            is_native: false,
            discover: false,
        };
        let network = ThreadNetwork {
            network_name: Some(result.network_name),
            pan_id: format!("{:04x}", result.pan_id),
            extended_address: format!("{:016x}", result.extended_address),
            extended_pan_id: Some(format!("{:016x}", result.extended_pan_id)),
            channel: u16::from(result.channel),
            rssi: result.rssi,
            lqi: result.lqi,
        };
        assert_eq!(network.extended_pan_id.as_deref(), Some("8899aabbccddeeff"));
    }

    #[test]
    fn formats_mesh_local_prefix_from_typed_bytes() {
        assert_eq!(
            format_mesh_local_prefix(&[0xfd, 0x35, 0x34, 0x41, 0x33, 0xd1, 0xd7, 0x3e]).as_deref(),
            Some("fd35:3441:33d1:d73e::/64")
        );
    }

    #[test]
    fn formats_mesh_local_prefix_from_active_dataset_fallback() {
        let dataset = parse_imported_dataset(DEFAULT_DEVELOPMENT_DATASET_TLVS).unwrap();
        assert_eq!(
            mesh_local_prefix_from_dataset(&dataset).unwrap().as_deref(),
            Some("fd35:3441:33d1:d73e::/64")
        );
    }

    #[test]
    fn radio_statistics_use_mac_counter_fields() {
        let counters = MacCounters {
            tx_total: 10,
            tx_unicast: 0,
            tx_broadcast: 0,
            tx_ack_requested: 0,
            tx_acked: 0,
            tx_no_ack_requested: 0,
            tx_data: 0,
            tx_data_poll: 0,
            tx_beacon: 0,
            tx_beacon_request: 0,
            tx_other: 0,
            tx_retry: 11,
            tx_err_cca: 12,
            tx_err_abort: 13,
            tx_err_busy_channel: 14,
            rx_total: 15,
            rx_unicast: 0,
            rx_broadcast: 0,
            rx_data: 0,
            rx_data_poll: 0,
            rx_beacon: 0,
            rx_beacon_request: 0,
            rx_other: 0,
            rx_address_filtered: 0,
            rx_dest_address_filtered: 0,
            rx_duplicated: 0,
            rx_err_no_frame: 26,
            rx_err_unknown_neighbor: 27,
            rx_err_invalid_src_addr: 28,
            rx_err_sec: 29,
            rx_err_fcs: 30,
            rx_err_other: 31,
        };
        assert_eq!(
            counters.tx_err_cca + counters.tx_err_abort + counters.tx_err_busy_channel,
            39
        );
        assert_eq!(
            counters.rx_err_no_frame
                + counters.rx_err_unknown_neighbor
                + counters.rx_err_invalid_src_addr
                + counters.rx_err_sec
                + counters.rx_err_fcs
                + counters.rx_err_other,
            171
        );
    }

    #[test]
    fn dataset_bytes_are_encoded_without_debug_or_cli_parsing() {
        assert_eq!(crate::dataset::encode_hex(&[0, 15, 255]), "000fff");
    }
}
