# Extrittio Zephyr client

This client targets Zephyr 4.4.1 on the nRF52840 DK. It provisions over BLE,
joins a Thread network from an active operational dataset, and uses Zenoh over
mutually authenticated TLS for the normal Extrittio device lifecycle.

## Build

Create a west workspace from this repository's nested manifest, then install
Zephyr's Python and toolchain prerequisites:

```sh
west init \
  -m https://github.com/patkepa/extrittio.git \
  --mf clients/zephyr/west.yml \
  zephyr-workspace
cd zephyr-workspace
west update
west zephyr-export
west build --sysbuild -b nrf52840dk/nrf52840 extrittio/clients/zephyr
```

The manifest pins Zephyr, zenoh-pico, and cJSON. The default nRF52840 layout
contains MCUboot primary and secondary slots, two bootstrap partitions for
atomic provisioning updates, and an NVS settings partition. Development builds
generate an unsigned MCUboot image; production releases must configure an
MCUboot signing key and distribute the corresponding signed image.

## Provisioning

An unprovisioned device advertises the Extrittio BLE service for the configured
provisioning window. A provisioned device opens the window only when the DK's
`sw0` button is held during boot. The capabilities characteristic advertises
bootstrap protocol versions 3 and 4; current iOS clients negotiate version 4
and older clients continue to use version 3.

The accepted bootstrap contains the Thread active dataset, tenant and device
identity, device contract, Zenoh endpoint, CA certificate, client certificate,
and private key. The client validates the complete package before atomically
committing it to the inactive bootstrap partition.

## Communication and OTA

The runtime joins Thread and connects to the provisioned Zenoh endpoint using
the device certificate and private key. It shares Extrittio's standard topic,
heartbeat, telemetry, command, shadow, and OTA behavior through the portable C
SDK.

OTA downloads require an HTTPS URL whose server certificate chains to the CA
in the device bootstrap. The client streams the image into the MCUboot
secondary slot, verifies the required SHA-256 digest, requests a test upgrade,
and confirms the image after a successful boot.

Fresh Extrittio installations issue P-256 CA and leaf certificates for Zephyr
compatibility. Installations that already use an Ed25519 CA need an explicit CA
rotation before provisioning these devices.
