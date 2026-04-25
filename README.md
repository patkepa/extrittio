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

After installing `libpq`, make sure the linker can find it:
```bash
export LIBRARY_PATH="/opt/homebrew/opt/libpq/lib:$LIBRARY_PATH"
```

After installing `llvm`, add it to your PATH so the faster linker is picked up automatically:
```bash
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
```

Add both lines to your `~/.zshrc` to make them permanent.

### Other dependencies

- **Rust** — install via [rustup](https://rustup.rs/)
- **Docker** — required to run PostgreSQL locally (`docker compose up -d postgres`)
- **Node.js 18+** — required for the frontend (`npm install && npm run dev`)
- **diesel_cli** — required for database migrations:
  ```bash
  cargo install diesel_cli --no-default-features --features postgres
  ```

## Development Setup

```bash
# 1. Start PostgreSQL
docker compose up -d postgres

# 2. Apply database migrations
cd backend && diesel migration run && cd ..

# 3. Start the backend (in one terminal)
export DATABASE_URL=postgres://extrittio:extrittio@localhost/extrittio
cargo run -p extrittio-backend

# 4. Start the frontend (in another terminal)
cd frontend && npm install && npm run dev
```

The frontend runs on http://localhost:5173 and proxies `/api` to the backend at http://localhost:8080.
