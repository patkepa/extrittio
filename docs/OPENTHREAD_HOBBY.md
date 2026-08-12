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

## Run Extrittio with an IPv6 Zenoh listener

The default Zenoh listener is `127.0.0.1`, which deliberately does not expose
the device port. Enable IPv6 explicitly for a Thread deployment:

```bash
ZENOH_TLS_ENABLED=true extrittio run --zenoh-listen-host ::
```

`::` is emitted as the valid Zenoh locator `tls/[::]:7447` (or
`tcp/[::]:7447` when TLS is disabled). Extrittio's usual mTLS certificate and
topic ACL protections continue to apply. Do not expose an unauthenticated TCP
listener outside an isolated development network.

For `extrittio serve`, use the equivalent setting:

```bash
extrittio serve --zenoh-listen-host ::
```

or set `ZENOH_LISTEN_HOST=::`. A Thread client must use a *routable server
address*, never the listener wildcard. For example, after choosing the server's
IPv6 address:

```text
tls/[fd12:3456:789a:1::20]:7447
```

Pass that value to `extrittio provision --zenoh-connect ...`, or place it in
the OpenThread client's Zenoh configuration. IPv6 literals require square
brackets in Zenoh locators.

## OpenThread Border Router

### Recommended Linux deployment

Run OTBR natively on a Linux host (a Raspberry Pi, mini PC, or the Linux
machine that runs Extrittio). Flash the Nordic dongle with the OpenThread RCP
image for the USB bootloader, connect it to that host, and use a stable
`/dev/serial/by-id/...` path for the radio. The official OpenThread nRF52840
instructions require `-DOT_BOOTLOADER=USB` when producing this firmware.

Install and start OTBR using its native setup, selecting the Ethernet or Wi-Fi
interface that reaches Extrittio:

```bash
git clone --recursive --depth=1 https://github.com/openthread/ot-br-posix
cd ot-br-posix
./script/bootstrap
INFRA_IF_NAME=eth0 ./script/setup
```

Set OTBR's radio URL in `/etc/default/otbr-agent` to the stable serial device,
for example:

```text
OTBR_AGENT_OPTS="-I wpan0 -B eth0 spinel+hdlc+uart:///dev/serial/by-id/<nrf52840>?uart-baudrate=460800"
```

Then restart and inspect the service:

```bash
sudo systemctl restart otbr-agent
sudo systemctl status otbr-agent
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

The supported OTBR native installation is Linux-oriented: it installs a system
service and configures host IPv6 forwarding, interfaces, and firewall rules.
Docker Desktop on macOS is not a substitute because its VM does not provide the
host networking and serial-device behavior OTBR needs.

For a Mac running `extrittio run`, place the RCP dongle and OTBR on a Linux host
on the same Ethernet/Wi-Fi network. OTBR advertises the Thread routes on that
infrastructure network, allowing Thread devices to reach the Mac's IPv6 Zenoh
listener. Running Extrittio and OTBR together on the Linux host is the simplest
first setup; the Extrittio hobby binary itself remains portable to macOS and
Linux.

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
