# Raspberry Pi Extrittio Edge Deployment

Build the complete appliance on an Apple Silicon Mac, then install it on a
64-bit Raspberry Pi OS host over SSH. The Pi never compiles Rust, frontend, or
OpenThread code. It only runs the Linux arm64 `extrittio` executable and its
matching `otbr-agent`.

This is intentionally separate from `cargo xtask install edge --release`: that command
builds for the host it runs on, so invoking it on macOS produces macOS binaries
and cannot be deployed to a Linux Pi.

## One-time Pi setup

Use a 64-bit Raspberry Pi OS or Debian arm64 host. Releases are built against
Debian Bookworm's glibc and libraries and can run on a newer compatible Debian
userspace, including the currently shipped Raspberry Pi Debian Trixie image.
Confirm the architecture before proceeding:

```bash
uname -m # must print aarch64
```

After configuring SSH key access from the Mac, run the safe Mac-side setup
task from the repository root:

```bash
cargo xtask edge setup-pi
```

It verifies the arm64 host and passwordless `sudo`, installs the OTBR runtime
libraries, installs the systemd service, and creates `/etc/extrittio/edge.env`
only if it does not already exist. Override its default Pi connection details
when necessary:

```bash
cargo xtask edge setup-pi \
  --host 192.0.2.10 \
  --user pi \
  --identity ~/.ssh/extrittio-pi
```

It intentionally leaves the service stopped. Configure the generated
`/etc/extrittio/edge.env` before starting it. For a manual setup instead, copy
the repository (or only the listed templates and bootstrap script) to the Pi
and run:

```bash
sudo ./scripts/bootstrap-edge-pi.sh
sudo install -m 0644 deploy/raspberry-pi/extrittio.service /etc/systemd/system/extrittio.service
sudo install -m 0600 deploy/raspberry-pi/edge.env.example /etc/extrittio/edge.env
sudoedit /etc/extrittio/edge.env
sudo systemctl daemon-reload
sudo systemctl enable extrittio.service
```

For an existing installation, run `cargo xtask edge setup-pi` once after
updating the repository. It renames the previous environment file to
`/etc/extrittio/edge.env` before installing the renamed service configuration.

Set `EXTRITTIO_THREAD_RCP` to the stable result of
`ls -l /dev/serial/by-id/`, and select the Pi's actual infrastructure interface
(normally `eth0`). The RCP must remain physically attached to the Pi. The
service starts as root because OTBR creates `wpan0` and configures IPv6 routes;
Extrittio itself supervises the agent and private D-Bus daemon. Do not install
a separate OTBR system service.

`/var/lib/extrittio` is the appliance's persistent state: its Turso database,
certificates, firmware, backups, and Thread state survive every release.

## Build on the Mac

Docker Desktop must be running:

```bash
cargo xtask package edge-linux-arm64
```

The packaging task runs a `linux/arm64` Bookworm Buildx build, runs a Turso
migration smoke test inside it, rejects a PostgreSQL-linked Extrittio Edge executable,
and verifies the OTBR agent has no missing linked libraries. It writes:

```text
dist/raspberry-pi/
├── extrittio-edge-linux-arm64-<git-sha>.tar.gz
└── extrittio-edge-linux-arm64-<git-sha>.tar.gz.sha256
```

The archive includes the following relative layout, required for automatic
OTBR discovery:

```text
bin/extrittio
libexec/extrittio/otbr-agent
```

## Deploy and roll back

From the Mac:

```bash
cargo xtask edge deploy-pi pi@extrittio-pi.local \
  dist/raspberry-pi/extrittio-edge-linux-arm64-<git-sha>.tar.gz
```

The deployment checks the upload's SHA-256, validates the two embedded binary
digests, refuses missing OTBR runtime libraries, installs a new immutable
release under `/opt/extrittio/releases/`, and atomically replaces
`/opt/extrittio/current`. It then restarts systemd and checks `/health`. It
never changes `/var/lib/extrittio`.

To roll back, select any prior installed release and restart:

```bash
sudo ln -s releases/extrittio-edge-linux-arm64-<previous-git-sha> /opt/extrittio/.current.new
sudo mv -Tf /opt/extrittio/.current.new /opt/extrittio/current
sudo systemctl restart extrittio.service
```

If a release does not become healthy, the deploy command leaves it selected for
diagnosis. Inspect `sudo journalctl -u extrittio.service -e` and
`/var/lib/extrittio/thread/otbr-agent.log`; roll back explicitly after fixing
the configuration.

## Network details

`EXTRITTIO_PUBLIC_URL` must be a stable address reachable by browsers and
devices fetching OTA files; it must not be `localhost`. Thread devices use the
routable, bracketed IPv6 Zenoh locator advertised by the Pi's Thread interface,
not the LAN HTTP address. See [OpenThread Edge Deployment](OPENTHREAD_EDGE.md)
for RCP firmware, Thread provisioning, DNS-SD, and IPv6 path verification.
