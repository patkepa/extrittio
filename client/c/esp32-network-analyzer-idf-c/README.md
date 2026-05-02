# ESP32 C Network Analyzer

ESP-IDF example that connects to WiFi, opens a Zenoh session, and publishes a
local IPv4 network scan to Extrittio once per minute.

The scan uses one-shot ICMP probes across the connected subnet, then attempts a
best-effort ARP lookup for MAC addresses. As a WiFi station, the ESP32 cannot
ask the access point for its association table, so hostnames and vendor names
are not available unless another discovery source is added later.

## Build

```bash
cd client/c/esp32-network-analyzer-idf-c
idf.py menuconfig
idf.py build flash monitor
```

Configure these values in `menuconfig`:

- `Extrittio Network Analyzer -> WiFi SSID`
- `Extrittio Network Analyzer -> WiFi Password`
- `Extrittio Network Analyzer -> Zenoh endpoint`
- `Extrittio Network Analyzer -> Device ID`

Register the configured device ID in Extrittio before publishing. Each scan is
sent as a `DeviceTelemetry` message with `metadata.kind=network_analyzer_scan`
and `metadata.snapshot_json` containing the scan payload.
