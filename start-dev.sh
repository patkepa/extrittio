#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$ROOT_DIR/docker/docker-compose.yml"
DATABASE_URL="${DATABASE_URL:-postgres://extrittio:extrittio@localhost/extrittio}"
RUST_LOG="${RUST_LOG:-extrittio_backend=info}"
POSTGRES_PORT="${POSTGRES_PORT:-}"

BACKEND_PID=""
FRONTEND_PID=""

log() {
  printf '[start-dev] %s\n' "$*"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf '[start-dev] Missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

port_is_listening() {
  lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1
}

compose_postgres_port() {
  if docker compose -f "$COMPOSE_FILE" ps --status running --services 2>/dev/null | grep -qx postgres; then
    docker compose -f "$COMPOSE_FILE" port postgres 5432 2>/dev/null | sed -n '1s/.*://p'
  fi
}

choose_postgres_port() {
  local existing_port

  existing_port="$(compose_postgres_port)"
  if [[ -n "$existing_port" ]]; then
    printf '%s\n' "$existing_port"
    return
  fi

  if [[ -n "$POSTGRES_PORT" ]]; then
    printf '%s\n' "$POSTGRES_PORT"
    return
  fi

  for port in 5432 5433 5434 5435 5436 5437 5438 5439; do
    if ! port_is_listening "$port"; then
      printf '%s\n' "$port"
      return
    fi
  done

  printf '[start-dev] Could not find a free PostgreSQL host port in 5432-5439.\n' >&2
  exit 1
}

cleanup() {
  local status=$?

  if [[ -n "$BACKEND_PID" ]] && kill -0 "$BACKEND_PID" 2>/dev/null; then
    log "Stopping backend..."
    kill "$BACKEND_PID" 2>/dev/null || true
  fi

  if [[ -n "$FRONTEND_PID" ]] && kill -0 "$FRONTEND_PID" 2>/dev/null; then
    log "Stopping frontend..."
    kill "$FRONTEND_PID" 2>/dev/null || true
  fi

  wait "$BACKEND_PID" "$FRONTEND_PID" 2>/dev/null || true
  exit "$status"
}

trap cleanup INT TERM EXIT

require_command docker
require_command cargo
require_command npm
require_command lsof

if [[ -f "$ROOT_DIR/.env" ]]; then
  log "Loading .env"
  set -a
  # shellcheck source=/dev/null
  source "$ROOT_DIR/.env"
  set +a
  DATABASE_URL="${DATABASE_URL:-postgres://extrittio:extrittio@localhost/extrittio}"
  RUST_LOG="${RUST_LOG:-extrittio_backend=info}"
  POSTGRES_PORT="${POSTGRES_PORT:-}"
fi

if [[ -z "${LIBRARY_PATH:-}" ]]; then
  for brew_prefix in /opt/homebrew /usr/local; do
    if [[ -d "$brew_prefix/opt/libpq/lib" ]]; then
      export LIBRARY_PATH="$brew_prefix/opt/libpq/lib"
      break
    fi
  done
fi

POSTGRES_PORT="$(choose_postgres_port)"
export POSTGRES_PORT

case "$DATABASE_URL" in
  postgres://extrittio:extrittio@localhost/extrittio|postgres://extrittio:extrittio@localhost:5432/extrittio)
    DATABASE_URL="postgres://extrittio:extrittio@localhost:${POSTGRES_PORT}/extrittio"
    ;;
esac

if [[ "$POSTGRES_PORT" != "5432" ]]; then
  log "Host port 5432 is busy; using PostgreSQL on localhost:${POSTGRES_PORT}"
fi

log "Starting PostgreSQL..."
docker compose -f "$COMPOSE_FILE" up -d postgres

log "Waiting for PostgreSQL to accept connections..."
postgres_ready=0
for _ in {1..30}; do
  if docker compose -f "$COMPOSE_FILE" exec -T postgres pg_isready -U extrittio -d extrittio >/dev/null 2>&1; then
    postgres_ready=1
    break
  fi
  sleep 1
done

if [[ "$postgres_ready" != "1" ]]; then
  printf '[start-dev] PostgreSQL did not become ready within 30 seconds.\n' >&2
  exit 1
fi

if [[ "${RUN_MIGRATIONS:-1}" != "0" ]]; then
  if command -v diesel >/dev/null 2>&1; then
    log "Running database migrations..."
    (cd "$ROOT_DIR/backend" && DATABASE_URL="$DATABASE_URL" diesel migration run)
  else
    log "diesel CLI not found; skipping migrations. Install with: cargo install diesel_cli --no-default-features --features postgres"
  fi
fi

if [[ ! -d "$ROOT_DIR/frontend/node_modules" ]]; then
  log "Installing frontend dependencies..."
  (cd "$ROOT_DIR/frontend" && npm install)
fi

log "Starting backend on http://localhost:8080"
(
  cd "$ROOT_DIR"
  export DATABASE_URL RUST_LOG
  cargo run -p extrittio-backend
) &
BACKEND_PID=$!

log "Starting frontend on http://localhost:5173"
(
  cd "$ROOT_DIR/frontend"
  npm run dev -- --host 127.0.0.1
) &
FRONTEND_PID=$!

log "Development services are starting. Press Ctrl-C to stop frontend and backend."

while kill -0 "$BACKEND_PID" 2>/dev/null && kill -0 "$FRONTEND_PID" 2>/dev/null; do
  sleep 1
done

printf '[start-dev] Backend or frontend exited. Shutting down remaining local process.\n' >&2
exit 1
