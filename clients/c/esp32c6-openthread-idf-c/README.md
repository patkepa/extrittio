# Extrittio ESP32-C6 Thread Device

This ESP-IDF example joins an existing Thread mesh with the ESP32-C6's native
IEEE 802.15.4 radio, then publishes simulated temperature, humidity, and
battery telemetry to an Extrittio hobby server through Zenoh over Thread IPv6.
It deliberately uses the normal Extrittio topics and payloads: Thread is the
network transport, not a separate Extrittio device protocol.

This project requires an **ESP32-C6**. An ESP32-S3 has no 802.15.4 radio, so it
cannot run this native-radio example; use an external Thread RCP with an S3
instead.

## Prerequisites

- ESP-IDF 6.0 with ESP32-C6 support (the project is compile-verified with this
  version).
- An Extrittio hobby server running with its OpenThread Border Router enabled
  (`extrittio run --thread-required`).
- A formed/imported Thread network in **Settings → Thread Network**.
- The hobby host's routable Thread IPv6 address. Do not use `::`, which is only
  a listener wildcard.

## Configure and flash

Start the hobby server and note its Thread IPv6 address. It must listen on
IPv6 (the hobby runtime does this automatically when Thread is enabled). For a
non-TLS local setup, the Zenoh locator looks like:

```text
tcp/[fd12:3456:789a:1::20]:7447
```

Copy the Active Operational Dataset TLVs from the Thread Network panel, then:

```bash
cd clients/c/esp32c6-openthread-idf-c
idf.py set-target esp32c6
idf.py menuconfig
idf.py build flash monitor
```

Under `Extrittio Thread Device`, set:

- `Device ID` — a new device ID, for example `esp32c6-thread-001`.
- `Zenoh IPv6 endpoint` — the bracketed IPv6 locator above.
- `Thread Active Operational Dataset (hex TLVs)` — the exact exported active
  dataset hex value. This includes the Thread network key; do not commit
  `sdkconfig` after configuring it.

Create the matching Extrittio device before flashing. A generic device type is
enough for this telemetry example:

```bash
cargo run -p extrittio -- provision \
  --name esp32c6-thread-001 \
  --device-type esp32c6-thread \
  --firmware v1.0.0-esp32c6-thread-c \
  --zenoh-connect 'tcp/[fd12:3456:789a:1::20]:7447'
```

Use the same device ID and Zenoh locator in `menuconfig`. After the log reports
`Attached to the configured Thread network` and `Zenoh connected`, telemetry
and heartbeats appear in the Extrittio device view.

## Verify connectivity

The log distinguishes the two layers:

1. `Attached to the configured Thread network` proves the supplied dataset
   joined the same mesh as the hobby OTBR.
2. `Zenoh connected` proves IPv6 routing from that mesh to the hobby server.

If Thread attaches but Zenoh cannot connect, confirm the server's address is a
routable Thread/OMR IPv6 address and that its Zenoh listener is on port 7447.
See [OpenThread hobby deployment](../../../docs/OPENTHREAD_HOBBY.md) for the
host and border-router setup.

## TLS and commands

The default project is intended for the hobby server's local TCP listener.
If TLS/mTLS is enabled on the server, this example needs the corresponding
Zenoh-pico TLS transport and device certificates; that configuration is not
included here. The example currently publishes telemetry and heartbeats only;
it is a compact connectivity reference rather than an OTA/command client.
