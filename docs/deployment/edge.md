# Edge deployment

Refactor-specific migration ordering and rollback limits are documented in
[Backend ownership and upgrade notes](backend-refactor.md).

Extrittio Edge is the single-node runtime selected by `extrittio run`. It uses a
local Turso database, embeds the built React UI, and stores all mutable state in
one data directory. PostgreSQL remains the production server and HA backend.

Turso Edge is local-only: one process owns a data directory, and a second
process fails fast. Production deployment profiles reject Turso rather than
silently falling back to another database.

## Installation choices

| Workflow | Artifact | OpenThread Border Router |
| --- | --- | --- |
| `cargo install ... --features edge` | Host-native executable | Not bundled; a compatible local agent can be discovered |
| `cargo xtask install edge --release` | Host-native executable plus `libexec` files | Builds and installs the pinned patched agent |
| `cargo xtask package edge-deb-arm64` | Debian arm64 package | Not bundled |
| `cargo xtask package edge-linux-arm64` | Raspberry Pi arm64 release archive | Bundled pinned agent |

Use the Debian package for a conventional package-managed installation. Use the
Raspberry Pi archive when the appliance must bring its matching OTBR binary and
support atomic release switching.

## Local install and first run

Build the frontend before compiling the embedded-UI feature:

```bash
cd apps/frontend
npm ci
npm run build
cd ../..

cargo install --path apps/extrittio --locked \
  --no-default-features --features edge
```

Choose an explicit durable data directory and bootstrap password:

```bash
EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD='choose-a-strong-password' \
  extrittio run --data-dir /srv/extrittio
```

The data directory contains the database and lock, certificates, local firmware
objects, backups, and Edge/OpenThread state. Do not place it inside a release
directory.

`extrittio run` enables Thread supervision by default but continues in
Wi-Fi-only mode when it cannot find both an RCP and a compatible `otbr-agent`.
Use `--thread-required` when Thread availability is mandatory. See
[OpenThread](openthread.md) before enabling it on an untrusted network.

## Database maintenance

Maintenance commands take the same exclusive lock as the server, so stop the
service first. Supply the same data directory on every command:

```bash
extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /srv/extrittio info

extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /srv/extrittio integrity

extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /srv/extrittio checkpoint

extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /srv/extrittio backup /srv/backups/extrittio.db

extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /srv/extrittio verify-backup /srv/backups/extrittio.db
```

Backup creates a checkpointed database copy and a sibling SHA-256 manifest. It
never overwrites an existing file. Restore verifies both before replacement;
`--force` is required when a target database exists, and the replaced files are
retained with a timestamped name.

```bash
extrittio database --database-backend turso --deployment-profile edge \
  --data-dir /srv/extrittio restore /srv/backups/extrittio.db --force
```

Logical `export` and `import` commands exist for backend-neutral JSON transfer.
Run `import --dry-run` first and take a physical backup before replacing data.
Use `extrittio database --help` for their exact arguments.

## Debian arm64 package

Build with Docker Buildx:

```bash
cargo xtask package edge-deb-arm64 --version 0.1.0
```

The package and checksum are written under `dist/debian/`. On a 64-bit Debian
or Raspberry Pi OS host:

```bash
sudo apt install ./extrittio_0.1.0_arm64.deb
sudoedit /etc/extrittio/extrittio.env
sudo systemctl enable --now extrittio.service
```

Set a stable `EXTRITTIO_PUBLIC_URL` and a strong bootstrap password before first
start. Persistent state lives in `/var/lib/extrittio` and is not overwritten by
package upgrades. The package does not include OTBR; install a compatible agent
separately and set `EXTRITTIO_OTBR_AGENT` only if it is not on `PATH`.

## Complete Raspberry Pi appliance

The complete archive is built for a 64-bit Raspberry Pi OS/Debian arm64 host
and includes both `extrittio` and the matching patched `otbr-agent`. Docker
Buildx performs the Linux arm64 build; the Pi does not compile the project.

Configure SSH key access and bootstrap the host from the repository root:

```bash
cargo xtask edge setup-pi \
  --host 192.0.2.10 \
  --user pi \
  --identity ~/.ssh/extrittio-pi
```

This verifies arm64 and passwordless `sudo`, installs runtime libraries and the
systemd unit, and creates `/etc/extrittio/edge.env` only when absent. Configure
that file before starting the service. Use a stable
`/dev/serial/by-id/...` RCP path and the Pi's actual infrastructure interface.

Build and deploy:

```bash
cargo xtask package edge-linux-arm64

cargo xtask edge deploy-pi pi@extrittio-pi.local \
  dist/raspberry-pi/extrittio-edge-linux-arm64-<revision>.tar.gz
```

Deployment verifies archive and binary digests, installs an immutable release
under `/opt/extrittio/releases`, atomically updates `/opt/extrittio/current`,
restarts systemd, and checks `/health`. It never replaces
`/var/lib/extrittio`.

Inspect failures with:

```bash
sudo journalctl -u extrittio.service -e
```

Prior release directories remain available for an explicit symlink rollback.
Database compatibility still applies: restore a verified pre-upgrade Edge
backup if an older executable cannot read the upgraded schema.
