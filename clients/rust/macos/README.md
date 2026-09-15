# macOS client

Provision a device from a published blueprint and download its
`GET /api/v1/devices/{id}/contract` response. The client requires that JSON file;
it verifies the hash and identity before opening a session.

```sh
extrittio-macos run --contract /absolute/path/device-contract.json
extrittio-macos install --contract /absolute/path/device-contract.json
```

Installation stores the absolute contract path in
`~/Library/Application Support/extrittio/config.toml` and installs the LaunchAgent.
The optional `--device-id` must match the contract. Endpoint and TLS settings can
be supplied in configuration. Heartbeat timing comes exclusively from the contract.

The client emits the `system` stream. Declare its payload in the blueprint:
numeric `cpu_usage_percent`, `memory_usage_percent`, `memory_total_gb`,
`net_rx_bytes`, `net_tx_bytes`, `system_uptime_secs`, `load_1m`, `load_5m`, `load_15m`;
string `hostname`, `os_version`, `chip_model`; and optional battery fields
`batteryLevel`, `battery_state`, `battery_cycles`, `battery_health_percent`.
Numeric values are JSON numbers, not strings inside a metadata field.

When the contract declares a location binding and CoreLocation supplies a fresh
fix, `position` contains `latitude`, `longitude`, `altitude`, `speed` and `heading`.
Coordinates are WGS84 degrees. Missing/expired fixes are omitted, and denied
location permission never enables simulation. There are no built-in temperature
or humidity placeholders. Payloads that do not match the contract are rejected
locally and are not published.
