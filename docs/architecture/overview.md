# System architecture

This document describes the implementation currently in the repository.

## Runtime shapes

The `extrittio` package is both the service executable and the administrative
CLI.

| Command | Persistence | UI | Primary use |
| --- | --- | --- | --- |
| `extrittio serve` | PostgreSQL by default | Built `apps/frontend/dist`, an explicit `--ui-dir`, or API-only with `--no-ui` | Development and production server deployments |
| `extrittio run` | Local Turso | Embedded in the Edge build | Single-node Edge installations |

The production Cargo feature enables PostgreSQL, S3-compatible firmware
storage, OTLP export, Swagger UI support, mDNS, and the production allocator.
Runtime configuration still decides which optional services are active. Edge
uses a local database and can supervise a compatible OpenThread Border Router;
it is not a remote Turso, multi-process, or HA mode.

## Data flow

```text
device client
  -> Zenoh topic and authenticated device identity
  -> legacy Protobuf handler or contract-routed event handler
  -> domain service and rule/action workers
  -> PostgreSQL or Turso persistence adapter
  -> Axum REST API
  -> React console, iOS app, or CLI
```

The backend starts Zenoh subscribers and supervised background workers with the
HTTP service. Device identity from the topic and authenticated connection is
validated against the payload. Unknown devices are rejected; they must be
created from a published blueprint before sending traffic.

The newer contract path accepts a universal event envelope, resolves its route
against the assigned materialized contract, validates the payload, stores the
canonical event, and extracts typed metric samples. Fixed Protobuf telemetry
and several `device_type` fields remain compatibility surfaces for existing
clients.

## Backend boundaries

Backend features are vertical domains under `crates/backend/src/domains`.
Their common shape is:

```text
HTTP route and DTOs
  -> domain service and authorization
  -> backend-neutral repository trait
  -> persistence/postgres or persistence/turso adapter
```

`AppState` holds the selected persistence ports, Zenoh session, worker health,
configuration, and shared services. PostgreSQL's synchronous Diesel work is
kept behind the repository executor boundary. Turso implements the same domain
ports for the supported Edge feature set.

The main domains include identity and RBAC, device blueprints and devices,
fleets and zones, telemetry and analytics, shadows and configuration, commands,
firmware and OTA, rules and alerts, audit/activity, and operational health and
metrics.

## Device model

A device blueprint is a reusable, tenant-owned definition. Drafts are mutable;
published revisions are immutable. Creating a device from a published revision
atomically creates the device, initial shadow, certificate material, resolved
contract, and contract assignment. The contract contains the concrete device
identity, resolved Zenoh endpoint, routes, schemas, commands, configuration,
and presentation metadata.

The compiler in `crates/device-contract` validates bounded JSON Schema,
normalizes the document, and produces a deterministic contract hash. Runtime
ingress uses the materialized contract rather than branching on a named device
family. See [Device blueprints](device-blueprints.md) for the current lifecycle.

## Public interfaces

- REST routes use `/api/v1`; liveness and readiness are `/health` and `/ready`.
- `api/openapi.json` is the committed HTTP contract.
- `apps/frontend/src/types/openapi.ts` is generated from that contract.
- Shared Protobuf messages and Zenoh topic helpers live in `crates/common`.
- Generated C/nanopb bindings are checked by `cargo xtask verify protocol`.

The browser authenticates with an HTTP-only session cookie. The CLI can request
a bearer token and stores it in its local configuration. Device connections use
provisioned identity and can use Zenoh TLS/mTLS.

## Persistence and mutable data

PostgreSQL is the production server database. Turso is a local file owned by a
single Edge process. Both adapters enforce tenant-scoped domain operations and
apply their own migration sets from `crates/backend/migrations`.

Firmware uses local object storage by default and can use S3-compatible storage
in the production build. Database files, firmware objects, certificates, and
backups are mutable deployment state and must remain outside release archives
and container image layers.

## Frontends

The operations console is a React SPA using generated OpenAPI types, TanStack
Query for server state, Zustand for UI state, and uPlot for time-series charts.
Feature slices live under `apps/frontend/src/features`; shared shell and
infrastructure code remain under `components`, `hooks`, `stores`, and `lib`.

The native iOS app uses SwiftUI, SwiftData for a disposable cache, Keychain for
credentials, and repository/use-case boundaries. It consumes the same REST API.

## Security invariants

- Request code never trusts a client-supplied tenant as authorization.
- Tenant-owned records and relationships stay tenant-scoped through service and
  persistence layers.
- Blueprint routes, payload sizes, schemas, and runtime expressions are bounded
  before use.
- Device topics and payload identity are cross-checked.
- Browser cookies, device certificates, JWT signing secrets, encryption keys,
  and readiness tokens must be replaced and protected in production.
- Production deployments terminate public HTTP TLS at a trusted ingress and use
  Zenoh TLS/mTLS for untrusted networks.
