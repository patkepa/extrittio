use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use extrittio_common::extrittio::{DeviceHeartbeat, DeviceTelemetry};
use extrittio_common::{device_status, topics};
use log::info;
use prost::Message;
use serde_json::json;

// Network explorer example for an ESP32-class board running as a WiFi station.
//
// It discovers hosts from the ESP32's local subnet by actively probing common
// TCP services. A station cannot ask most consumer APs for their association
// table, so MAC address, vendor, and hostname are only available if you add a
// network-specific source for them. The emitted telemetry calls this out.

const DEVICE_ID: &str = "esp32-network-explorer-001";
const WIFI_SSID: &str = "your-wifi-ssid";
const WIFI_PASS: &str = "your-wifi-password";
const FIRMWARE_VERSION: &str = "v1.0.0-network-explorer";

const TELEMETRY_INTERVAL_SECS: u64 = 300;
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

// Zenoh router endpoint. Leave empty to use Zenoh multicast scouting.
const ZENOH_CONNECT: &str = "tcp/192.0.2.100:7447";

// ESP32 deployments usually scan a /24 home/office LAN. Adjust for larger or
// smaller networks; avoid broad scans on networks you do not own/administer.
const SCAN_PREFIX_LEN: u8 = 24;
const MAX_HOSTS_REPORTED: usize = 64;
const CONNECT_TIMEOUT_MS: u64 = 75;

const PROBE_PORTS: &[(u16, &str)] = &[
    (22, "ssh"),
    (23, "telnet"),
    (53, "dns"),
    (80, "http"),
    (139, "netbios"),
    (443, "https"),
    (445, "smb"),
    (554, "rtsp"),
    (1883, "mqtt"),
    (5000, "upnp-http"),
    (5001, "nas-https"),
    (5357, "ws-discovery"),
    (8080, "http-alt"),
    (8443, "https-alt"),
    (8883, "mqtts"),
    (9100, "printer"),
];

#[derive(Debug)]
struct OpenService {
    port: u16,
    service: &'static str,
    latency_ms: u128,
}

#[derive(Debug)]
struct HostObservation {
    ip: Ipv4Addr,
    open_services: Vec<OpenService>,
    closed_probe_ports: Vec<u16>,
    role_hints: Vec<&'static str>,
}

#[derive(Debug)]
struct NetworkSnapshot {
    local_ip: Ipv4Addr,
    network: Ipv4Addr,
    broadcast: Ipv4Addr,
    prefix_len: u8,
    scan_duration_ms: u128,
    hosts: Vec<HostObservation>,
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Extrittio ESP32 network explorer starting as '{DEVICE_ID}'");

    let peripherals = Peripherals::take().expect("Failed to take peripherals");
    let sysloop = EspSystemEventLoop::take().expect("Failed to take event loop");
    let nvs = EspDefaultNvsPartition::take().expect("Failed to take NVS partition");

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs)).expect("Failed to create WiFi"),
        sysloop,
    )
    .expect("Failed to wrap WiFi");

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().expect("SSID too long"),
        password: WIFI_PASS.try_into().expect("Password too long"),
        ..Default::default()
    }))
    .expect("Failed to set WiFi configuration");

    wifi.start().expect("Failed to start WiFi");
    wifi.connect().expect("Failed to connect to WiFi");
    wifi.wait_netif_up()
        .expect("Failed to bring up network interface");

    info!("WiFi connected");

    let mut zenoh_cfg = zenoh::Config::default();
    if !ZENOH_CONNECT.is_empty() {
        zenoh_cfg
            .connect
            .endpoints
            .set(vec![ZENOH_CONNECT.parse().expect("Bad Zenoh endpoint")])
            .expect("Failed to set Zenoh connect endpoints");
    }

    let session = zenoh::open(zenoh_cfg)
        .wait()
        .expect("Failed to open Zenoh session");
    let session = Arc::new(session);

    let telemetry_topic = topics::telemetry(DEVICE_ID);
    let heartbeat_topic = topics::heartbeat(DEVICE_ID);
    let start = Instant::now();

    let hb_session = session.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));

        let heartbeat = DeviceHeartbeat {
            device_id: DEVICE_ID.to_string(),
            timestamp: extrittio_sdk::time::now_millis(),
            status: device_status::ONLINE.to_string(),
            firmware: FIRMWARE_VERSION.to_string(),
            uptime_seconds: start.elapsed().as_secs() as i64,
        };

        if let Err(e) = hb_session
            .put(&heartbeat_topic, heartbeat.encode_to_vec())
            .wait()
        {
            log::warn!("Failed to send heartbeat: {e}");
        }
    });

    let _wifi = wifi;

    info!("Sending network exploration telemetry to '{telemetry_topic}'");

    loop {
        thread::sleep(Duration::from_secs(TELEMETRY_INTERVAL_SECS));

        let local_ip = match infer_local_ipv4() {
            Some(ip) => ip,
            None => {
                log::warn!("Unable to infer local IPv4 address; skipping scan");
                continue;
            }
        };

        let snapshot = scan_network(local_ip, SCAN_PREFIX_LEN);
        let metadata = metadata_from_snapshot(&snapshot);

        let telemetry = DeviceTelemetry {
            device_id: DEVICE_ID.to_string(),
            timestamp: extrittio_sdk::time::now_millis(),
            temperature: 0.0,
            humidity: 0.0,
            battery_level: 0.0,
            metadata,
            latitude: 0.0,
            longitude: 0.0,
            speed: 0.0,
            altitude: 0.0,
            heading: 0.0,
        };

        let host_count = snapshot.hosts.len();
        let payload = telemetry.encode_to_vec();
        match session.put(&telemetry_topic, payload).wait() {
            Ok(_) => info!(
                "Network telemetry sent: {} host(s), scan={}ms",
                host_count, snapshot.scan_duration_ms
            ),
            Err(e) => log::warn!("Failed to send telemetry: {e}"),
        }
    }
}

fn infer_local_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

fn scan_network(local_ip: Ipv4Addr, prefix_len: u8) -> NetworkSnapshot {
    let started = Instant::now();
    let prefix_len = prefix_len.clamp(1, 30);
    let mask = u32::MAX << (32 - prefix_len);
    let local = u32::from(local_ip);
    let network = local & mask;
    let broadcast = network | !mask;

    let mut hosts = Vec::new();

    for candidate in network + 1..broadcast {
        let ip = Ipv4Addr::from(candidate);
        if ip == local_ip {
            continue;
        }

        if let Some(host) = probe_host(ip) {
            hosts.push(host);
        }

        if hosts.len() >= MAX_HOSTS_REPORTED {
            break;
        }
    }

    NetworkSnapshot {
        local_ip,
        network: Ipv4Addr::from(network),
        broadcast: Ipv4Addr::from(broadcast),
        prefix_len,
        scan_duration_ms: started.elapsed().as_millis(),
        hosts,
    }
}

fn probe_host(ip: Ipv4Addr) -> Option<HostObservation> {
    let mut open_services = Vec::new();
    let mut closed_probe_ports = Vec::new();
    let timeout = Duration::from_millis(CONNECT_TIMEOUT_MS);

    for (port, service) in PROBE_PORTS {
        let socket = SocketAddr::new(IpAddr::V4(ip), *port);
        let started = Instant::now();

        match TcpStream::connect_timeout(&socket, timeout) {
            Ok(_) => {
                open_services.push(OpenService {
                    port: *port,
                    service: *service,
                    latency_ms: started.elapsed().as_millis(),
                });
            }
            Err(err) if err.kind() == ErrorKind::ConnectionRefused => {
                closed_probe_ports.push(*port);
            }
            Err(_) => {}
        }
    }

    if open_services.is_empty() && closed_probe_ports.is_empty() {
        None
    } else {
        let role_hints = infer_role_hints(ip, &open_services);
        Some(HostObservation {
            ip,
            open_services,
            closed_probe_ports,
            role_hints,
        })
    }
}

fn infer_role_hints(ip: Ipv4Addr, services: &[OpenService]) -> Vec<&'static str> {
    let mut hints = Vec::new();
    let last_octet = ip.octets()[3];

    if last_octet == 1 || last_octet == 254 {
        hints.push("gateway_candidate");
    }

    if has_port(services, 53) {
        hints.push("dns_candidate");
    }
    if has_port(services, 80) || has_port(services, 443) || has_port(services, 8080) {
        hints.push("web_ui");
    }
    if has_port(services, 1883) || has_port(services, 8883) {
        hints.push("mqtt_broker_or_iot_device");
    }
    if has_port(services, 445) || has_port(services, 139) {
        hints.push("file_sharing");
    }
    if has_port(services, 554) {
        hints.push("camera_or_media_device");
    }
    if has_port(services, 9100) {
        hints.push("printer");
    }

    hints
}

fn has_port(services: &[OpenService], port: u16) -> bool {
    services.iter().any(|service| service.port == port)
}

fn metadata_from_snapshot(snapshot: &NetworkSnapshot) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    metadata.insert("explorer.kind".into(), "esp32_wifi_network_explorer".into());
    metadata.insert(
        "explorer.discovery_method".into(),
        "active_tcp_connect".into(),
    );
    metadata.insert(
        "explorer.limitations".into(),
        "ESP32 station mode cannot read most AP client tables; mac_address, vendor, hostname, and silent hosts require ARP/DHCP/router integration"
            .into(),
    );

    metadata.insert("network.local_ip".into(), snapshot.local_ip.to_string());
    metadata.insert("network.prefix_len".into(), snapshot.prefix_len.to_string());
    metadata.insert(
        "network.cidr".into(),
        format!("{}/{}", snapshot.network, snapshot.prefix_len),
    );
    metadata.insert("network.broadcast".into(), snapshot.broadcast.to_string());
    metadata.insert(
        "network.scan_duration_ms".into(),
        snapshot.scan_duration_ms.to_string(),
    );
    metadata.insert(
        "network.host_count".into(),
        snapshot.hosts.len().to_string(),
    );
    metadata.insert(
        "network.probe_ports".into(),
        PROBE_PORTS
            .iter()
            .map(|(port, name)| format!("{port}:{name}"))
            .collect::<Vec<_>>()
            .join(","),
    );

    let hosts_json = snapshot
        .hosts
        .iter()
        .map(|host| {
            json!({
                "ip": host.ip.to_string(),
                "mac_address": null,
                "hostname": null,
                "vendor": null,
                "role_hints": host.role_hints,
                "closed_probe_ports": host.closed_probe_ports,
                "open_services": host.open_services.iter().map(|service| {
                    json!({
                        "port": service.port,
                        "name": service.service,
                        "connect_latency_ms": service.latency_ms,
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    metadata.insert(
        "network.hosts_json".into(),
        serde_json::to_string(&hosts_json).unwrap_or_else(|_| "[]".to_string()),
    );

    metadata
}
