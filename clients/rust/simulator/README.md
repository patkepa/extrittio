# Extrittio Device Simulator

Simulates many logical devices over the legacy Zenoh Protobuf heartbeat and
telemetry topics. Every generated ID must already exist in Extrittio; the
backend rejects traffic from unprovisioned devices and does not auto-register
heartbeats. Create the devices from a published blueprint before starting the
simulator, using IDs such as `sim-device-000001` through the selected `--count`.

```bash
cargo run -p extrittio-simulator -- \
  --count 500 \
  --scenario mobile \
  --location-area poland \
  --connect tcp/127.0.0.1:7447 \
  --telemetry-interval 5 \
  --heartbeat-interval 30 \
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
