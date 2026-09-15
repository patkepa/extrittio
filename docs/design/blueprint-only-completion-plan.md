# Blueprint-only migration: completion and resume plan

Status: unfinished draft; implementation checkpoint `cfe46e9`.
Branch: `plan/blueprint-only-migration`. PR: https://github.com/patkepa/extrittio/pull/109.

This is the actionable remaining-work list as of the checkpoint. The original
[migration plan](blueprint-only-migration-plan.md) defines the full scope; its
chronological progress notes include superseded statements. Use current source
and test evidence, not old progress bullets, to decide what is finished.

## Constraints and definition of done

- No production databases exist. Keep clean blueprint-only baselines; do not
  restore migration chains, backfills, dual reads/writes or legacy decoders.
- Do not delete existing developer databases without explicit permission.
  Use isolated disposable databases for tests.
- Preserve every maintained client family in the original plan. Retiring a
  client requires an explicit scope decision, not silently dropping its tests.
- Temperature, humidity, battery and GPS may appear in blueprint payloads and
  sensor implementations, but must not be mandatory platform telemetry fields.
- Complete means all remaining implementation and verification gates below
  pass, including supported clients and both deployment modes. A successful
  build or a search with no matches is not sufficient.

## Resume here

1. Read this file, the original target-design decisions, repository instructions
   and relevant skills. Inspect branch status and PR changes before editing.
2. Start with PostgreSQL device CRUD/deletion under workstream 1. Its current
   single/bulk deletion directly deletes device rows; verify dependent contract,
   assignment and event foreign keys before deciding whether an ordering fix is
   necessary. No deletion fix was started before this checkpoint.
3. Finish backend contracts in workstreams 2–4 before updating their consumers.
4. Finish all producer/mobile consumers and shared protocol together, then run
   the complete integration matrix. Keep the PR draft until the gates pass.
5. Record evidence and remaining blockers here as work completes. Do not keep
   appending contradictory completion claims to the historical log.

## 1. PostgreSQL runtime coverage and transactional correctness

Primary areas: `crates/backend-postgres/src/{devices,events,analytics,firmware,
ci_ingest}.rs`, corresponding Turso adapters, and shared adapter tests.

- [ ] Test fresh-baseline device creation with a published revision, assigned
  contract, configuration and certificate; cover get/list/search/update and
  single/bulk deletion. Resolve foreign-key ordering atomically if needed.
- [ ] Test PostgreSQL typed event/sample ingestion, latest/history queries,
  single and batch locations, analytics and firmware CRUD/OTA against real SQL.
- [ ] Test transaction rollback, duplicate delivery, missing/foreign identities,
  and tenant isolation on both adapters.
- [ ] Verify assignment/fleet changes cannot race ingestion into applying stale
  rule targets or contracts. Verify firmware publishing checks revision and
  API-key scope within the relevant transaction, not only before insertion.
- [ ] Exercise batch locations with multiple valid devices, missing locations,
  stale/future observations, reassignment and fleet-scale request sizes.

Gate: shared behavioral coverage passes on both real engines; no reliance on a
compilation-only check for SQL behavior. Preserve the existing PostgreSQL
baseline lifecycle and cooldown tests.

## 2. Generic metric lifecycle: retention and rollups

Primary areas: core events/analytics ports, both adapters and backend workers.
The fixed telemetry maintenance worker is gone; generic metric retention and
durable rollups are not yet implemented.

- [ ] Define and implement aggregates keyed by tenant/device, revision semantics,
  stream, exact field path and time bucket. Support declared aggregates and
  preserve count/sum/min/max and timestamped latest as applicable.
- [ ] Implement coordinated raw-event, typed-sample and aggregate retention in
  both engines, with appropriate indexes, configuration and worker scheduling.
- [ ] Preserve incomplete-hour protection, weighted averages, empty buckets,
  late events, duplicate ingestion and retry/crash boundaries.
- [ ] Wire analytics/history reads to the generic aggregates and prove retention
  does not delete data before required aggregation is durable.

Gate: boundary, restart/idempotency and large-data tests pass on both engines.
Retention is necessary before sustained unattended operation; rollups are also
required to satisfy the full original plan.

## 3. Structured rule selectors and revision-aware history

Primary areas: `crates/rule-engine`, core rule/event/analytics applications,
HTTP DTOs, generated OpenAPI and frontend rule/history models.

- [ ] Replace remaining public string-only selector assumptions with structured
  blueprint/stream/exact-path identity and contract-aware validation.
- [ ] Reject ambiguous paths, unknown declarations and invalid type/operator
  combinations; preserve exact integer comparisons and missing-value semantics.
- [ ] Resolve historical samples against their originating contracts/revisions,
  not merely the currently assigned contract. Do not merge incompatible units
  or value types just because field paths match.
- [ ] Verify multi-stream evaluation, geofence freshness/dwell, revision changes
  and durable external actions with generic payloads on both adapters.

Gate: at least two materially different blueprints and incompatible revisions
work without reserved sensor names or fabricated zero values.

## 4. Web completion and visual verification

- [ ] Expire map locations client-side at the declared freshness deadline,
  including between polling intervals; preserve event/contract provenance.
- [ ] Finish revision-aware telemetry/history and structured rule controls after
  their backend contracts are settled.
- [ ] Review graph `deviceType*` presentation names and declared-connection type
  descriptors. Remove retired model assumptions; distinguish legitimate external
  connection metadata from managed-device identity before changing it.
- [ ] Run browser checks for provisioning, detail/history, analytics, rules,
  map, firmware/OTA, API-key scope and error/empty states. Include a blueprint
  with no temperature, battery or location and one with multiple streams.

Gate: regenerated types, typecheck, unit tests, production build and browser
evidence all pass. Existing unit tests do not substitute for browser checks.

## 5. iOS migration

Primary area: `apps/mobile-app-ios`.

- [ ] Replace fixed device-type/telemetry entities, request models, repositories
  and dependency injection with blueprint, revision, contract and generic data.
- [ ] Migrate creation/editing, settings, telemetry/history, firmware and API-key
  views and all corresponding fixtures/tests.
- [ ] Delete obsolete persisted cache models/decoders; use new cache contracts,
  without converting old development caches.
- [ ] Build/test and verify device detail, provisioning and caching in Simulator.

Gate: iOS consumes only supported APIs and handles both reference blueprints.
The current iOS source still contains retired device fields and fixed charts.

## 6. Producers, provisioning and shared wire protocol

Primary areas: `clients/c`, `clients/arduino`, `clients/rust`, CLI provisioning,
`crates/common`, and `tools/xtask/src/protocol.rs`.

- [ ] Implement bounded contract-event APIs in C SDK, C ESP32 examples and Arduino;
  replace fixed telemetry encoders and examples with blueprint-native payloads.
- [ ] Remove shared `DeviceTelemetry`, generated nanopb artifacts, fixed topic
  helpers and legacy wire fixtures. Split retained heartbeat/command/shadow/log
  messages out of `telemetry.proto`; regenerate all maintained consumers.
- [ ] Update build scripts, protocol tooling and CI checks for the new artifacts.
- [ ] Complete CLI ESP NVS provisioning, contract recovery/delivery and runtime
  acknowledgement/reassignment behavior for supported constrained clients.
- [ ] Verify all Rust platforms, simulator and provisioning scripts end to end;
  resolve the ESP-IDF toolchain/build issue and validate device memory bounds.
- [ ] Record real hardware evidence for reconnect, TLS, event publication and
  contract changes on supported embedded targets; validate Pi/systemd setup.

Gate: every maintained producer provisions and emits validated contract events,
reconnects and handles contract changes; universal control-plane messages still
interoperate. No producer publishes the removed fixed telemetry format.

## 7. Final repository cleanup and release matrix

- [ ] Audit tracked source, hidden CI, deployment/package scripts, docs, fixtures,
  generated code and dependencies for retired APIs/model assumptions. Classify
  sensor-name matches by meaning, not a blanket text replacement.
- [ ] Update maintained setup/deployment documentation for fresh databases and
  blueprint-based provisioning. Remove stale upgrade and compatibility guidance.
- [ ] Run core, contract, rule-engine, shared adapter, HTTP and CLI suites.
- [ ] Run fresh initialization/restart, retention/concurrency/tenant-isolation
  tests on PostgreSQL and Turso; verify generated API/protocol drift checks.
- [ ] Complete browser, iOS and every supported producer build/runtime gate.
- [ ] Demonstrate publish → provision → acknowledge → ingest → latest/history/
  rollup → rule/location → firmware/OTA on both deployment modes.
- [ ] Review every checkbox in the original completion checklist with evidence;
  attach test commands/results and runtime artifacts to the PR before readying it.

## Evidence already obtained (not proof of whole-system completion)

- Both-adapter backend compilation; 49 core and 22 Turso unit tests passed.
- PostgreSQL 17: actual Diesel baseline apply/reapply and schema constraints;
  real cooldown writer/reactivation/rollback tests passed on disposable storage.
- Frontend typecheck and 35 tests passed; builds and CLI regressions passed in
  earlier implementation steps. Re-run them for the eventual final tree.
- Firmware/OTA required-revision schema regression passed; OpenAPI/types updated.
- Temporary database containers were removed. No migration test is left running.

## Priority versus permission to pause

It is safe to pause development at this draft checkpoint. It is not a complete
replacement release: maintained clients still depend on removed interfaces.
Functional integration, tenant/transaction correctness and supported-client
updates are release-critical; retention is critical for sustained operation.
Visual naming cleanup and some documentation can be sequenced last, but remain
required for the original no-legacy-leftovers completion goal. Removing client
families or dropping planned capabilities requires an explicit scope change.
