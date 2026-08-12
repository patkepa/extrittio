# Extrittio ESP32-C6 Thread Device

This ESP-IDF example joins an existing Thread mesh with the ESP32-C6's native
IEEE 802.15.4 radio, discovers the Extrittio hobby server using Thread DNS-SD,
then publishes simulated temperature, humidity, and battery telemetry through
Zenoh over Thread IPv6. It deliberately uses the normal Extrittio topics and
payloads: Thread is the network transport, not a separate Extrittio device
protocol.

This project requires an **ESP32-C6**. An ESP32-S3 has no 802.15.4 radio, so it
cannot run this native-radio example; use an external Thread RCP with an S3
instead.

## Prerequisites

- ESP-IDF 6.0 with ESP32-C6 support (the project is compile-verified with this
  version).
- An Extrittio hobby server running with its OpenThread Border Router enabled
  (`extrittio run --thread-required`).
- A formed/imported Thread network in **Settings → Thread Network**.
- The default Extrittio Thread DNS-SD advertisement. It is started
  automatically by the hobby server with OTBR; no backend IPv6 address is
  configured on the device.

## Configure and flash

Start the hobby server. With OTBR enabled it advertises
`_extrittio-zenoh._tcp.default.service.arpa.` on the Thread mesh and supplies
the current IPv6 address, Zenoh port, and TLS flag. Copy the Active Operational
Dataset TLVs from OTBR after creating/importing the same network, then:

```bash
cd clients/c/esp32c6-openthread-idf-c
idf.py set-target esp32c6
idf.py menuconfig
idf.py build flash monitor
```

Under `Extrittio Thread Device`, set:

- `Device ID` — a new device ID, for example `esp32c6-thread-001`.
- `Thread DNS-SD Zenoh service` — keep the default unless the server uses a
  custom Thread DNS-SD service name.
- `Thread Active Operational Dataset (hex TLVs)` — the exact exported active
  dataset hex value. This includes the Thread network key; do not commit
  `sdkconfig` after configuring it.

Create the matching Extrittio device before flashing. A generic device type is
enough for this telemetry example:

```bash
cargo run -p extrittio -- provision \
  --name esp32c6-thread-001 \
  --device-type esp32c6-thread \
  --firmware v1.0.0-esp32c6-thread-c
```

Use the same device ID in `menuconfig`. After the log reports `Attached to the
configured Thread network`, `Discovered Extrittio Zenoh endpoint`, and `Zenoh
connected`, telemetry and heartbeats appear in the Extrittio device view.

## Verify connectivity

The log distinguishes the two layers:

1. `Attached to the configured Thread network` proves the supplied dataset
   joined the same mesh as the hobby OTBR.
2. `Discovered Extrittio Zenoh endpoint` proves Thread DNS-SD found the
   backend's current service record.
3. `Zenoh connected` proves IPv6 routing from that mesh to the hobby server.

If discovery does not find a service, ensure the backend log contains
`Advertised Zenoh DNS-SD service on the Thread mesh` and that OTBR's DNS-SD/SRP
proxy is enabled. See [OpenThread hobby deployment](../../../docs/OPENTHREAD_HOBBY.md)
for the host and border-router setup.

## TLS and commands

The default project is intended for the hobby server's local TCP listener. If
the discovered service advertises TLS/mTLS, the example rejects it because the
corresponding Zenoh-pico TLS transport and device certificates are not included
here. The example currently publishes telemetry and heartbeats only; it is a
compact connectivity reference rather than an OTA/command client.
