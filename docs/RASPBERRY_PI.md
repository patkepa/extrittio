# Raspberry Pi Deployment

This guide covers running Extrittio on a Raspberry Pi, especially when the OS
boots from an SD card. The main constraint is write volume: PostgreSQL, device
telemetry, device logs, Docker logs, and internal metrics can all create steady
small writes.

## Backend RPI Mode

Set `RPI_MODE=true` for SD-card friendly backend defaults:

```env
RPI_MODE=true
```

`RPI_MODE` only changes defaults. Any explicit environment variable still wins.

| Setting | Normal default | RPI default | Purpose |
| --- | ---: | ---: | --- |
| `DB_POOL_SIZE` | `16` | `4` | Fewer PostgreSQL connections and lower memory use |
| `OFFLINE_TIMEOUT_SECS` | `300` | `600` | Less churn from transient device gaps |
| `COMMAND_TIMEOUT_SECS` | `30` | `60` | Fewer timeout updates under slow I/O |
| `ALERT_RETENTION_DAYS` | `30` | `7` | Smaller alert history |
| `TELEMETRY_RETENTION_DAYS` | `30` | `7` | Smaller raw telemetry table |
| `LOG_RETENTION_DAYS` | `30` | `7` | Smaller device log table |
| `SYSTEM_METRICS_INTERVAL_SECS` | `10` | `60` | Fewer host metric writes |
| `APP_METRICS_FLUSH_INTERVAL_SECS` | `10` | `60` | Fewer app metric writes |
| `METRICS_RETENTION_HOURS` | `24` | `24` | Internal metrics history |

If `RUST_LOG` is not set, `RPI_MODE=true` also changes the backend's default
log filter from info-level backend logs to warn-level logs.

Recommended starter profile:

```env
RPI_MODE=true
RUST_LOG=warn
DB_POOL_SIZE=4
TELEMETRY_RETENTION_DAYS=7
LOG_RETENTION_DAYS=7
ALERT_RETENTION_DAYS=7
SYSTEM_METRICS_INTERVAL_SECS=60
APP_METRICS_FLUSH_INTERVAL_SECS=60
METRICS_RETENTION_HOURS=24
```

For very small installations, reduce telemetry and log retention further:

```env
TELEMETRY_RETENTION_DAYS=3
LOG_RETENTION_DAYS=3
```

## Storage Layout

Best practical setup:

- Boot the Pi from a high-endurance SD card.
- Put PostgreSQL data on a USB SSD or NVMe drive.
- Keep Docker logs capped.
- Run the backend in release mode and serve the frontend as static files.

If PostgreSQL must stay on the SD card, use short telemetry/log retention and
make sure the device has stable power.

Example PostgreSQL volume on an attached SSD:

```yaml
services:
  postgres:
    volumes:
      - /mnt/extrittio-postgres:/var/lib/postgresql/data
```

## Docker Logs

Docker JSON logs can grow quickly on SD cards. Add a logging limit to each
Compose service:

```yaml
logging:
  driver: json-file
  options:
    max-size: "10m"
    max-file: "3"
```

Apply this to `postgres`, `backend`, `frontend`, and any Zenoh service.

## PostgreSQL Tuning

Start with conservative settings for a Pi with limited RAM:

```conf
shared_buffers = 128MB
effective_cache_size = 512MB
max_connections = 20
checkpoint_timeout = 15min
checkpoint_completion_target = 0.9
wal_compression = on
random_page_cost = 2.0
autovacuum_naptime = 2min
```

If losing the most recent few seconds of writes during power loss is acceptable:

```conf
synchronous_commit = off
```

Do not use that setting for deployments where every telemetry, command, or audit
write must survive sudden power loss.

## OS Mounts

Use `noatime` to avoid write amplification from file access timestamps:

```fstab
UUID=<root-fs-uuid> / ext4 defaults,noatime,commit=60 0 1
tmpfs /tmp tmpfs defaults,noatime,nosuid,size=256m 0 0
```

Prefer zram over swap files on the SD card:

```bash
sudo apt install zram-tools
```

## Build And Run

Do not run the Vite dev server on the Pi. Build the frontend once:

```bash
cd frontend
npm ci
npm run build
```

Build the backend in release mode:

```bash
cargo build --release -p extrittio-backend
```

Then run the release binary with the RPI profile environment:

```bash
RPI_MODE=true \
RUST_LOG=warn \
DATABASE_URL=postgres://extrittio:extrittio@localhost/extrittio \
./target/release/extrittio-backend
```

## Operational Notes

- Keep device telemetry intervals reasonable. The backend cannot reduce writes
  from devices that publish too often.
- Prefer raw telemetry retention in days, not months, on SD-card deployments.
- Use external storage for firmware blobs and backups when possible.
- Back up PostgreSQL regularly to another machine or drive.
- Watch free disk space and table growth after adding new devices.
