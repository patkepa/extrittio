# Extrittio

Extrittio is a self-hosted IoT hub for provisioning, operating, and observing
connected devices. It combines a Rust control plane, Zenoh device messaging,
Protobuf contracts, PostgreSQL production persistence or embedded Turso hobby
persistence, a React operations console, and a native SwiftUI companion app for
iOS.

The platform supports multi-tenant device and fleet management, telemetry and
logs, desired/reported shadows, commands, OTA firmware deployments, rules,
alerts, audit events, operational metrics, and native/embedded client SDKs.

## One-command hobby install

Build the frontend once, install the Turso-only executable, and run the whole
hub:

```bash
cd apps/frontend
export NODE_AUTH_TOKEN='your-github-packages-token'
npm ci
npm run build
cd ../..

cargo install --path apps/extrittio --locked \
  --no-default-features --features hobby

extrittio run
```

On first run Extrittio creates its local database, certificates, and firmware
directory, generates an owner password when one was not supplied, prints the
credentials and web URL, and starts the UI, API, Zenoh listener, and background
workers. Open [http://localhost:8080](http://localhost:8080).

By default mutable data uses the operating system's local application-data
directory. Override it with `extrittio run --data-dir /path/to/extrittio`.

## Architecture

```text
Devices and gateways
  -> Zenoh + Protobuf
  -> supervised ingestion and rule processing
  -> tenant-scoped services and PostgreSQL or local Turso
  -> Axum REST API / OpenAPI
  -> React operations console and CLI
```

The Rust backend is organized as vertical domains under
`crates/backend/src/domains`. Shared device contracts and topic builders live in
`crates/common`; native clients share lifecycle, identity, shadow, and OTA logic
through `clients/rust/runtime`.

## Repository Layout

| Path | Purpose |
| --- | --- |
| `apps/extrittio` | Server and administrative CLI |
| `apps/frontend` | React/TypeScript operations console |
| `apps/mobile-app-ios` | Native SwiftUI companion app |
| `crates/backend` | API, services, repositories, workers, and Zenoh ingestion |
| `crates/common` | Canonical Protobuf types, topics, shadows, OTA constants |
| `clients/rust` | Shared SDK/runtime and Linux, macOS, RPi, ESP32 clients |
| `clients/c` | C SDK and ESP-IDF examples |
| `clients/arduino` | Arduino/PlatformIO client library |
| `deploy/docker` | Development and production Compose definitions |
| `api` | Committed OpenAPI contract |

## Quick Start

Required tools are Docker, the Rust toolchain declared in `rust-toolchain.toml`,
Protobuf, PostgreSQL client libraries, and Node.js 22. The frontend consumes
GitHub Packages under `@extrittio`; set `NODE_AUTH_TOKEN` to a token with package
read access before running `npm ci`.

On macOS, install native dependencies with:

```bash
brew install protobuf libpq
export LIBRARY_PATH="/opt/homebrew/opt/libpq/lib:$LIBRARY_PATH"
```

> **Note:** Without the `LIBRARY_PATH` export, the build will fail with `ld: library 'pq' not found` even after `brew install libpq`.

The native iOS app additionally requires Xcode 26.x with the iOS 26 SDK,
XcodeGen, SwiftLint, and SwiftFormat. The expected tool versions are listed in
`apps/mobile-app-ios/Tools/versions.env`.

Start the full development stack:

```bash
./start-dev.sh
```

Or run each part explicitly:

```bash
docker compose -f deploy/docker/docker-compose.yml up -d postgres
export DATABASE_URL=postgres://extrittio:extrittio@localhost/extrittio
cargo run -p extrittio -- migrate
cargo run -p extrittio -- serve

cd apps/frontend
npm ci
npm run dev
```

The backend listens on `http://localhost:8080`; Vite listens on
`http://localhost:5173` and proxies `/api` to the backend. The production binary
can serve the built SPA directly.

## CLI

```bash
cargo run -p extrittio -- --help
cargo run -p extrittio -- serve
cargo run -p extrittio -- migrate
cargo run -p extrittio -- init
cargo run -p extrittio -- health
cargo run -p extrittio -- ready
cargo run -p extrittio -- auth login --username admin
cargo run -p extrittio -- device-types list
cargo run -p extrittio -- fleets list
```

Browser authentication uses secure HTTP-only cookies. The CLI can opt into a
bearer token stored in its local configuration; override its connection with
`--url`, `--token`, `EXTRITTIO_URL`, or `EXTRITTIO_TOKEN`.

## iOS App

The native SwiftUI companion app lives in [`apps/mobile-app-ios`](apps/mobile-app-ios/README.md).

```bash
cd apps/mobile-app-ios
make bootstrap
make build
make test
```

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude extrittio-macos --all-targets -- -D warnings
cargo test --workspace --exclude extrittio-macos

cd apps/frontend
npm run format:check
npm run lint
npm test
npm run build
```

CI also verifies constrained Rust SDK feature sets, canonical Protobuf-to-nanopb
generation, the C SDK, generated OpenAPI types, dependency changes, and dependency
vulnerabilities. Release images include an SPDX SBOM, keyless signature, and
provenance attestation. Reviewed, time-bounded dependency-policy exceptions are
documented in [docs/DEPENDENCY_EXCEPTIONS.md](docs/DEPENDENCY_EXCEPTIONS.md).

## Production and Operations

Use the version-pinned stack in
[`deploy/docker/docker-compose.production.yml`](deploy/docker/docker-compose.production.yml),
not the development Compose file. Production configuration fails fast for unsafe
origins, missing secrets, malformed URLs, and incompatible TLS settings.

The service exposes liveness and dependency-aware readiness endpoints, structured
request IDs and error responses, audit events, bounded rate limiting, JSON logs,
and optional OTLP/HTTP tracing. See:

- [Production deployment](docs/PRODUCTION_DEPLOYMENT.md)
- [Server upgrades and rollback](docs/SERVER_UPGRADES.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Governance](GOVERNANCE.md)
- [Code of conduct](CODE_OF_CONDUCT.md)

## Private Project

This is a private, proprietary project. No license or permission to use, copy,
modify, publish, or redistribute the source is granted. All first-party Rust and
frontend packages are explicitly marked as non-publishable.
