# Extrittio

Extrittio is a self-hosted IoT hub for provisioning, operating, and observing
connected devices. It combines a Rust control plane, Zenoh device messaging,
Protobuf contracts, PostgreSQL production persistence or embedded Turso Edge
persistence, a React operations console, and a native SwiftUI companion app for
iOS.

The platform supports multi-tenant device and fleet management, telemetry and
logs, desired/reported shadows, commands, OTA firmware deployments, rules,
alerts, audit events, operational metrics, and native/embedded client SDKs.

## One-command Extrittio Edge install

Build the frontend once, install the Turso-only executable, and run the whole
hub:

```bash
cd apps/frontend
npm ci
npm run build
cd ../..

cargo install --path apps/extrittio --locked \
  --no-default-features --features edge

extrittio run
```

On first run Extrittio creates its local database, certificates, and firmware
directory, creates the local owner account as `admin` / `admin`, prints the web
URL, and starts the UI, API, Zenoh listener, and background workers. Open
[http://localhost:8080](http://localhost:8080) and change the default password
after signing in.

To build on an Apple Silicon Mac and deploy the complete Linux arm64 Extrittio
Edge package (including OpenThread Border Router) to a Raspberry Pi, use the
[Raspberry Pi Edge Deployment](docs/RASPBERRY_PI_EDGE_DEPLOYMENT.md)
workflow. It produces a checksum-verified release archive and deploys it
atomically without replacing the Pi's persistent data.

For a native Raspberry Pi OS/Debian package that contains only the embedded-UI
single-node executable and optionally uses a locally installed OpenThread agent,
use the
[Debian Edge Package Deployment](docs/DEBIAN_EDGE_DEPLOYMENT.md) workflow.

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
Protobuf, PostgreSQL client libraries, and Node.js 22. The frontend consumes the
public `@patkepa/kantzen-ui` package from npm, so `npm ci` needs no package token.

On macOS, install native dependencies with:

```bash
brew install protobuf libpq
export LIBRARY_PATH="/opt/homebrew/opt/libpq/lib:$LIBRARY_PATH"
```

> **Note:** Without the `LIBRARY_PATH` export, the build will fail with `ld: library 'pq' not found` even after `brew install libpq`.

The native iOS app additionally requires Xcode 26.x with the iOS 26 SDK,
XcodeGen, SwiftLint, and SwiftFormat. The expected tool versions are listed in
`apps/mobile-app-ios/Tools/versions.env`.

Use `cargo xtask --help` for supported build, install, packaging, native iOS,
protocol-generation, and Edge operations. For local development, run each
service explicitly:

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
cargo xtask ios bootstrap
cargo xtask ios build
cargo xtask ios test
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
