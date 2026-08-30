# Extrittio ESP32-C6 Thread Device

This ESP-IDF example joins an existing Thread mesh with the ESP32-C6's native
IEEE 802.15.4 radio, discovers Extrittio Edge using Thread DNS-SD,
then publishes simulated temperature, humidity, and battery telemetry through
Zenoh over Thread IPv6. It also advertises a read-only Bluetooth LE contact
service so the Extrittio iOS app can identify the device when an iPhone is
brought close. It deliberately uses the normal Extrittio topics and payloads:
Thread is the network transport, not a separate Extrittio device protocol.

This project requires an **ESP32-C6**. An ESP32-S3 has no 802.15.4 radio, so it
cannot run this native-radio example; use an external Thread RCP with an S3
instead.

The example is intentionally a Minimal Thread Device (MTD): it joins an
existing RCP border-router mesh and cannot elect itself leader. If its log
remains detached, place it near the RCP and check the configured dataset rather
than treating an isolated partition as a working connection.

## Prerequisites

- ESP-IDF 6.0 with ESP32-C6 support (the project is compile-verified with this
  version).
- Extrittio Edge running with its OpenThread Border Router enabled
  (`extrittio run --thread-required`).
- The default Extrittio development Thread network, or a network imported in
  **Settings → Thread Network**.
- The default Extrittio Thread DNS-SD advertisement. It is started
  automatically by Extrittio Edge with OTBR; no backend IPv6 address is
  configured on the device.

## Configure and flash

Start Extrittio Edge. On its first successful OTBR start, it creates the
public development network embedded in this example. With OTBR enabled it advertises
`_extrittio-zenoh._tcp.default.service.arpa.` on the Thread mesh and supplies
the current IPv6 address, Zenoh port, and TLS flag. The default
`sdkconfig.defaults` already contains the matching development dataset, then:

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
- `Thread Zenoh locator fallback` — used after three failed DNS-SD discovery
  attempts. The development default targets the bundled Thread network. For a
  custom network, replace it with the `tcp/[fdxx:...]:7447` address from the
  backend's `Advertised Zenoh DNS-SD service on the Thread mesh` log.
- `Thread Active Operational Dataset (hex TLVs)` — leave the supplied
  development value unchanged to join the backend's default network. Replace
  it with the exact exported active dataset whenever the backend uses a custom
  network. This includes the Thread network key; do not commit `sdkconfig`
  after configuring it.

The bundled development network has a public key and is only for a controlled
local test mesh. Create or import a private network in the backend before any
non-test deployment, then replace this ESP32-C6 dataset with that network's
exported Active Operational Dataset.

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

## BLE contact pairing

The firmware advertises `Extrittio ESP32-C6` every 100 ms. In the iOS app, open
**Devices → Pair Nearby Device**, then move the iPhone close to the board. The
app waits for a strong, repeated signal before connecting and reading:

- Extrittio device ID
- hardware model
- firmware version
- primary transport

After identification, the panel can write a UTF-8 contact message of up to 120
bytes. A successful acknowledged write queues three green flashes on both of
the CI ESP32-C6 board's serial SK6805 LEDs, driven from GPIO 9.

The service UUID is `9D8D0001-1A7C-4A21-9F25-E6B17EB89C11`; its four read-only
identity characteristics use the same UUID with `0002` through `0005`. The
writable contact-message characteristic uses `0006`. This first pairing step
intentionally exchanges identity and a user-entered message only. It does not
send Thread credentials, authentication tokens, or other secrets, and it does
not create BLE bonding.
NimBLE and OpenThread share the ESP32-C6 radio through ESP-IDF coexistence.

## Verify connectivity

The log distinguishes the two layers:

1. `Attached to the configured Thread network` proves the ESP32-C6 joined a
   parent on the Edge OTBR's mesh. As an MTD, it cannot form a separate leader
   partition.
2. `Discovered Extrittio Zenoh endpoint` proves Thread DNS-SD found the
   backend's current service record.
3. `Zenoh connected` proves IPv6 routing from that mesh to Extrittio Edge.

If discovery does not find a service, ensure the backend log contains
`Advertised Zenoh DNS-SD service on the Thread mesh` and that OTBR's DNS-SD/SRP
proxy is enabled. See [OpenThread Edge deployment](../../../docs/OPENTHREAD_EDGE.md)
for the host and border-router setup.

## TLS and commands

The default project is intended for Extrittio Edge's local TCP listener. If
the discovered service advertises TLS/mTLS, the example rejects it because the
corresponding Zenoh-pico TLS transport and device certificates are not included
here. The example currently publishes telemetry and heartbeats only; it is a
compact connectivity reference rather than an OTA/command client.
