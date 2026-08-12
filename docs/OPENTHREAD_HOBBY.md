# OpenThread Hobby Deployment

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
Extrittio hobby server's Zenoh TCP/TLS listener
```

The nRF52840 dongle is an RCP radio, not a network bridge by itself. OTBR runs
on a host, owns the dongle over USB, and routes between the Thread mesh and the
host's infrastructure network. For a real end-to-end test, use a second Thread
device (or another existing Thread endpoint) in addition to the RCP dongle.
A dongle flashed as the RCP cannot also be the independent telemetry device.

## Run Extrittio with the bundled Thread runtime

The hobby feature includes `extrittio-openthread-runtime`. `extrittio run`
auto-detects exactly one connected RCP, starts its `otbr-agent` child, and
changes the Zenoh listener from its secure loopback default to IPv6. It shuts
OTBR down with Extrittio so stale Thread routes are not left on the host.

```bash
extrittio run --thread-required
```

`--thread-required` turns an absent/ambiguous RCP or unavailable OTBR runtime
into a startup error. Without it, a machine with no RCP continues in Wi-Fi-only
mode. `--thread-enabled false` disables automatic OTBR startup. The RCP is
normally detected as `/dev/cu.usbmodem…` on macOS or a Nordic `/dev/serial/by-id`
device on Linux; specify it explicitly when other USB serial hardware is
attached:

```bash
extrittio run \
  --thread-required \
  --thread-rcp /dev/cu.usbmodem14101 \
  --thread-infra-interface en0
```

For a Cargo development build, point Extrittio to the locally built OTBR agent:

```bash
extrittio run --thread-required \
  --thread-otbr-agent /path/to/ot-br-posix/build/otbr/src/agent/otbr-agent
```

The packaged agent is resolved from `libexec/extrittio/otbr-agent`; the
`EXTRITTIO_OTBR_AGENT` environment variable and `PATH` are fallback locations
for development and custom packaging. OTBR needs the privileges required to
create its Thread interface and configure IPv6 routing. Install the packaged
agent with those privileges, or run the explicitly configured local setup using
your platform's normal privilege mechanism.

Extrittio emits a valid IPv6 listen locator such as `tls/[::]:7447` when
`ZENOH_TLS_ENABLED=true`. A Thread client must use a *routable server address*,
never the wildcard. For example:

```text
tls/[fd12:3456:789a:1::20]:7447
```

Pass that value to `extrittio provision --zenoh-connect ...`, or place it in
the OpenThread client's Zenoh configuration. IPv6 literals require square
brackets in Zenoh locators.

## OpenThread Border Router

### Linux deployment

Run OTBR natively on a Linux host (a Raspberry Pi, mini PC, or the Linux
machine that runs Extrittio). Flash the Nordic dongle with the OpenThread RCP
image for the USB bootloader, connect it to that host, and use a stable
`/dev/serial/by-id/...` path for the radio. The official OpenThread nRF52840
instructions require `-DOT_BOOTLOADER=USB` when producing this firmware.

For a Cargo development build, build the official agent and pass it with
`--thread-otbr-agent`; production hobby packages install that agent beside the
Extrittio executable. The official native setup builds OTBR with the selected
Ethernet or Wi-Fi interface:

```bash
git clone --recursive --depth=1 https://github.com/openthread/ot-br-posix
cd ot-br-posix
./script/bootstrap
INFRA_IF_NAME=eth0 ./script/setup
```

When Extrittio owns the process, it supplies the equivalent arguments directly
and does not install a system service. Inspect the router using the normal OTBR
tools:

```bash
sudo ot-ctl state
sudo ot-ctl ipaddr
sudo ot-ctl netdata show
```

`state` should report a participating Thread role (normally `leader` for a
fresh mesh), and `netdata show` should include an off-mesh-routable (OMR)
prefix. The official OTBR native setup and the current Nordic RCP preparation
guide are the source of truth for host dependencies and the firmware build:
[OTBR native install](https://openthread.io/guides/border-router/build-native),
[nRF52840 OTBR preparation](https://openthread.io/guides/border-router/prepare).

### macOS deployment

Current macOS is supported by the same hobby runtime. Connect the flashed RCP,
then run `extrittio run --thread-required`; `en0` is used as the default
adjacent Wi-Fi/Ethernet interface. Set `--thread-infra-interface` if the Mac
uses a different interface. Docker Desktop remains unsuitable because its VM
does not provide the host serial-device and network-interface behavior OTBR
requires.

## Verify the path

1. Confirm OTBR's OMR prefix and the Thread device's IPv6 address with
   `ot-ctl netdata show` and the device CLI.
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
