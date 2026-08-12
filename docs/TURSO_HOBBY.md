# Turso Hobby Deployment

Extrittio supports a local-only Turso database for single-node hobby and appliance deployments. PostgreSQL remains the default and the only supported production/HA backend. Turso is rejected when `EXTRITTIO_DEPLOYMENT_PROFILE=production`, and database open failures never fall back to another backend.

## Install and first run

The standalone release shape embeds the React UI and excludes PostgreSQL/Diesel:

```bash
cd apps/frontend
npm ci
npm run build
cd ../..
cargo install --path apps/extrittio --locked \
  --no-default-features --features hobby

extrittio run
```

`extrittio run` always selects the local Turso hobby profile. On first run it
creates the owner account as `admin` / `admin` unless `--admin-username` and
`--admin-password` (or their environment variables) were supplied. It then
prints the browser URL and starts the complete stack. Change the default
password after signing in.

Useful overrides:

```bash
extrittio run \
  --data-dir /srv/extrittio \
  --port 8080 \
  --zenoh-port 7447 \
  --admin-username owner \
  --public-url http://hub.local:8080
```

For devices that reach the hub through a Thread mesh, use
`--zenoh-listen-host ::` and configure an OpenThread Border Router. The
[OpenThread hobby deployment guide](OPENTHREAD_HOBBY.md) covers the nRF52840
RCP topology and IPv6 Zenoh locator.

The data directory contains `extrittio.db`, `extrittio.lock`, certificates, local firmware objects, and operator-created backups. One process exclusively owns a data directory. A second process fails fast.

## Maintenance

These commands acquire the same process lock as the server, so stop the server before using them:

```bash
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data info
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data integrity
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data checkpoint
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data backup ./backups/extrittio.db
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data verify-backup ./backups/extrittio.db
```

Backup creates a checkpointed database copy and a sibling `.sha256` manifest, then reopens the copy and runs its integrity/schema checks. Existing backup files are never overwritten.

Restore validates the backup and checksum before changing the target. If the target exists, `--force` is required; the replaced database and WAL sidecars are preserved with a `pre-restore-<UTC timestamp>` name.

```bash
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data \
  restore ./backups/extrittio.db --force
```

Logical JSON archives preserve SQL value types, including BLOBs, and are intended for model-level portability and comparison:

```bash
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data export ./backups/export.json
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data import ./backups/export.json --dry-run
extrittio database --database-backend turso --deployment-profile hobby --data-dir ./data import ./backups/export.json
```

Import replaces all logical table contents in one deferred-foreign-key transaction. Always take a physical backup first.

## Durability and limits

- Foreign keys are enabled, synchronous mode is `FULL`, and writes use one serialized connection.
- A WAL truncate checkpoint runs every 15 minutes and during graceful shutdown.
- Telemetry/log retention is bounded and maintained by the normal worker tree.
- Turso mode is not multi-process, HA, remote, or cloud-synchronized.
- Database files and firmware objects are mutable state and must stay outside the executable.
- Remote map tiles remain an optional external dependency unless a local tile source is configured separately.

The CI hobby artifact gate builds Linux amd64 and arm64, runs the Turso unit suite and first-run migration smoke test, and rejects binaries linked to `libpq` or PostgreSQL runtime libraries.
