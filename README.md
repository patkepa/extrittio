# Extrittio
Open-Source IoT Hub Platform - A self-hosted alternative to Azure IoT Hub

Extrittio is a modern, scalable IoT platform built with Rust and React, designed for managing and monitoring IoT devices with enterprise-grade features, using high performance stack using zenoh and protobufs.

## Prerequisites

### macOS

```bash
# Protocol Buffers compiler (required — used by the common crate)
brew install protobuf

# PostgreSQL client library (required — used by Diesel/backend)
brew install libpq

# LLVM / lld (optional — enables faster incremental linking on Apple Silicon)
brew install llvm
```

After installing, add these to your `~/.zshrc` and run `source ~/.zshrc`:

```bash
# Required: lets the linker find libpq when building the backend
export LIBRARY_PATH="/opt/homebrew/opt/libpq/lib:$LIBRARY_PATH"

# Optional: enables the faster lld linker (only if you installed llvm above)
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
```

> **Note:** Without the `LIBRARY_PATH` export, the build will fail with `ld: library 'pq' not found` even after `brew install libpq`.

### Other dependencies

- **Rust** — install via [rustup](https://rustup.rs/)
- **Docker** — required to run PostgreSQL locally (`docker compose -f docker/docker-compose.yml up -d postgres`)
- **Node.js 18+** — required for the frontend (`npm install && npm run dev`)
- **diesel_cli** — required for database migrations:
  ```bash
  cargo install diesel_cli --no-default-features --features postgres
  ```

## Development Setup

```bash
# Start PostgreSQL, run migrations when diesel_cli is installed, then start
# the backend and frontend dev servers.
./start-dev.sh
```

The frontend runs on http://localhost:5173 and proxies `/api` to the backend at http://localhost:8080.
If port 5432 is already in use, the script will automatically publish Docker PostgreSQL on the next available port from 5433-5439 and pass that URL to the backend.

Manual setup:

```bash
# 1. Start PostgreSQL
docker compose -f docker/docker-compose.yml up -d postgres

# 2. Apply database migrations
cd backend && diesel migration run && cd ..

# 3. Start the backend (in one terminal)
export DATABASE_URL=postgres://extrittio:extrittio@localhost/extrittio
cargo run -p extrittio-backend

# 4. Start the frontend (in another terminal)
cd frontend && npm install && npm run dev
```
