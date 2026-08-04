# Extrittio Device Simulator

Simulates many logical Extrittio devices over the same Zenoh protobuf topics used by real clients. Each simulated device sends an initial heartbeat before telemetry, so the backend can auto-register it through the normal heartbeat path.

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
