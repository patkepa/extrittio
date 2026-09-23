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
2. Continue workstream 1 with PostgreSQL firmware CRUD/OTA, tenant isolation
   and concurrency coverage. The device CRUD/deletion, analytics and
   location-batch regressions now run against real PostgreSQL.
3. Finish backend contracts in workstreams 2–4 before updating their consumers.
4. Finish web consumers after backend contracts settle, then run the backend/web
   integration matrix. Keep the PR draft until the gates pass.
5. Record evidence and remaining blockers here as work completes. Do not keep
   appending contradictory completion claims to the historical log.

## 1. PostgreSQL runtime coverage and transactional correctness

Primary areas: `crates/backend-postgres/src/{devices,events,analytics,firmware,
ci_ingest}.rs`, corresponding Turso adapters, and shared adapter tests.

- [x] Test fresh-baseline device creation with a published revision, assigned
  contract, configuration and certificate; cover get/list/search/update and
  single/bulk deletion. Resolve foreign-key ordering atomically if needed.
- [x] Test PostgreSQL typed event/sample ingestion, latest/history queries,
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
Generic hourly rollups and coordinated retention now exist. The remaining gate
is deeper behavior and scale verification, especially revision compatibility.

- [ ] Define and implement aggregates keyed by tenant/device, revision semantics,
  stream, exact field path and time bucket. Support declared aggregates and
  preserve count/sum/min/max and timestamped latest as applicable.
- [x] Implement coordinated raw-event, typed-sample and aggregate retention in
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

- [x] Replace remaining public string-only selector assumptions with structured
  blueprint/stream/exact-path identity and contract-aware validation.
- [ ] Reject ambiguous paths, unknown declarations and invalid type/operator
  combinations; preserve exact integer comparisons and missing-value semantics.
- [x] Resolve historical samples against their originating contracts/revisions,
  not merely the currently assigned contract. Do not merge incompatible units
  or value types just because field paths match.
- [ ] Verify multi-stream evaluation, geofence freshness/dwell, revision changes
  and durable external actions with generic payloads on both adapters.
  The web geofence form currently sends `zone_id`, `zone_state` and
  `dwell_seconds`, but the HTTP condition DTO drops `zone_id`, creation rejects
  the geofence trigger, and runtime evaluation only handles entry. Align the
  public contract, validation, storage, evaluation and form before counting
  geofence coverage.

Gate: at least two materially different blueprints and incompatible revisions
work without reserved sensor names or fabricated zero values.

## 4. Web completion and visual verification

- [x] Expire map locations client-side at the declared freshness deadline,
  including between polling intervals; preserve event/contract provenance.
- [ ] Finish revision-aware telemetry/history and structured rule controls after
  their backend contracts are settled. The rule metric picker now emits exact
  stream/JSON-pointer keys; the public condition DTO and contract-aware rule
  validation still need structured selector work.
- [x] Review graph `deviceType*` presentation names and declared-connection type
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
  and duplicate-version rollback. Twelve concurrent attempts to publish one
  version produce exactly one firmware row. Turso already performed these
  reads inside its writer transaction.
- The complete PostgreSQL adapter suite, including the ignored fresh-baseline
  and cooldown/rollback regressions on a separate empty database, passed.
  The complete Turso adapter suite passed; the latest count is 25 tests.
- Turso regressions for stale-contract ingestion and fresh/expired locations
  passed. Both adapters now recheck assignment inside the event transaction.
- The location API exposes each observation's contract-derived expiration;
  regenerated OpenAPI/types, frontend formatting, lint, tests and build pass.
  The frontend map unit test covers the exact expiry boundary. Both backend
  adapters now exclude observations at that boundary. PostgreSQL integration
  coverage exercises a 204-ID batch with valid, missing, future, foreign-tenant
  and reassigned devices. Browser evidence remains open.
- A real PostgreSQL analytics query over typed blueprint samples passes for
  the requested tenant and excludes a foreign tenant. Later rollup and
  retention regressions below extend this coverage.
- Both fresh database baselines now contain generic numeric hourly rollups keyed
  by tenant/device/originating revision/stream/exact path/hour. Ingestion updates
  samples and rollups in one transaction. Both adapters pass late-event,
  timestamp-tie, duplicate and nonnumeric regressions; PostgreSQL also proves
  deletion cascade. Analytics now reads completed full hours from rollups and
  partial/current hours from raw samples, preserving count-weighted averages and
  latest ordering. Both adapters retain the same full-hour result after raw
  events are deleted in tests; Turso tests a mixed rollup/raw range.
- Analytics resolves the latest published field declaration and admits historical
  revisions only when exact stream/path, value type, unit and aggregate
  declarations match. The catalog still presents only the latest revision.
  Both adapters keep historical rollup reads available after reassignment.
  Full incompatible-revision API integration and larger dataset verification
  remain open.
- Both baselines now retain delivery receipts separately from raw events, and
  ingestion stores the receipt, event, typed samples and rollup in one
  transaction. An hourly worker advances durable raw/rollup watermarks and
  prunes all three data classes atomically; expired arrivals and analytics
  ranges return explicit errors. Disposable PostgreSQL and Turso tests cover
  duplicate replay after raw deletion, partial-hour protection, rollup
  preservation, subsequent expiry, stale-arrival rejection and retry after a
  transaction rollback. Explicit raw-history queries now report expiration
  from a consistent snapshot in both adapters. The fresh PostgreSQL baseline
  constraint regression also passes with the retention tables. Turso reopen and
  a fresh PostgreSQL connection pool preserve the watermark. Twenty concurrent
  ingestions raced a prune pass on each adapter without losing or duplicating
  rollup counts. Large-data verification remains open.
  Do not treat workstream 2 or 3 as complete.
- Both-adapter backend compilation; the latest local core and Turso suites
  passed with 54 and 25 tests respectively.
- PostgreSQL 17: actual Diesel baseline apply/reapply and schema constraints;
  real cooldown writer/reactivation/rollback tests passed on disposable storage.
  A fresh-baseline firmware/OTA test covers CRUD, blob metadata, tenant isolation,
  invalid artifacts, incompatible revisions, deployment creation and terminal
  status transitions.
- The rule picker preserves exact JSON pointers. Frontend typecheck, 39 tests
  and production build pass on the current implementation; CLI regressions
  passed in earlier steps. Re-run them for the eventual final tree.
- Metric history now joins each sample to its originating contract and revision
  in both adapters, returns the field's original label, unit and presentation,
  and serializes int64 as decimal text. Real PostgreSQL and Turso tests cover
  incompatible revisions sharing a path; web history separates their charts
  and table columns. The frontend suite passes 39 tests, with browser QA open.
- Rule conditions now use a tagged public selector carrying blueprint, revision,
  stream and exact JSON pointer for metrics. Both fresh database adapters persist
  and reload that identity; PostgreSQL and Turso tests also check tenant isolation.
  Runtime telemetry compares the event's originating revision before evaluating
  a rule. The web editor emits and round-trips structured selectors, including
  dotted stream keys and escaped pointers; OpenAPI/types were regenerated.
  Geofence creation now validates tenant-owned zones and one state with optional
  inside dwell. Focused runtime tests cover dwell and exit transitions. Full
  adapter-backed rule/action and browser flows remain open. Checks on this slice:
  `cargo test -p extrittio-rule-engine -p extrittio-backend-core -p extrittio-backend --lib`
  (71, 55, 60 passed), `cargo test -p extrittio-backend-turso` (25 passed
  before the new focused selector test, which passed), the new PostgreSQL rule
  selector test on a fresh disposable baseline, and web typecheck, 42 unit tests
  and production build.
- Firmware/OTA required-revision schema regression passed; OpenAPI/types updated.
- A disposable PostgreSQL 17 container is available for the current execution
  pass; no existing development database was deleted.

## Deferred scope

The iOS app, C SDK, Arduino, shared Protobuf, and embedded producers remain on
their own migration track. This PR does not claim an end-to-end hardware release
or old-client compatibility. Backend/web integration and tenant/transaction
correctness remain merge-critical. Generic retention is required for sustained
operation; rollups are required for the backend analytics target.
