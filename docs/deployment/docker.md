# Docker production deployment

Refactor-specific migration ordering and rollback limits are documented in
[Backend ownership and upgrade notes](backend-refactor.md).

The supported server topology is
`deploy/docker/docker-compose.production.yml`. It runs PostgreSQL, the combined
Extrittio API/UI image, and an nginx HTTP edge proxy. The development Compose
file has different services and defaults and is not a production template.

## Prepare

Copy the maintained environment template:

```bash
cp deploy/docker/production.env.example deploy/docker/.env.production
```

Before starting, set:

- an explicit `EXTRITTIO_VERSION` image tag;
- the public domain, URL, and matching CORS origin;
- unique PostgreSQL, JWT, key-encryption, readiness, and bootstrap-admin
  secrets;
- the Zenoh TLS and firmware-storage settings appropriate for the network.

Keep this file outside version control and readable only by the deployment
operator. `EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD` is used only while the database
has no users, but it must still be set to a strong value for first boot.

The included nginx service listens on HTTP port 80 and the application sets
secure browser cookies. Put the stack behind a trusted HTTPS ingress or load
balancer; do not expose this HTTP listener directly to users over an untrusted
network.

The Zenoh device port is published separately on `ZENOH_PORT` (7447 by
default). Keep `ZENOH_TLS_ENABLED=true` for an untrusted network and provision
each device with its client certificate.

## Start and verify

```bash
docker compose \
  --env-file deploy/docker/.env.production \
  -f deploy/docker/docker-compose.production.yml \
  up -d
```

This blueprint-only release starts from a fresh database schema. It has no
legacy device-type backfill or dual-read path. After first boot, publish a
[device blueprint](../architecture/device-blueprints.md) before provisioning.

The application runs pending migrations during initialization. Verify the
public routing path and the dependency-aware readiness check:

```bash
curl -fsS -H 'Host: extrittio.example.com' http://127.0.0.1/health
curl -fsS \
  -H 'Host: extrittio.example.com' \
  -H 'X-Extrittio-Health-Token: replace-with-your-token' \
  http://127.0.0.1/ready
```

`/health` proves the process is alive. `/ready` also checks the database,
migrations, Zenoh, and supervised workers.

## Firmware storage

The default `local` backend stores firmware in the named `firmware` volume.
Back up that volume with PostgreSQL.

For externally managed storage, use `FIRMWARE_STORAGE_BACKEND=s3` and configure
the bucket, region, credentials, and optional S3-compatible endpoint from the
environment template. Plain HTTP object-store endpoints require
`FIRMWARE_S3_ALLOW_HTTP=true` and should be limited to a trusted private
network.

## Upgrade

Use a concrete release tag; `latest` is not a production upgrade contract. The
guarded script starts PostgreSQL, waits for it, creates a `pg_dump -Fc` backup,
pulls the target image, stops the application and proxy, runs migrations once,
starts the services, and waits for `/ready`.

The script reads Compose variables from the shell (or the standard Compose
`.env`), not from `.env.production` automatically. Load the prepared file before
running it:

```bash
set -a
. deploy/docker/.env.production
set +a
scripts/upgrade-compose.sh --to 0.1.1
```

Backups are written to `backups/` by default. Set `BACKUP_DIR` to durable
storage outside the repository or Compose volume lifecycle. Do not use
`--skip-backup` unless a verified external backup already exists.

## Rollback

Application and database rollback are separate:

- If no incompatible migration ran, restore the previous
  `EXTRITTIO_VERSION` and start the stack again.
- If a migration is not backward compatible, stop the application and restore
  the pre-upgrade PostgreSQL dump before starting the previous image.
- Follow release-specific notes for any data migration; the generic script
  cannot determine whether an older binary can read a newer schema.

Always keep database backups, encryption keys, firmware objects, and the exact
previous image tag or digest outside the container lifecycle.

## Observability

The service emits structured logs when `EXTRITTIO_LOG_FORMAT=json`. Set
`OTEL_EXPORTER_OTLP_ENDPOINT` to enable OTLP/HTTP tracing in the production
image. Inspect the stack with:

```bash
docker compose \
  --env-file deploy/docker/.env.production \
  -f deploy/docker/docker-compose.production.yml \
  ps

docker compose \
  --env-file deploy/docker/.env.production \
  -f deploy/docker/docker-compose.production.yml \
  logs extrittio
```
