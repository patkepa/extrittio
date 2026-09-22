# Extrittio

Extrittio is a self-hosted IoT platform for provisioning, managing, and
monitoring connected devices. It is designed for single-binary edge
deployments, with an embedded web console and client support for embedded
devices, including Raspberry Pi, ESP32, and Arduino-class hardware.

Core capabilities include device blueprints, fleet management, telemetry,
desired and reported state, remote commands, firmware deployments, rules,
alerts, audit events, and operational metrics. Devices communicate over Zenoh,
and the HTTP API is defined in [`api/openapi.json`](api/openapi.json).



**CAUTION:** It's still work in progress!
## Runtime options

| Runtime | Storage | Use case |
| --- | --- | --- |
| `extrittio serve` | PostgreSQL | Development and multi-service deployments |
| `extrittio run` | Local Turso | Single-node edge appliances with an embedded web UI |

## Quick start

Extrittio requires Rust 1.90 and Node.js 22. Verify your environment:

```bash
cargo xtask doctor edge
cargo xtask doctor cloud
```

Start the PostgreSQL-backed development stack:

```bash
cargo xtask cloud run dev
```

Or start the single-node Edge runtime:

```bash
cargo xtask edge run dev
```

The API runs at `http://localhost:8080` and the web console at
`http://localhost:5173`. See the command help for backend-only, frontend-only,
and custom-port options.

## Project structure

| Path | Description |
| --- | --- |
| `apps/extrittio` | Server, Edge runtime, and administration CLI |
| `apps/frontend` | React and TypeScript operations console |
| `apps/mobile-app-ios` | Native SwiftUI companion app |
| `crates` | Core services, persistence, protocols, and shared libraries |
| `clients` | Native, embedded, Arduino, and simulator clients |
| `deploy` | Docker, Debian, and Raspberry Pi packaging |
| `tools/xtask` | Development, verification, and packaging tasks |

## Development

Run checks for the area you changed:

```bash
cargo xtask verify backend
cargo xtask verify frontend
cargo xtask verify protocol
cargo xtask verify ios
```

For setup, architecture, and deployment guidance, see the
[documentation index](docs/README.md). Contribution and security policies are
available in [CONTRIBUTING.md](CONTRIBUTING.md) and
[SECURITY.md](SECURITY.md).

## License

This source-available portfolio project is not open source. No permission is
granted to use, copy, modify, publish, or redistribute the source. All rights
are reserved.
