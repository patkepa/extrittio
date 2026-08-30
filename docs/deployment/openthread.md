# OpenThread

Extrittio uses Thread as IPv6 networking, not as a second application protocol.
A Thread client reaches the normal Zenoh listener through an OpenThread Border
Router (OTBR) and publishes the same Extrittio topics as a Wi-Fi client.

```text
Thread device with a Zenoh client
  -> IEEE 802.15.4 / Thread IPv6
  -> radio co-processor (RCP) + host OTBR
  -> host IPv6 routing
  -> Extrittio Zenoh TCP/TLS listener
```

An RCP dongle is only the border router's radio. It cannot simultaneously act
as the independent telemetry device used to test the mesh.

## Supported Edge path

The Edge feature includes OTBR supervision but not every installation artifact
contains an agent. `cargo xtask install edge --release` and
`cargo xtask package edge-linux-arm64` build the pinned source, apply Extrittio's
D-Bus and loopback REST-authentication patches, and install the matching agent
beside the application. The Debian Edge package contains only the application
and can use a separately installed compatible agent.

At startup, `extrittio run`:

1. discovers a single supported RCP unless a path was configured;
2. resolves a compatible `otbr-agent`;
3. starts an isolated D-Bus daemon and the agent;
4. enables a Zenoh IPv6 listener;
5. advertises the concrete Thread-side Zenoh endpoint with DNS-SD;
6. supervises and stops the child processes with Extrittio.

Without `--thread-required`, missing or ambiguous hardware leaves the hub in
Wi-Fi-only mode and discovery is retried. Useful overrides are:

```bash
extrittio run \
  --thread-required \
  --thread-rcp /dev/serial/by-id/<radio> \
  --thread-infra-interface eth0
```

Use `--thread-enabled false` to disable supervision. On macOS the RCP normally
appears under `/dev/cu.usbmodem...`; on Linux use the stable
`/dev/serial/by-id/...` path. OTBR needs permission to create its Thread
interface and configure IPv6 routes, which is why the packaged systemd service
runs with elevated privileges.

For a repository development build:

```bash
cargo xtask otbr build
cargo build --release -p extrittio --no-default-features --features edge
sudo target/release/extrittio run --thread-required
```

The runtime discovers the repository agent under
`target/openthread/build/src/agent/otbr-agent`. `dbus-daemon` must be available
on the host.

## Thread network

When a ready RCP has no Active Operational Dataset, Edge forms the bundled
`extrittio-c6-dev` network by default. Its credentials are committed for local
development and must not be used for a private or production mesh. Disable this
behavior with:

```bash
extrittio run --thread-seed-default-dataset false
```

The **Mesh Network → OpenThread Settings** screen can select a radio, inspect
status, form a new network, or import an Active Operational Dataset. Selecting
a different dataset disconnects devices on the old mesh. Dataset export
contains network credentials, is owner-restricted, and is returned with
no-store cache headers.

## DNS-SD and addressing

After OTBR is ready, Extrittio advertises:

```text
extrittio-backend._extrittio-zenoh._tcp.default.service.arpa.
```

The target is a concrete, non-loopback IPv6 address on the Thread path and the
port is the active Zenoh listener port. The TXT record identifies Extrittio,
Zenoh, protocol version 1, and whether TLS is enabled. The service name and
instance can be overridden with:

```bash
EXTRITTIO_THREAD_ZENOH_SERVICE_NAME=_extrittio-zenoh._tcp
EXTRITTIO_THREAD_ZENOH_SERVICE_INSTANCE=extrittio-backend
```

Thread clients should use DNS-SD or a routable bracketed IPv6 locator such as:

```text
tls/[fd12:3456:789a:1::20]:7447
```

Never configure a client with the wildcard listener address `[::]`. Many
devices can share the DNS-SD record because their provisioned identities and
device-specific Zenoh topics isolate their traffic.

Plain TCP is suitable only for a controlled development mesh. Production
devices should use the TLS/mTLS listener and their provisioned Extrittio client
certificate. The ESP32-C6 reference client currently supports the plain-TCP
development path only.

## Verify

1. Confirm that Edge reports a ready OTBR and an active Thread role.
2. Confirm the RCP and test device use the same Active Operational Dataset.
3. Resolve the Extrittio DNS-SD service from the Thread device, or obtain the
   host's concrete Thread IPv6 address from the OpenThread settings screen.
4. Test TCP reachability to port 7447 before starting Zenoh.
5. Create the device from a published blueprint and use the exact provisioned
   device ID.
6. Start the device and confirm that heartbeat or contract events appear in the
   device view.

If the Linux host does not install OTBR-advertised routes, inspect IPv6 router
advertisement acceptance on the infrastructure interface. For radio firmware
and host networking prerequisites, use the upstream
[OTBR native build guide](https://openthread.io/guides/border-router/build-native)
and [nRF52840 RCP preparation guide](https://openthread.io/guides/border-router/prepare).
