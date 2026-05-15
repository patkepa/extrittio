#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-$ROOT_DIR/docker/docker-compose.prod.yml}"
BACKUP_DIR="${BACKUP_DIR:-$ROOT_DIR/backups}"
TARGET_VERSION="${EXTRITTIO_VERSION:-}"
SKIP_BACKUP=0

log() {
  printf '[upgrade-compose] %s\n' "$*"
}

die() {
  printf '[upgrade-compose] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Usage: scripts/upgrade-compose.sh --to <version> [--skip-backup]

Upgrades a Docker Compose production install to a pinned Extrittio version.

Options:
  --to <version>     Target Extrittio image tag, for example 1.4.2.
  --skip-backup      Do not create a PostgreSQL backup before migrating.
  -h, --help         Show this help text.

Environment:
  COMPOSE_FILE       Compose file path. Defaults to docker/docker-compose.prod.yml.
  BACKUP_DIR         Local backup directory. Defaults to ./backups.
  EXTRITTIO_VERSION  Target version if --to is not provided.
USAGE
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    die "Missing required command: $1"
  fi
}

compose() {
  docker compose -f "$COMPOSE_FILE" "$@"
}

wait_for_postgres() {
  log "Waiting for PostgreSQL readiness..."
  for _ in {1..60}; do
    if compose exec -T postgres pg_isready -U extrittio -d extrittio >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done

  die "PostgreSQL did not become ready in time."
}

wait_for_backend() {
  log "Waiting for backend readiness..."
  for _ in {1..90}; do
    if compose exec -T backend curl -fsS http://127.0.0.1:8080/ready >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done

  die "Backend did not become ready in time. Check: docker compose -f \"$COMPOSE_FILE\" logs backend"
}

create_backup() {
  mkdir -p "$BACKUP_DIR"

  local timestamp
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  local backup_file="$BACKUP_DIR/extrittio-${timestamp}-before-${TARGET_VERSION}.dump"

  log "Creating PostgreSQL backup at $backup_file"
  compose exec -T postgres pg_dump -U extrittio -d extrittio -Fc > "$backup_file"

  if [[ ! -s "$backup_file" ]]; then
    rm -f "$backup_file"
    die "Backup file was not created."
  fi

  log "Backup complete: $backup_file"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --to)
      [[ $# -ge 2 ]] || die "--to requires a version value."
      TARGET_VERSION="$2"
      shift 2
      ;;
    --skip-backup)
      SKIP_BACKUP=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "Unknown argument: $1"
      ;;
  esac
done

[[ -n "$TARGET_VERSION" ]] || die "Set a target version with --to <version> or EXTRITTIO_VERSION."
[[ -f "$COMPOSE_FILE" ]] || die "Compose file not found: $COMPOSE_FILE"

require_command docker

export EXTRITTIO_VERSION="$TARGET_VERSION"

log "Upgrading Extrittio to $EXTRITTIO_VERSION"
log "Using Compose file: $COMPOSE_FILE"

log "Starting PostgreSQL if needed..."
compose up -d postgres
wait_for_postgres

if [[ "$SKIP_BACKUP" == "1" ]]; then
  log "Skipping PostgreSQL backup by request."
else
  create_backup
fi

log "Pulling target backend and frontend images..."
compose pull backend frontend

log "Stopping application services before migration..."
compose stop frontend backend >/dev/null || true

log "Running database migrations with the target backend image..."
compose run --rm --no-deps backend ./extrittio migrate

log "Starting application services..."
compose up -d backend frontend

wait_for_backend

log "Upgrade completed successfully."
