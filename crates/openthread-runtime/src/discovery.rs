use std::{
    collections::BTreeMap,
    env, fs,
    net::{IpAddr, Ipv6Addr, UdpSocket},
    path::{Path, PathBuf},
};

use nix::ifaddrs::getifaddrs;
use serialport::{SerialPortType, UsbPortInfo};
use tracing::debug;

use crate::{
    error::{OpenThreadError, Result},
    types::{ThreadRcpCandidate, ThreadRcpConfidence},
};

pub(crate) fn discover_rcp() -> Result<Option<PathBuf>> {
    let candidates = serial_candidate_details()?;
    let verified = candidates
        .iter()
        .filter(|candidate| candidate.confidence == ThreadRcpConfidence::Verified)
        .collect::<Vec<_>>();
    match verified.as_slice() {
        [candidate] => Ok(Some(candidate.path.clone())),
        [] if candidates.len() == 1 => Ok(Some(candidates[0].path.clone())),
        _ => Ok(None),
    }
}

pub(crate) fn serial_candidate_details() -> Result<Vec<ThreadRcpCandidate>> {
    #[cfg(target_os = "macos")]
    let roots = [Path::new("/dev")];
    #[cfg(target_os = "linux")]
    let roots = [Path::new("/dev/serial/by-id"), Path::new("/dev")];
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let roots: [&Path; 0] = [];

    let mut paths = Vec::new();
    let usb_metadata = usb_serial_metadata(&mut paths);
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries {
            let entry = entry.map_err(|source| OpenThreadError::FileSystem {
                operation: "inspect serial device",
                path: root.to_path_buf(),
                source,
            })?;
            let name = entry.file_name();
            if is_rcp_candidate_name(&name.to_string_lossy()) {
                paths.push(entry.path());
            }
        }
    }
    Ok(deduplicate_serial_paths(paths)
        .into_iter()
        .map(|path| {
            let identity = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let metadata = usb_metadata.get(&identity);
            let (confidence, match_reason) = candidate_confidence(&path, metadata);
            ThreadRcpCandidate {
                path,
                confidence,
                match_reason,
                usb_vendor_id: metadata.map(|metadata| metadata.vid),
                usb_product_id: metadata.map(|metadata| metadata.pid),
                manufacturer: metadata.and_then(|metadata| metadata.manufacturer.clone()),
                product: metadata.and_then(|metadata| metadata.product.clone()),
                serial_number: metadata.and_then(|metadata| metadata.serial_number.clone()),
            }
        })
        .collect())
}

fn usb_serial_metadata(paths: &mut Vec<PathBuf>) -> BTreeMap<PathBuf, UsbPortInfo> {
    let ports = match serialport::available_ports() {
        Ok(ports) => ports,
        Err(error) => {
            debug!(%error, "USB serial metadata enumeration is unavailable");
            return BTreeMap::new();
        }
    };
    let mut metadata = BTreeMap::new();
    for port in ports {
        let path = PathBuf::from(port.port_name);
        if !is_rcp_candidate_name(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
        ) {
            continue;
        }
        paths.push(path.clone());
        if let SerialPortType::UsbPort(usb) = port.port_type {
            let identity = fs::canonicalize(&path).unwrap_or(path);
            metadata.insert(identity, usb);
        }
    }
    metadata
}

fn candidate_confidence(
    path: &Path,
    metadata: Option<&UsbPortInfo>,
) -> (ThreadRcpConfidence, String) {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let metadata_identity = metadata.map_or_else(String::new, |metadata| {
        format!(
            "{} {}",
            metadata.manufacturer.as_deref().unwrap_or_default(),
            metadata.product.as_deref().unwrap_or_default()
        )
        .to_ascii_lowercase()
    });
    let explicit_tokens = ["openthread", "open_thread", "thread", "rcp"];
    let board_tokens = ["nordic", "nrf"];
    if explicit_tokens
        .iter()
        .any(|token| metadata_identity.contains(token))
    {
        return (
            ThreadRcpConfidence::Verified,
            "USB metadata identifies an OpenThread RCP".to_string(),
        );
    }
    if explicit_tokens.iter().any(|token| name.contains(token)) {
        return (
            ThreadRcpConfidence::Verified,
            "stable serial identity contains an OpenThread RCP marker".to_string(),
        );
    }
    if board_tokens
        .iter()
        .any(|token| metadata_identity.contains(token) || name.contains(token))
        || metadata.is_some_and(|metadata| metadata.vid == 0x1915)
    {
        return (
            ThreadRcpConfidence::Verified,
            "USB identity matches a Nordic nRF radio candidate".to_string(),
        );
    }
    (
        ThreadRcpConfidence::Plausible,
        "device matches a supported USB serial transport but has no RCP-specific identity"
            .to_string(),
    )
}

fn deduplicate_serial_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut by_identity = BTreeMap::<PathBuf, PathBuf>::new();
    for path in paths {
        let identity = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        by_identity
            .entry(identity)
            .and_modify(|current| {
                if is_stable_serial_path(&path) && !is_stable_serial_path(current) {
                    *current = path.clone();
                }
            })
            .or_insert(path);
    }
    let mut candidates = by_identity.into_values().collect::<Vec<_>>();
    candidates.sort();
    candidates
}

fn is_stable_serial_path(path: &Path) -> bool {
    path.starts_with("/dev/serial/by-id") || path.to_string_lossy().contains("/by-id/")
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

pub(crate) fn discover_agent(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return executable_if_present(path).map(Some);
    }
    if let Some(path) = env::var_os("EXTRITTIO_OTBR_AGENT") {
        return executable_if_present(Path::new(&path)).map(Some);
    }
    for path in automatic_agent_paths() {
        if is_executable(&path) {
            return Ok(Some(path));
        }
    }
    Ok(find_in_path("otbr-agent"))
}

pub(crate) fn executable_if_present(path: &Path) -> Result<PathBuf> {
    if !is_executable(path) {
        return Err(OpenThreadError::Discovery(format!(
            "OpenThread border-router executable is missing or not executable: {}",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

fn automatic_agent_paths() -> Vec<PathBuf> {
    env::current_exe().map_or_else(|_| Vec::new(), |path| agent_paths_next_to(&path))
}

fn agent_paths_next_to(executable: &Path) -> Vec<PathBuf> {
    let Some(bin_dir) = executable.parent() else {
        return Vec::new();
    };
    let mut paths = vec![
        bin_dir.join("../libexec/extrittio/otbr-agent"),
        bin_dir.join("otbr-agent"),
    ];

    // A repository Edge build lives at target/{debug,release}/extrittio,
    // while the pinned OTBR build produced by `cargo xtask otbr build` lives at
    // target/openthread/build. Keep that development layout just as automatic
    // as the installed libexec layout.
    if let Some(target_dir) = bin_dir.parent() {
        paths.push(target_dir.join("openthread/build/src/agent/otbr-agent"));
    }
    paths
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(program))
            .find(|candidate| is_executable(candidate))
    })
}

#[must_use]
pub fn default_infrastructure_interface() -> String {
    routed_local_address()
        .and_then(|local_address| {
            interface_addresses()
                .ok()?
                .into_iter()
                .find_map(|(interface, address)| (address == local_address).then_some(interface))
        })
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "en0".to_string()
            } else {
                "eth0".to_string()
            }
        })
}

fn routed_local_address() -> Option<IpAddr> {
    let ipv4 = UdpSocket::bind("0.0.0.0:0").ok()?;
    if ipv4.connect("192.0.2.1:9").is_ok()
        && let Ok(address) = ipv4.local_addr()
        && !address.ip().is_unspecified()
    {
        return Some(address.ip());
    }
    let ipv6 = UdpSocket::bind("[::]:0").ok()?;
    ipv6.connect("[2001:db8::1]:9").ok()?;
    let address = ipv6.local_addr().ok()?.ip();
    (!address.is_unspecified()).then_some(address)
}

pub(crate) fn thread_interface_ipv6_address(
    interface: &str,
    mesh_local_prefix: Option<&str>,
) -> Result<Ipv6Addr> {
    let addresses = thread_interface_ipv6_addresses(interface)?;
    let preferred_prefix = mesh_local_prefix.and_then(parse_ipv6_prefix);
    select_thread_ipv6_address(addresses, preferred_prefix).ok_or_else(|| {
        OpenThreadError::Control(
            "OTBR has no usable IPv6 address on its Thread interface".to_string(),
        )
    })
}

fn select_thread_ipv6_address(
    addresses: Vec<Ipv6Addr>,
    preferred_prefix: Option<Ipv6Addr>,
) -> Option<Ipv6Addr> {
    addresses
        .iter()
        .copied()
        .find(|address| {
            preferred_prefix.is_some_and(|prefix| same_ipv6_prefix_64(*address, prefix))
        })
        .or_else(|| addresses.into_iter().next())
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_mesh_local_ipv6_address(mesh_local_prefix: &str) -> Result<Ipv6Addr> {
    let prefix = parse_ipv6_prefix(mesh_local_prefix).ok_or_else(|| {
        OpenThreadError::InvalidResponse(
            "OTBR returned an invalid mesh-local IPv6 prefix".to_string(),
        )
    })?;
    interface_addresses()?
        .into_iter()
        .find_map(|(interface, address)| match address {
            IpAddr::V6(address)
                if interface.starts_with("utun") && same_ipv6_prefix_64(address, prefix) =>
            {
                Some(address)
            }
            _ => None,
        })
        .ok_or_else(|| {
            OpenThreadError::Control(
                "no macOS utun interface has an address in the OTBR mesh-local prefix".to_string(),
            )
        })
}

pub(crate) fn thread_interface_ipv6_addresses(interface: &str) -> Result<Vec<Ipv6Addr>> {
    let mut addresses = interface_addresses()?
        .into_iter()
        .filter_map(|(candidate, address)| {
            (candidate == interface)
                .then_some(address)
                .and_then(|address| match address {
                    IpAddr::V6(address) if usable_thread_address(address) => Some(address),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();
    Ok(addresses)
}

fn interface_addresses() -> Result<Vec<(String, IpAddr)>> {
    let interfaces = getifaddrs().map_err(|error| {
        OpenThreadError::Discovery(format!("unable to inspect network interfaces: {error}"))
    })?;
    let mut addresses = Vec::new();
    for interface in interfaces {
        let Some(address) = interface.address else {
            continue;
        };
        if let Some(address) = address.as_sockaddr_in() {
            addresses.push((interface.interface_name, IpAddr::V4(address.ip())));
        } else if let Some(address) = address.as_sockaddr_in6() {
            addresses.push((interface.interface_name, IpAddr::V6(address.ip())));
        }
    }
    Ok(addresses)
}

fn parse_ipv6_prefix(prefix: &str) -> Option<Ipv6Addr> {
    prefix
        .trim()
        .split_once('/')
        .map_or(prefix.trim(), |(address, _)| address)
        .parse()
        .ok()
}

fn same_ipv6_prefix_64(left: Ipv6Addr, right: Ipv6Addr) -> bool {
    left.octets()[..8] == right.octets()[..8]
}

fn usable_thread_address(address: Ipv6Addr) -> bool {
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && !address.is_unicast_link_local()
}

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_only_mesh_reachable_ipv6_addresses() {
        let addresses = parse_thread_ipv6_addresses(
            "inet6 ::1 prefixlen 128\ninet6 fe80::1234%wpan0 prefixlen 64\ninet6 fd12:3456::20/64 scope global\n",
        );
        assert_eq!(
            addresses,
            vec!["fd12:3456::20".parse::<Ipv6Addr>().unwrap()]
        );
    }

    #[test]
    fn compares_mesh_local_prefixes_at_sixty_four_bits() {
        assert!(same_ipv6_prefix_64(
            "fd35:3441:33d1:d73e::1".parse().unwrap(),
            "fd35:3441:33d1:d73e::".parse().unwrap(),
        ));
        assert!(!same_ipv6_prefix_64(
            "fd35:3441:33d1:d73f::1".parse().unwrap(),
            "fd35:3441:33d1:d73e::".parse().unwrap(),
        ));
    }

    #[test]
    fn prefers_the_address_matching_otbrs_mesh_local_prefix() {
        let omr = "fd11:22::1".parse().unwrap();
        let mesh_local = "fd35:3441:33d1:d73e::abcd".parse().unwrap();
        let prefix = "fd35:3441:33d1:d73e::".parse().unwrap();
        assert_eq!(
            select_thread_ipv6_address(vec![omr, mesh_local], Some(prefix)),
            Some(mesh_local)
        );
    }

    #[test]
    fn classifies_stable_rcp_identity_separately_from_generic_serial_ports() {
        assert_eq!(
            candidate_confidence(
                Path::new("/dev/serial/by-id/usb-Nordic_OpenThread_RCP"),
                None
            )
            .0,
            ThreadRcpConfidence::Verified
        );
        assert_eq!(
            candidate_confidence(Path::new("/dev/ttyACM0"), None).0,
            ThreadRcpConfidence::Plausible
        );
    }

    #[test]
    fn classifies_candidates_using_usb_metadata() {
        let metadata = UsbPortInfo {
            vid: 0x1915,
            pid: 0x521f,
            serial_number: Some("123456".to_string()),
            manufacturer: Some("Nordic Semiconductor".to_string()),
            product: Some("OpenThread RCP".to_string()),
        };
        let (confidence, reason) = candidate_confidence(Path::new("/dev/ttyACM0"), Some(&metadata));
        assert_eq!(confidence, ThreadRcpConfidence::Verified);
        assert!(reason.contains("USB metadata"));
    }

    #[test]
    fn repository_agent_path_matches_the_cargo_target_layout() {
        let executable = Path::new("/work/extrittio/target/release/extrittio");
        let paths = agent_paths_next_to(executable);

        assert!(
            paths.contains(
                &Path::new("/work/extrittio/target/openthread/build/src/agent/otbr-agent")
                    .to_path_buf()
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn deduplicates_serial_symlinks_and_prefers_stable_path() {
        use std::{os::unix::fs::symlink, time::SystemTime};

        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("extrittio-rcp-dedup-{suffix}"));
        let by_id = root.join("serial/by-id");
        fs::create_dir_all(&by_id).unwrap();
        let device = root.join("ttyACM0");
        fs::write(&device, []).unwrap();
        let stable = by_id.join("usb-Nordic_RCP");
        symlink(&device, &stable).unwrap();

        let candidates = deduplicate_serial_paths(vec![device, stable.clone()]);
        assert_eq!(candidates, vec![stable]);

        fs::remove_dir_all(root).unwrap();
    }
}
