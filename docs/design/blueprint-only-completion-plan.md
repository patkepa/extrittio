# Blueprint-only migration: completion and resume plan

Status: in progress; backend and web completion scope agreed on 2026-09-22.
Branch: `plan/blueprint-only-migration`. PR: https://github.com/patkepa/extrittio/pull/109.

This is the actionable remaining-work list for this PR. The original
[migration plan](blueprint-only-migration-plan.md) describes a broader target;
its chronological progress notes include superseded statements. Use current
source and test evidence, not old progress bullets, to decide what is finished.
This PR completes the backend and web migration. iOS, C SDK, Arduino, shared
Protobuf, and embedded producer migrations are explicitly deferred. Their old
client payloads are not supported by the blueprint-only backend.

## Constraints and definition of done

- No production databases exist. Keep clean blueprint-only baselines; do not
  restore migration chains, backfills, dual reads/writes or legacy decoders.
- Do not delete existing developer databases without explicit permission.
  Use isolated disposable databases for tests.
- Do not claim the deferred clients or shared wire format have migrated. Preserve
  their existing code and tests unless a backend/web change requires an edit.
- Temperature, humidity, battery and GPS may appear in blueprint payloads and
  sensor implementations, but must not be mandatory platform telemetry fields.
- Complete means all backend and web implementation and verification gates below
  pass on both deployment modes. A successful build or a search with no matches
  is not sufficient.

## Resume here

1. Read this file, the original target-design decisions, repository instructions
   and relevant skills. Inspect branch status and PR changes before editing.
2. Start with PostgreSQL device CRUD/deletion under workstream 1. Its current
   single/bulk deletion directly deletes device rows; verify dependent contract,
   assignment and event foreign keys before deciding whether an ordering fix is
   necessary. No deletion fix was started before this checkpoint.
3. Finish backend contracts in workstreams 2–4 before updating their consumers.
4. Finish web consumers after backend contracts settle, then run the backend/web
   integration matrix. Keep the PR draft until the gates pass.
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

## 5. Backend/web cleanup and verification matrix

- [ ] Audit backend and web source, CI, deployment scripts, docs, fixtures,
  generated API code and dependencies for retired APIs/model assumptions.
  Classify sensor-name matches by meaning, not a blanket text replacement.
- [ ] Update backend/web setup and deployment documentation for fresh databases
  and blueprint-based provisioning. State the deferred client limitation.
- [ ] Run core, contract, rule-engine, shared adapter, HTTP and CLI suites.
- [ ] Run fresh initialization/restart, retention/concurrency/tenant-isolation
  tests on PostgreSQL and Turso; verify generated OpenAPI/frontend type drift.
- [ ] Complete browser checks and an API-driven contract-event integration flow.
- [ ] Demonstrate publish → provision → acknowledge → ingest → latest/history/
  rollup → rule/location → firmware/OTA on both deployment modes using the
  supported backend APIs and web console, without relying on deferred clients.
- [ ] Review every in-scope checkbox here with evidence; attach test commands,
  results and runtime artifacts to the PR before readying it.

## Evidence already obtained (not proof of whole-system completion)

- 2026-09-22: `DATABASE_URL` against an isolated PostgreSQL 17 container,
  `cargo test --locked -p extrittio-backend-postgres --test blueprint_devices`
  passed. It covers blueprint-backed create/get/list/search/update, tenant
  isolation, failed-creation rollback, single/bulk deletion and dependent row
  cascades, duplicate event delivery, typed metric history, fresh/stale location
  reads and rejection of ingestion after a contract reassignment. This is only
  part of workstream 1.
- PostgreSQL CI firmware publishing now authorizes the key and revision under
  row locks in the insert transaction. The same disposable-database test covers
  unknown keys, foreign revisions, scoped-key rejection, successful publishing
  and duplicate-version rollback. Turso already performed these reads inside
  its writer transaction.
- The complete PostgreSQL adapter suite, including the ignored fresh-baseline
  and cooldown/rollback regressions on a separate empty database, passed.
  The complete Turso adapter suite passed (23 tests).
- Turso regressions for stale-contract ingestion and fresh/expired locations
  passed. Both adapters now recheck assignment inside the event transaction.
- The location API exposes each observation's contract-derived expiration;
  regenerated OpenAPI/types, frontend formatting, lint, tests and build pass.
  The frontend map unit test covers the exact expiry boundary. Browser evidence
  remains open.
- Both-adapter backend compilation; 49 core and 22 Turso unit tests passed.
- PostgreSQL 17: actual Diesel baseline apply/reapply and schema constraints;
  real cooldown writer/reactivation/rollback tests passed on disposable storage.
- Frontend typecheck and 35 tests passed; builds and CLI regressions passed in
  earlier implementation steps. Re-run them for the eventual final tree.
- Firmware/OTA required-revision schema regression passed; OpenAPI/types updated.
- Temporary database containers were removed. No migration test is left running.

## Deferred scope

The iOS app, C SDK, Arduino, shared Protobuf, and embedded producers remain on
their own migration track. This PR does not claim an end-to-end hardware release
or old-client compatibility. Backend/web integration and tenant/transaction
correctness remain merge-critical. Generic retention is required for sustained
operation; rollups are required for the backend analytics target.
