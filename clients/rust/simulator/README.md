# Extrittio Device Simulator

Simulates many logical devices using validated contract events and platform
heartbeats. Publish the [simulator blueprint](../../../blueprints/simulator.json),
then create devices from its published revision using IDs such as
`sim-device-000001` through the selected `--count`. Download each device's
`GET /api/v1/devices/{id}/contract` response into a directory as `<device-id>.json`.
The simulator validates every contract's hash, identity and scenario schema
before opening any network session. It does not auto-register devices.

```bash
cargo run -p extrittio-simulator -- \
  --count 500 \
  --contracts-dir ./simulator-contracts \
  --scenario mobile \
  --location-area poland \
  --connect tcp/127.0.0.1:7447 \
  --telemetry-interval 5 \
  --jitter-percent 35 \
  --device-interval-variance-percent 20 \
  --sensor-noise-percent 8 \
  --stats-interval 5
```

For a TLS listener, also pass `--ca-cert`, `--client-cert`, and `--client-key`.
The simulator can share one Zenoh session for throughput tests or create one
session per device for connection-pressure tests.

Useful options:

```bash
--scenario normal|hot|low-battery|mobile|flaky|burst
--session-mode shared|per-device
--duration 10m
--prefix sim-device
--startup-spread-ms 5000
```

Notes:

- `--jitter-percent` randomizes every send interval by `+/- percent`.
- `--device-interval-variance-percent` gives every simulated device its own persistent cadence variance.
- `--sensor-noise-percent` adds measurement noise to emitted telemetry values.
- `--stats-interval` controls how often aggregate send counts and rates are logged.
- Use `shared` session mode for cheap high-count simulations, and `per-device` when testing Zenoh connection/session pressure.
- Heartbeat periods come from each device contract; there is no heartbeat-period override.
- `--stream` selects the declared stream (default `environment`). Scenario payloads
  contain `temperature`, `humidity` and `batteryLevel`; mobile scenarios additionally
  include a `position` object. These fields belong to the sample blueprint.
- Without `--connect`, endpoints come from contracts. Shared sessions require a
  common endpoint; per-device sessions can use distinct contract endpoints.
