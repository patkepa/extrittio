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

The firmware also reads runtime provisioning values from the default NVS
partition. Values in the `extrittio` namespace override the menuconfig
defaults:

- `device_id`
- `wifi_ssid`
- `wifi_pass`
- `zenoh`
- `fw_version`

The Extrittio CLI can create the backend device record and flash these NVS
values into a connected ESP32:

```bash
cargo run -p extrittio-cli -- provision \
  --name analyzer-001 \
  --device-type esp32-network-analyzer \
  --firmware v1.0.0-network-analyzer-c \
  --zenoh-connect tcp/192.168.0.10:7447 \
  --wifi-ssid "$WIFI_SSID" \
  --wifi-password "$WIFI_PASSWORD" \
  --flash-esp32-nvs
```

Run the provisioning command from an ESP-IDF shell so the CLI can find
`nvs_partition_gen.py`, `esptool.py`, and the ESP-IDF Python environment.

Register the configured or provisioned device ID in Extrittio before
publishing. Each scan is sent as a `DeviceTelemetry` message with
`metadata.kind=network_analyzer_scan` and `metadata.snapshot_json` containing
the scan payload.
