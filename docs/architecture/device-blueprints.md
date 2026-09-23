# Device blueprints and contracts

Status: implemented for backend and web device creation, contract-routed events,
analytics, rules, and firmware targeting. A fresh database is required for the
blueprint-only baselines. iOS, C SDK, Arduino, shared Protobuf, and embedded
producer migrations are deferred; their old payloads are not accepted by this
backend.

Device blueprints keep device-family knowledge in validated data instead of
hard-coded backend or frontend branches. Extrittio still owns the universal
control-plane rules: identity, tenancy, routes, schemas, limits, timestamps,
state reconciliation, command correlation, and lifecycle.

## Model

```text
mutable blueprint draft
  -> validate
  -> immutable published blueprint revision
  + deployment-owned Zenoh endpoint and device identity
  + device configuration overlay
  -> immutable materialized device contract
  -> device assignment and acknowledgement
```

- A **blueprint** is a stable tenant-owned identity and display record.
- Its **draft** is the editable authoring document.
- A **published revision** is immutable and has a deterministic document hash.
- A **device contract** is the resolved per-device snapshot compiled from one
  published revision.
- A **contract assignment** records the desired contract, acknowledgement,
  status, and error state.

The compiler and document types live in `crates/device-contract`. Maintained,
vendor-neutral API creation examples cover three common device categories:

- [`environment-sensor.create-request.json`](../../blueprints/environment-sensor.create-request.json)
- [`smart-plug.create-request.json`](../../blueprints/smart-plug.create-request.json)
- [`asset-tracker.create-request.json`](../../blueprints/asset-tracker.create-request.json)

The examples are validated by the same production validator used when a draft
is published.

## Implemented lifecycle

The web console exposes authoring under **Settings → Device Blueprints**. The
HTTP API supports:

```text
GET|POST  /api/v1/device-blueprints
GET       /api/v1/device-blueprints/{id}
GET|PUT   /api/v1/device-blueprints/{id}/draft
POST      /api/v1/device-blueprints/{id}/draft/validate
POST      /api/v1/device-blueprints/{id}/draft/publish
GET       /api/v1/device-blueprints/{id}/revisions/latest
GET       /api/v1/device-blueprint-revisions/{id}
```

Only published revisions can create devices. `POST /api/v1/devices` requires a
`blueprint_revision_id`; `device_type_id` is no longer accepted.
Creation validates the configuration overlay and atomically creates the device,
initial shadow, certificate material, materialized contract, and assignment.

`GET /api/v1/devices/{id}/contract` returns the effective contract and current
assignment state. The first valid contract event acknowledges convergence.

Use the web console or `extrittio devices create --blueprint-revision-id` for
blueprint-based provisioning.

## Blueprint contents

The current document model can declare:

- runtime heartbeat and message/rate limits;
- logical transports and routes;
- bounded JSON schemas and payload encodings;
- typed stream fields, units, aggregates, and presentation hints;
- commands and their input/result schemas;
- desired configuration and reported state;
- health signals, firmware compatibility, relationships, and presentation.

Publishing performs structural and semantic validation, including key and route
references, bounded schema complexity, field paths and types, supported
aggregates, command/config schemas, and presentation references. Compilation
canonicalizes the effective contract and hashes it deterministically.

The platform resolves the physical Zenoh endpoint from the server address used
by the operator plus the active listener settings. Reusable blueprint documents
therefore do not contain environment-specific server addresses or credentials.

## Runtime behavior

Contract-routed device events use a universal envelope containing `apiVersion`,
`eventId`, `contractHash`, `occurredAt`, and `payload`. Device and route identity
come from the Zenoh topic; schema and encoding come from the assigned contract.
The backend:

1. verifies the provisioned device and topic identity;
2. resolves the route from the assigned contract;
3. checks direction, schema, payload size, event ID, and contract hash;
4. stores the canonical event idempotently;
5. extracts declared typed metric samples;
6. makes those metrics available to analytics and rules.

Command inputs and route selection are also validated from the materialized
contract. Firmware artifacts can target blueprint revisions, and OTA
eligibility is checked against the device's assigned revision.

## Compatibility surfaces

The blueprint-only removal is in progress; see the
[implementation plan](../design/blueprint-only-migration-plan.md). Fixed Protobuf
telemetry is no longer ingested. Remaining legacy read views, device-type models
and client interfaces are pending removal, not supported compatibility surfaces.

## Rule metrics

Rule metric fields currently use `streamKey./exact/json/pointer`, for example
`environment./temperature`. Pointer segments are not converted to dots, so
`environment./a/b` and `environment./a.b` identify different fields. Semantic
labels do not add implicit rule aliases. Contract integer observations retain
their integer type during rule comparison and in alert/webhook values. Public
structured rule selectors are still being migrated.

## Location bindings

A blueprint can declare `spec.location` to bind one stream's numeric JSON field
paths to a WGS84 position. For example, when `position` declares numeric
`/coordinates/north` and `/coordinates/east` fields:

```yaml
location:
  stream: position
  latitudePath: /coordinates/north
  longitudePath: /coordinates/east
  coordinateSystem: wgs84
  unit: degrees
  maxAge: 30s
```

Contract-event geofence evaluation reads both coordinates from the same event.
Missing, out-of-range, future-dated and expired observations are ignored; `(0, 0)`
is valid. The binding and freshness limit participate in the contract hash and
revision change analysis. Map/latest-position persistence is still being migrated.
