# Extrittio

Extrittio is a self-hosted IoT hub for provisioning, operating, and observing
connected devices. The repository contains a Rust control plane, a React
operations console, a native iOS app, and clients for Linux, macOS, Raspberry
Pi, ESP32, and Arduino-class projects.

The platform currently supports tenant-scoped device blueprints and contracts,
fleets, telemetry and analytics, desired/reported state, commands, firmware and
OTA deployments, rules, alerts, audit events, and operational metrics. Devices
communicate over Zenoh; the HTTP API is described by the committed
[`api/openapi.json`](api/openapi.json) contract.

## Choose a runtime

Extrittio has two supported runtime shapes:

| Runtime | Database | Intended use |
| --- | --- | --- |
| `extrittio serve` | PostgreSQL | Development and multi-service production deployments |
| `extrittio run` | Local Turso | A single-node Edge appliance with an embedded web UI |

The Edge runtime can supervise a packaged OpenThread Border Router when a
compatible radio co-processor is attached. It is not a high-availability or
multi-process database mode.

## Local development

The repository pins Rust 1.90 and requires Node.js 22. Check the prerequisites
for the runtime you want to develop:

```bash
cargo xtask doctor edge
cargo xtask doctor cloud
```

On macOS, the core native dependencies are:

```bash
brew install protobuf libpq cmake ninja
```

Start the PostgreSQL-backed Cloud development stack from the repository root:

```bash
cargo xtask cloud run dev
```

For the single-node Edge runtime with local Turso storage:

```bash
cargo xtask edge run dev
```

Both commands install frontend dependencies when needed, start the Rust backend
and Vite in one terminal, and stop their child processes on Ctrl-C. The API
listens on `http://localhost:8080`; the web console is at
`http://localhost:5173`. Edge uses the development account `admin` / `admin`;
Cloud uses `admin` / `Extrittio-dev1!` to satisfy the server password policy.
Use `--backend-only`, `--frontend-only`, or the port flags shown by `--help` for
more focused work.

## Single-node Edge

Use `cargo xtask edge run dev` for source development. To build the deployable
Edge executable with its web console embedded, build the assets and install it:

```bash
cd apps/frontend
npm ci
npm run build
cd ../..

cargo install --path apps/extrittio --locked \
  --no-default-features --features edge

EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD='choose-a-strong-password' extrittio run
```

By default, Edge stores its database, certificates, firmware, and backups in
the operating system's application-data directory. Use `--data-dir` or
`EXTRITTIO_DATA_DIR` to choose an explicit location.

See [Edge deployment](docs/deployment/edge.md) for packaged Debian and
Raspberry Pi workflows and [OpenThread](docs/deployment/openthread.md) for the
radio and IPv6 path.

## Repository layout

| Path | Purpose |
| --- | --- |
| `apps/extrittio` | Server, Edge runtime, and administrative CLI |
| `apps/frontend` | React/TypeScript operations console |
| `apps/mobile-app-ios` | Native SwiftUI companion app |
| `crates/backend` | HTTP/Zenoh transport, runtime composition, workers, and outbound integrations |
| `crates/backend-core` | Business applications, policy, and persistence ports |
| `crates/backend-postgres`, `crates/backend-turso` | Database repositories, SQL, migrations, and lifecycle operations |
| `crates/device-contract` | Blueprint validation and deterministic contract compilation |
| `crates/common` | Shared Protobuf messages and Zenoh topic helpers |
| `clients` | Native, embedded, Arduino, and simulator clients |
| `deploy` | Docker, Debian, and Raspberry Pi packaging |
| `tools/xtask` | Repository build, verification, packaging, and iOS tasks |

## Verification

Run the checks for the part of the repository you changed:

```bash
cargo xtask edge test
cargo xtask cloud test
cargo xtask verify backend
cargo xtask verify frontend
cargo xtask verify protocol
cargo xtask verify ios
```

`cargo xtask verify all` includes the native iOS build and therefore requires
the Apple toolchain.

## Documentation

Start with the [documentation index](docs/README.md). It links to the current
architecture, device-blueprint and analytics behavior, production Docker
deployment, Edge deployment, and OpenThread setup.

Contributor workflow and security reporting live in
[CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).

## License status

This repository is publicly visible as a source-available portfolio project.
It is not open source: no license or permission to use, copy, modify, publish,
or redistribute the source is granted. All rights are reserved.
