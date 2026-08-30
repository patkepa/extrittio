# Extrittio Rust ESP32 client

This standalone crate is the general ESP-IDF client for an ESP32-C6. It
connects over Wi-Fi and Zenoh, publishes compatibility telemetry and
heartbeats, synchronizes desired and reported shadow state, and handles
firmware updates.

The crate is intentionally outside the root Cargo workspace because it uses the
ESP-IDF cross-compilation toolchain.

## Build

Install the Espressif Rust prerequisites, then build from this directory:

```bash
cd clients/rust/esp32
cargo build --release
```

The default target and ESP-IDF version are declared in `.cargo/config.toml`.
Before flashing, replace the development values in `src/main.rs` with the exact
provisioned device ID, Wi-Fi credentials, and reachable Zenoh endpoint.

Create the device from a published blueprint first. Extrittio rejects messages
from unknown device IDs and does not auto-register heartbeats.

The current client uses the compatibility Protobuf topics rather than loading a
materialized device contract. It is a development reference, not a production
provisioning workflow; move credentials and device configuration into secure
device storage before deployment.
