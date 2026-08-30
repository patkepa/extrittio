# OpenThread Extrittio Edge Deployment

Extrittio accepts Thread-connected devices without a separate backend protocol:
Thread supplies IPv6 networking and Zenoh carries the existing Extrittio device
topics over TCP/TLS. A Thread device therefore publishes the same telemetry,
heartbeats, shadows, logs, and command responses as a Wi-Fi device.

## Architecture

```text
Thread device running a Zenoh client
          | IEEE 802.15.4 / Thread IPv6
          v
nRF52840 USB Dongle in RCP firmware + OpenThread Border Router (OTBR)
          | IPv6 routing over Ethernet or Wi-Fi
          v
Extrittio Edge server's Zenoh TCP/TLS listener
```

The nRF52840 dongle is an RCP radio, not a network bridge by itself. OTBR runs
on a host, owns the dongle over USB, and routes between the Thread mesh and the
host's infrastructure network. For a real end-to-end test, use a second Thread
device (or another existing Thread endpoint) in addition to the RCP dongle.
A dongle flashed as the RCP cannot also be the independent telemetry device.

## Run Extrittio with the bundled Thread runtime

The Edge feature includes `extrittio-openthread-runtime`. `extrittio run`
auto-detects exactly one connected RCP, starts its `otbr-agent` child, and
changes the Zenoh listener from its secure loopback default to IPv6. It shuts
OTBR down with Extrittio so stale Thread routes are not left on the host.

On a ready RCP with no Active Operational Dataset, `extrittio run` also forms
the bundled **`extrittio-c6-dev`** development mesh. Its fixed credentials are
intended only for local development and are shared with the ESP32-C6 example;
do not use it for a private or production deployment. The startup path never
replaces an existing dataset. Pass `--thread-seed-default-dataset false` to
leave an empty RCP ready for explicit provisioning instead.

The **OpenThread Settings** panel selects a radio when multiple RCP candidates
are connected and uses Extrittio's private, typed D-Bus control channel to show
mesh status, form a new network, or import an existing Active Operational
Dataset. The radio choice is stored in the Edge data directory and reused on
later starts. Creating or importing a dataset replaces the current mesh and
disconnects existing Thread devices. Extrittio does not store Thread
credentials in its database or logs. The explicit dataset export route can
return them with `Cache-Control: no-store` and is restricted to the appliance
owner.

Build the default Extrittio Edge installation from the repository root. It installs
`extrittio` and the pinned upstream `otbr-agent` together under the Cargo
installation root, so no separate OTBR installation is needed:

```bash
cargo xtask install edge --release
```

Omit `--release` for a debug build. Override
`EXTRITTIO_INSTALL_ROOT=/path` when a different installation prefix is needed.

```bash
extrittio run
```

Use `--thread-required` to turn an absent/ambiguous RCP or unavailable OTBR
runtime into a startup error. Without it, a machine with no RCP continues in
Wi-Fi-only mode. `--thread-enabled false` disables automatic OTBR startup. The RCP is
normally detected as `/dev/cu.usbmodem…` on macOS or a Nordic `/dev/serial/by-id`
device on Linux. Choose it in OpenThread Settings when other USB serial
hardware makes auto-discovery ambiguous. The CLI options remain available as
one-run overrides, including for unattended setup:

```bash
extrittio run \
  --thread-required \
  --thread-rcp /dev/cu.usbmodem14101 \
  --thread-infra-interface en0
```

For a direct Cargo development build, build the pinned OTBR agent once. Edge
binaries under `target/debug` or `target/release` discover it automatically:

```bash
cargo xtask otbr build
cargo build --release -p extrittio --no-default-features --features edge
sudo target/release/extrittio run
```

The packaged agent is resolved from `libexec/extrittio/otbr-agent`; the
repository build is resolved from `target/openthread/build/src/agent/otbr-agent`.
The `EXTRITTIO_OTBR_AGENT` environment variable, explicit CLI option, and
`PATH` remain overrides for custom packaging. OTBR needs the privileges
required to create its Thread interface and configure IPv6 routing. Install the
packaged agent with those privileges, or run the local setup using your
platform's normal privilege mechanism.

The control panel starts an isolated local D-Bus daemon for OTBR and stops it
with Extrittio. Ensure `dbus-daemon` is installed and available on `PATH`
(`dbus` on common Linux distributions; `brew install dbus` on macOS) when
building a local development environment.

Extrittio emits a valid IPv6 listen locator such as `tls/[::]:7447` when
`ZENOH_TLS_ENABLED=true`. A Thread client must use a *routable server address*,
never the wildcard. For example:

```text
tls/[fd12:3456:789a:1::20]:7447
```

Pass that value to `extrittio provision --zenoh-connect ...`, or place it in
the OpenThread client's Zenoh configuration. IPv6 literals require square
brackets in Zenoh locators.

## Zenoh DNS-SD discovery on Thread

When the local OTBR runtime is ready, Extrittio publishes exactly one DNS-SD
service. OTBR's DNS-SD discovery proxy translates its local mDNS registration
onto the Thread service domain, so Thread clients discover:

```text
extrittio-backend._extrittio-zenoh._tcp.default.service.arpa.
```

Its target is a concrete, non-loopback IPv6 address assigned to OTBR's Thread
interface, and its port is the active `ZENOH_TLS_PORT` (normally `7447`). It
never publishes the Zenoh wildcard `::` or an IPv4-only infrastructure address.
The TXT record is `role=server`, `proto=zenoh`, `version=1`, and `tls=1` when
Zenoh TLS is enabled (`tls=0` otherwise).

The safe defaults can be changed without altering device traffic:

```bash
EXTRITTIO_THREAD_ZENOH_SERVICE_NAME=_extrittio-zenoh._tcp
EXTRITTIO_THREAD_ZENOH_SERVICE_INSTANCE=extrittio-backend
```

Extrittio registers the service only after OTBR is available, removes it during
backend shutdown, and re-registers it after OTBR recovery or a Thread dataset
replacement. The bundled OTBR build enables OpenThread's DNS-SD discovery and
SRP advertising proxies; a custom OTBR build must enable equivalent DNS-SD/SRP
proxy support. The mesh also needs OTBR's normal SRP server service and IPv6
routing between Thread clients and the OTBR host.

Many Thread devices discover this same backend record. They remain isolated by
their existing device-specific Zenoh keys—such as
`extrittio/devices/<device-id>/heartbeat` and
`extrittio/devices/<device-id>/telemetry`—not by separate DNS-SD records.
Devices do not advertise their own DNS-SD services. Topic validation and
provisioned-device enforcement remain unchanged: an administrator must create
each device from a published blueprint before it can send traffic.

Plain TCP (`tls=0`) is for controlled test environments only. Production
deployments should use the default TLS/mTLS listener and provision each device
with its Extrittio client certificate.

## OpenThread Border Router

### Linux deployment

Run OTBR natively on a Linux host (a Raspberry Pi, mini PC, or the Linux
machine that runs Extrittio). Flash the Nordic dongle with the OpenThread RCP
image for the USB bootloader, connect it to that host, and use a stable
`/dev/serial/by-id/...` path for the radio. The official OpenThread nRF52840
instructions require `-DOT_BOOTLOADER=USB` when producing this firmware.

For a Cargo development build, run `cargo xtask otbr build` from this repository.
Repository Edge binaries discover
`target/openthread/build/src/agent/otbr-agent` automatically. This applies
Extrittio's D-Bus contract and mandatory loopback REST authentication patches;
the runtime rejects an unpatched agent.
Production Extrittio Edge packages install the patched agent beside the Extrittio
executable. The official native setup builds OTBR with the selected Ethernet
or Wi-Fi interface:

```bash
git clone --recursive --depth=1 https://github.com/openthread/ot-br-posix
cd ot-br-posix
./script/bootstrap
INFRA_IF_NAME=eth0 ./script/setup
```

When Extrittio owns the process, it supplies the equivalent arguments directly
and does not install a system service. Inspect the router from **Settings →
Thread Network** or the owner-authenticated `/api/v1/system/thread` endpoints.
The status should report a participating Thread role (normally `leader` for a
fresh mesh). The official OTBR native setup and the current Nordic RCP preparation
guide are the source of truth for host dependencies and the firmware build:
[OTBR native install](https://openthread.io/guides/border-router/build-native),
[nRF52840 OTBR preparation](https://openthread.io/guides/border-router/prepare).

### macOS deployment

Current macOS is supported by the same Extrittio Edge runtime. Connect the flashed RCP,
then run `extrittio run --thread-required`; the host's default-route interface
is selected automatically. Set `--thread-infra-interface` to override it.
Docker Desktop remains unsuitable because its VM
does not provide the host serial-device and network-interface behavior OTBR
requires.

## Verify the path

1. Confirm the mesh-local prefix and host Thread IPv6 address in **Settings →
   Thread Network**, then confirm the Thread device's address with its CLI.
2. From the Thread device, test TCP reachability to the Extrittio server's IPv6
   address on port `7447` before starting Zenoh.
3. Provision the device with the bracketed IPv6 locator and its Extrittio
   client certificate when TLS is enabled.
4. Start the device. Its normal heartbeat and telemetry should appear in the
   Extrittio device view; no Thread-specific topic mapping is needed.

If a Linux host does not install routes advertised by OTBR, check that IPv6
router advertisements and route information options are accepted on its
infrastructure interface. The OpenThread border-router codelab documents the
required Linux `accept_ra` and `accept_ra_rt_info_max_plen` settings:
[IPv6 host verification](https://openthread.io/codelabs/openthread-border-router).
