# Debian Extrittio Edge package deployment

This package is for a 64-bit Raspberry Pi OS or Debian host. It contains one
executable: the Turso-backed Extrittio Edge build with the operations UI embedded.

It does **not** include OpenThread Border Router (`otbr-agent`), an RCP, or
their system dependencies. At startup, Extrittio searches for an RCP and an
executable `otbr-agent`. If both are present, it supervises the agent; if
either is absent, it continues as a Wi-Fi/cloud-only hub and retries later.
Use the full Raspberry Pi Extrittio Edge package when you want Extrittio to bring its
own pinned OTBR binary.

## Build

On a development machine with Docker Buildx:

```bash
cargo xtask package edge-deb-arm64
```

This creates a versioned Debian package and checksum in `dist/debian/`. Supply
a release version with `EXTRITTIO_PACKAGE_VERSION=1.2.3` or `--version 1.2.3`.

## Install and run

Copy the package to a 64-bit Raspberry Pi OS or Debian machine, then install
it locally:

```bash
sudo apt install ./extrittio_<version>_arm64.deb
sudoedit /etc/extrittio/extrittio.env
sudo systemctl enable --now extrittio.service
```

Set `EXTRITTIO_PUBLIC_URL` to the Pi's stable LAN URL. Set a strong
`EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD` before the first start; otherwise the
application's development default is used. The package never overwrites this
configuration on upgrade.

The systemd service runs as root only because a discovered OTBR agent must
create `wpan0` and configure IPv6 routes. Persistent data lives in
`/var/lib/extrittio`, including its local database, certificates, and firmware.
It survives package upgrades and removal.

## Optional local OpenThread

Install and configure OTBR and an RCP using your operating system's normal
procedure. Ensure the `otbr-agent` executable is in the system PATH (for
example `/usr/sbin/otbr-agent`), or set `EXTRITTIO_OTBR_AGENT` in
`/etc/extrittio/extrittio.env`. Set `EXTRITTIO_THREAD_RCP` to a stable
`/dev/serial/by-id/...` path. Leave `EXTRITTIO_THREAD_REQUIRED=false` unless
you want the entire service to fail when Thread is unavailable.

To upgrade, install a newer local package with `apt install`. To inspect logs:

```bash
sudo journalctl -u extrittio.service -e
```
