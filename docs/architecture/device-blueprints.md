# Device blueprints and contracts

Status: implemented for new device creation, contract-routed events, analytics,
rules, firmware targeting, and shared Rust runtime provisioning.

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

The compiler and document types live in `crates/device-contract`. A maintained
example is [`blueprints/smoke-sensor.yaml`](../../blueprints/smoke-sensor.yaml);
the matching API creation body is
[`blueprints/smoke-sensor.create-request.json`](../../blueprints/smoke-sensor.create-request.json).

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
`blueprint_revision_id`; `device_type_id` is an optional compatibility field.
Creation validates the configuration overlay and atomically creates the device,
initial shadow, certificate material, materialized contract, and assignment.

`GET /api/v1/devices/{id}/contract` returns the effective contract and current
assignment state. The first valid contract event acknowledges convergence.

Use the web console for blueprint-based provisioning. The administrative CLI's
legacy `devices create` and `provision` arguments still target `device_type_id`
and are not the source of truth for this workflow.

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

Contract-routed device traffic uses a universal envelope containing the device,
route, schema, sequence, timestamp, contract hash, encoding, and payload. The
backend:

1. verifies the provisioned device and topic identity;
2. resolves the route from the assigned contract;
3. checks direction, schema, payload size, sequence, and contract hash;
4. stores the canonical event idempotently;
5. extracts declared typed metric samples;
6. makes those metrics available to analytics and rules.

Command inputs and route selection are also validated from the materialized
contract. Firmware artifacts can target blueprint revisions, and OTA
eligibility is checked against the device's assigned revision.

## Compatibility surfaces

The repository still contains fixed Protobuf telemetry, a compatibility
`device_types` model, legacy telemetry views, and some CLI arguments based on
device types. They support existing clients but are not the model for new
features. Do not add new device-family-specific branches to these paths; new
capabilities should be declared in a blueprint and consumed generically.
