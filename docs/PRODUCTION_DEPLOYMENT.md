# Production Deployment

The supported single-host production topology is the version-pinned Compose
stack in `deploy/docker/docker-compose.production.yml`. It runs PostgreSQL, the
combined Extrittio API/UI image, and an nginx edge proxy. The development
Compose file is intentionally separate and must not be used as a production
template.

## Prepare

```bash
cp deploy/docker/production.env.example deploy/docker/.env.production
```

Replace every placeholder secret, set a concrete `EXTRITTIO_VERSION`, and set
the public domain/URL. Keep `.env.production` outside version control and limit
its permissions to the deployment operator.

The Zenoh device port is published on `ZENOH_PORT` (7447 by default). Enable
`ZENOH_TLS_ENABLED=true` for an internet-reachable deployment. Set
`OTEL_EXPORTER_OTLP_ENDPOINT` when an OTLP/HTTP collector is available.

Firmware uploads default to the durable `firmware` Compose volume. For multiple
application replicas or externally managed durability, set
`FIRMWARE_STORAGE_BACKEND=s3`, configure the bucket/region and optional
S3-compatible endpoint, and provide credentials through the deployment secret
manager. HTTP object-store endpoints require an explicit
`FIRMWARE_S3_ALLOW_HTTP=true` opt-in and should only be used on trusted networks.

## Start

```bash
docker compose \
  --env-file deploy/docker/.env.production \
  -f deploy/docker/docker-compose.production.yml \
  up -d
```

Check both liveness and dependency-aware readiness:

```bash
curl -fsS -H 'Host: extrittio.example.com' http://127.0.0.1/health
curl -fsS \
  -H 'Host: extrittio.example.com' \
  -H 'X-Extrittio-Health-Token: replace-with-random-secret' \
  http://127.0.0.1/ready
```

## Operate

- Pin releases; do not deploy `latest` in production.
- Back up PostgreSQL before migrations.
- Use `scripts/upgrade-compose.sh --to <version>` for the guarded upgrade flow.
- Store database backups and encryption keys outside the Compose volume lifecycle.
- Back up the local firmware volume, or apply versioning and lifecycle policy to
  the configured object-storage bucket.
- Terminate public TLS at a trusted ingress and restrict direct database access.

See [Server Upgrades](SERVER_UPGRADES.md) for migration and rollback procedures.
