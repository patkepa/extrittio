# Extrittio ESP32 Rust Client

## Network Explorer Example

`examples/network_explorer.rs` is an ESP32 WiFi station that reports local
network inventory through Extrittio telemetry.

It connects to WiFi, opens a Zenoh session, scans the local IPv4 subnet, probes
common TCP services, and publishes a `DeviceTelemetry` message where rich host
data is stored in `metadata`.

Build it from this directory:

```bash
cargo build --example network_explorer
```

Before flashing, edit these constants in
`examples/network_explorer.rs`:

- `WIFI_SSID`
- `WIFI_PASS`
- `ZENOH_CONNECT`
- `SCAN_PREFIX_LEN`

The main host inventory is in `metadata["network.hosts_json"]`. Each host entry
contains:

- `ip`
- `open_services` with port, service name, and connect latency
- `closed_probe_ports` when the host replied with TCP connection refused
- `role_hints` inferred from observed services
- `mac_address`, `hostname`, and `vendor` placeholders

An ESP32 connected as a normal WiFi station cannot usually read the access
point's client association table, DHCP lease table, or ARP cache for the whole
LAN. This example therefore reports hosts that respond to active TCP probes.
For MAC addresses, vendors, hostnames, and silent clients, integrate a
network-specific source such as the router API, DHCP lease export, mDNS, or an
ESP-IDF/lwIP ARP-table extension.
