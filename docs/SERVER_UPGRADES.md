# Server Upgrades

This document describes how Extrittio should handle upgrades of the self-hosted
server stack. It is separate from device firmware OTA: server upgrades update
the Extrittio backend, frontend, database schema, and deployment configuration
running on customer infrastructure.

## Goal

Server upgrades should be intentional, versioned, observable, and recoverable.
Customers should be able to move from one Extrittio version to another without
guessing which containers are running, whether migrations were applied, or how
to recover if the new version fails.

The professional upgrade model is:

1. Pin every production deployment to a concrete Extrittio version.
2. Publish signed release metadata for each version.
3. Back up PostgreSQL before risky upgrades.
4. Run database migrations as a controlled step.
5. Restart services and verify health/readiness.
6. Keep a rollback path for application images and a restore path for database
   changes.

## Current Deployment Shape

The production Compose file requires an explicit release tag for the combined
API and web application image:

```yaml
ghcr.io/extrittio/extrittio:${EXTRITTIO_VERSION:?set EXTRITTIO_VERSION}
```

Set `EXTRITTIO_VERSION` to a concrete release before running production Compose:

```bash
export EXTRITTIO_VERSION=1.4.2
docker compose -f deploy/docker/docker-compose.production.yml up -d
```

Pinned versions make upgrades explicit and make rollbacks possible. `latest` is
not the production contract.

The production Compose file also includes:

- PostgreSQL readiness checks with `pg_isready`.
- Dependency-aware Extrittio readiness checks against `/ready`.
- Edge-proxy health checks against the combined UI/API service.
- `depends_on` health conditions so application services do not start before
  their dependencies are ready.

Extrittio already has useful primitives:

- `extrittio migrate` runs pending Diesel migrations and exits.
- `extrittio health` checks backend liveness.
- `extrittio ready` checks backend readiness, including database reachability.
- `/health` is a liveness probe.
- `/ready` checks database, migration, Zenoh, and worker health.
- `/api/v1/system/version` returns runtime version metadata and database status.
- The backend currently also runs migrations during service initialization.

For production-grade upgrades, migrations should be treated as a first-class
upgrade step. Boot-time migrations are acceptable for development and small
single-node installs, but explicit migration commands are easier to audit and
safer for multi-instance or automated deployments.

## Recommended Upgrade Flow

The target customer-facing flow should be:

```bash
extrittioctl backup
extrittioctl upgrade --to 1.4.2
```

Until `extrittioctl` exists, use the repository script:

```bash
scripts/upgrade-compose.sh --to 1.4.2
```

The script performs the current safe Compose upgrade sequence:

1. Requires a target `EXTRITTIO_VERSION`.
2. Starts PostgreSQL if needed.
3. Waits for PostgreSQL readiness.
4. Creates a local `pg_dump -Fc` backup in `./backups`.
5. Pulls the target Extrittio image.
6. Stops the proxy and application.
7. Runs `./extrittio migrate` once with the target application image.
8. Starts the application and proxy.
9. Polls `/ready`.

The equivalent manual flow is:

```bash
# 1. Choose the exact version.
export EXTRITTIO_VERSION=1.4.2

# 2. Start and verify PostgreSQL.
docker compose -f deploy/docker/docker-compose.production.yml up -d postgres
docker compose -f deploy/docker/docker-compose.production.yml exec -T postgres \
  pg_isready -U extrittio -d extrittio

# 3. Back up Postgres.
mkdir -p backups
docker compose -f deploy/docker/docker-compose.production.yml exec -T postgres \
  pg_dump -U extrittio -d extrittio -Fc > backups/extrittio-before-${EXTRITTIO_VERSION}.dump

# 4. Pull the pinned images for the target release.
docker compose -f deploy/docker/docker-compose.production.yml pull extrittio proxy

# 5. Stop application services and run migrations once.
docker compose -f deploy/docker/docker-compose.production.yml stop proxy extrittio
docker compose -f deploy/docker/docker-compose.production.yml run --rm --no-deps extrittio ./extrittio migrate

# 6. Restart application services.
docker compose -f deploy/docker/docker-compose.production.yml up -d extrittio proxy

# 7. Verify backend readiness from inside the backend container.
docker compose -f deploy/docker/docker-compose.production.yml exec -T extrittio \
  curl -fsS -H "X-Extrittio-Health-Token: $EXTRITTIO_HEALTH_TOKEN" http://127.0.0.1:8080/ready
```

The exact commands will evolve with packaging, but the ordering should not:
preflight, backup, pull, migrate, restart, verify.

## Release Metadata

Each published release should include signed metadata. The updater should read
this metadata before pulling images or running migrations.

Example manifest:

```json
{
  "version": "1.4.2",
  "channel": "stable",
  "released_at": "2026-05-15T12:00:00Z",
  "images": {
    "application": "ghcr.io/extrittio/extrittio:1.4.2"
  },
  "compose_url": "https://releases.extrittio.com/1.4.2/docker-compose.production.yml",
  "min_upgrade_from": "1.3.0",
  "requires_backup": true,
  "migration_risk": "normal",
  "release_notes_url": "https://github.com/extrittio/extrittio/releases/tag/v1.4.2",
  "signature": "base64-ed25519-signature"
}
```

The signature should cover the manifest content. The updater should reject an
unsigned or invalid manifest before doing any destructive work.

## Runtime Version Metadata

The backend exposes runtime metadata at:

```bash
curl -fsS http://localhost/api/v1/system/version
```

Response shape:

```json
{
  "version": "0.1.0",
  "commit_sha": "unknown",
  "build_timestamp": "unknown",
  "database": "ready"
}
```

Docker images should be built with release metadata:

```bash
docker build \
  --build-arg EXTRITTIO_VERSION=1.4.2 \
  --build-arg EXTRITTIO_COMMIT_SHA="$(git rev-parse HEAD)" \
  --build-arg EXTRITTIO_BUILD_TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  -f deploy/docker/Dockerfile \
  -t ghcr.io/extrittio/extrittio:1.4.2 \
  .
```

If metadata is not provided, the endpoint falls back to the crate version for
`version` and reports `unknown` for commit/build fields.

## Version Channels

Support explicit channels rather than implicit update behavior:

- `stable`: recommended for production installs.
- `beta`: early access for non-critical environments.
- `dev`: internal or nightly builds.
- `lts`: optional future channel for long-lived deployments.

Customers should be able to stay pinned to a version, follow a channel, or
manually approve each upgrade.

## Database Migrations

Database changes are the highest-risk part of server upgrades.

Use these rules:

- Prefer backward-compatible migrations.
- Use expand/contract migration patterns for breaking schema changes.
- Never require multiple application instances to race on migrations.
- Record migration state in the database and expose it in diagnostics.
- Document whether a release is reversible.
- Take a backup before migrations that modify existing data or drop columns.

Recommended long-term split:

```bash
extrittio migrate
extrittio serve
extrittio doctor
extrittio version
```

`extrittio serve` should eventually be able to start with migrations disabled in
production, while `extrittio migrate` is run as a pre-start job.

## Rollback Policy

Application rollback and database rollback are different.

If the new containers fail before migrations run, rollback is simple:

```bash
docker compose -f deploy/docker/docker-compose.production.yml up -d extrittio proxy
```

with the previous image tags restored in Compose.

If migrations have run, rollback depends on migration compatibility:

- Compatible migration: roll back images to the previous version.
- Incompatible migration: restore the PostgreSQL backup.
- Data migration: follow release-specific rollback instructions.

The upgrade tool should make this explicit before proceeding.

## Web UI Behavior

The Extrittio web app should not directly update the host by default.

Avoid mounting the Docker socket into the backend or frontend just to support
"one-click update". Giving the web application Docker control is effectively
giving it root-equivalent control over the host.

The safe default UI behavior is informational:

- Show the current Extrittio version.
- Show the latest available stable version.
- Show release notes and migration risk.
- Show whether a backup is recommended or required.
- Show the exact command an operator should run.

One-click updates should only be added through a separate host updater service
with narrow permissions, signed release manifests, explicit admin approval,
audit logs, and health-check based rollback behavior.

## Host Updater Service

If Extrittio later supports managed one-click updates, use a separate updater
agent instead of the main web application.

Responsibilities:

- Poll or receive available release metadata.
- Verify manifest signatures.
- Check compatibility from the current version.
- Create and verify backups.
- Pull pinned images.
- Run migrations once.
- Restart services.
- Verify `/health` and `/ready`.
- Roll back application images on startup failure where safe.
- Record audit events.

The updater should expose a small local API to the backend or CLI. It should not
accept arbitrary shell commands from the web app.

## Kubernetes And Larger Installs

Compose should be the primary self-hosted path first. Larger installs should use
a Helm chart with the same upgrade model:

1. Pin chart and image versions.
2. Run migrations as a Kubernetes Job.
3. Use readiness probes before serving traffic.
4. Use rolling deployments for the combined application image.
5. Keep database backups outside the cluster lifecycle.

Do not rely on every backend pod running migrations during boot in a
multi-instance Kubernetes deployment.

## Roadmap

1. Replace `latest` in production documentation with pinned release tags.
2. Add a documented Compose upgrade procedure.
3. Add release manifests and signature verification.
4. Add `extrittioctl backup`, `extrittioctl upgrade`, and `extrittioctl rollback`.
5. Make boot-time migrations configurable for production.
6. Add UI update notifications.
7. Add an optional host updater service for controlled one-click upgrades.
8. Add Helm packaging with migration jobs for larger deployments.

## Acceptance Criteria

A server upgrade system is production-ready when an operator can answer:

- Which Extrittio version is currently running?
- Which version is available?
- Are the target images signed and pinned?
- Will this release run database migrations?
- Was a backup created before the migration?
- Did the new version become healthy?
- What is the rollback or restore path?

If those answers are visible before and after the upgrade, the system behaves
like a professional self-hosted product instead of a best-effort container pull.
