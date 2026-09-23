# Extrittio Rust ESP32 client

This standalone crate is the general ESP-IDF client for an ESP32-C6. It
connects over Wi-Fi and Zenoh, publishes validated contract events and
heartbeats, synchronizes desired and reported shadow state, and handles
firmware updates.

The crate is intentionally outside the root Cargo workspace because it uses the
ESP-IDF cross-compilation toolchain.

## Build

Install the Espressif Rust prerequisites, then build from this directory:

```bash
cd clients/rust/esp32
EXTRITTIO_DEVICE_CONTRACT=/absolute/path/to/device-contract.json cargo build --release
```

The default target and ESP-IDF version are declared in `.cargo/config.toml`.
Before building, set Wi-Fi credentials in `src/main.rs` and download the JSON
response from `/api/v1/devices/{id}/contract` for the provisioned device. The
build embeds that response; firmware verifies its hash and stream schema before
connecting. Device identity, endpoint and heartbeat timing come from the contract.
This TCP-only build requires a `tcp/` endpoint; it fails instead of silently
falling back to scouting or downgrading a TLS contract.

Create the device from a published blueprint first. Extrittio rejects messages
from unknown device IDs and does not auto-register heartbeats.

The example emits simulated `temperature`, `humidity`, and `batteryLevel` fields
on the `environment` stream every five seconds. Declare those fields in the
blueprint (the simulator blueprint is a starting point); they are example data,
not platform telemetry fields. Each event is validated and size-checked by the
shared Rust contract codec. No fixed telemetry publisher or no-contract mode
exists. UTC time must synchronize over SNTP before publication starts.

Contract reassignment requires rebuilding/reprovisioning this reference firmware.
It does not yet download and apply replacement contracts at runtime. Move Wi-Fi
credentials and device configuration into secure storage before deployment.
Cross-compilation and real-device memory, reconnect, OTA, and interoperability
tests are required before declaring this client supported for a release.
