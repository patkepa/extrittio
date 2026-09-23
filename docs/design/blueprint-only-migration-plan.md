# Blueprint-only device and telemetry migration

Status: implementation in progress. Fresh baselines are consolidated; remaining
work is tracked in the [completion and resume plan](blueprint-only-completion-plan.md).
The progress log below is historical and includes superseded intermediate states.
Created: 2026-09-15. Branch: `plan/blueprint-only-migration`.

Scope update (2026-09-22): this PR's completion gate covers the backend and web
console. iOS, C SDK, Arduino, shared Protobuf and embedded producer migrations
are deferred by explicit user direction. The broader client and protocol gates
below describe the original target, not requirements for this PR. The backend
does not provide compatibility for those clients' retired payloads.

Planning assumption confirmed by the user: there are no production databases.
Implement a clean break. Existing database contents, old clients, wire formats
and public APIs do not require backward compatibility or data migration.

This is a replacement of the development model, not a production migration.
Breaking changes are expected: delete retired interfaces and update maintained
callers directly. Do not introduce transitional schemas, compatibility aliases,
dual reads/writes, legacy decoders, backfills, or staged cutover work. Build and
test against empty databases initialized from the new baselines. Recreating any
existing local database is a separate, explicitly authorized destructive action.

This constraint takes precedence over any earlier compatibility proposal.
Scope decision: remove legacy support rather than migrate legacy data. Delete
the old SQL migration chains and supply one blueprint-only baseline per database
engine. Update every maintained client, generated API, fixture and document to
the new contract directly. Do not retain old fields or endpoints as deprecated
aliases, even temporarily. No implementation or test work is allocated to
upgrading old installations or preserving old payloads.

References below to revision compatibility mean compatibility between new
blueprint revisions and firmware, never support for the retired device model.
No work package depends on preserving or upgrading an existing installation.

Acceptance is based exclusively on the blueprint-only system booting and
working from empty storage. Do not spend implementation effort making an old
database, client, API payload or cache work with the replacement. Remove those
paths outright; update maintained consumers to the new contract in the same
change. Any legacy dependencies noted in the progress log below are unfinished
removal work, not approved compatibility requirements or intended end states.

## Implementation progress

- Executed the actual Diesel fresh-baseline integration test against a new
  localhost-only disposable PostgreSQL 17 database: the sole baseline applies,
  rerunning migrations is a no-op, and auth-epoch/zone constraints pass. Added
  and ran a PostgreSQL regression through the real cooldown writer and alert
  transition function, verifying monotonic timestamps, reactivation clearing
  and transactional rollback against that baseline. Both tests roll back their
  schema/fixtures; the PostgreSQL skill guided the test's transaction/locking
  setup. These close the baseline-lifecycle and cooldown runtime checks only;
  device CRUD, event/location/analytics/firmware runtime coverage and full-system
  acceptance remain outstanding.

- Removed the obsolete cooldown-reset table and all dependent indexes, foreign
  keys, Diesel/checkpoint declarations and reactivation writes from both fresh
  baselines/adapters. Eliminated the constant-true decision switch and its dead
  reset predicate: cooldown writes now directly upsert the greatest timestamp.
  Reactivation still clears cooldown state in its transaction. Added a real
  fresh-baseline Turso regression proving timestamp monotonicity, reactivation
  clearing and rollback; all 22 Turso tests and both-adapter compilation pass.
  The PostgreSQL skill informed dependent-object removal and preservation of
  the existing transactional locking contract. PostgreSQL runtime execution of
  the revised baseline/alert path remains unverified; its source exclusion test
  now prohibits the removed table.

- Removed raw unversioned rule-action decoding and the deferred cooldown
  replay operation from the delivery application, alert application, repository
  port and both adapters. The outbox requires its version-1 envelope and rejects
  runtime-only cooldown/zone mutations before delivery; ingestion still commits
  those mutations transactionally. Added tests for valid envelopes, raw payloads,
  missing/unsupported versions/actions, metadata mismatches and runtime mutation
  enqueue/decode rejection. Both-adapter compilation, all 49 core tests and all
  21 Turso tests pass. The now-unused
  cooldown-reset watermark table and constant-true SQL decision switch still
  require cleanup; no old cooldown delivery path remains authorized by this plan.

- Removed the legacy zone-entry outbox replay operation, its application and
  worker dependency, and its repository implementations. Removed the handoff
  table, indexes, foreign keys, Diesel declarations, checkpoint inventory and
  live-observation bookkeeping from both adapters/baselines. Current zone-entry
  mutations remain inside the ingestion transaction; outbox encoding/decoding
  rejects deferred zone-state mutations. Added enqueue rejection and baseline
  exclusion regressions. Both-adapter compilation, all 47 core tests and all 21
  Turso tests pass, including fresh-schema initialization/reopen and backup tests.
  PostgreSQL fresh-runtime verification after this removal remains pending.
  The PostgreSQL skill informed removal of the table's dependent schema objects.
  Legacy cooldown replay and raw outbox-payload decoding still need removal;
  this change does not claim the entire rule-delivery path is blueprint-only.

- Required blueprint revision identity in firmware and global OTA read records,
  PostgreSQL models/Diesel declarations, Turso decoding and HTTP responses.
  OTA matching now takes a required artifact revision; absent device assignments
  still reject deployment. Optional list filters remain intentional. Regenerated
  OpenAPI/frontend types and removed missing-revision presentation branches from
  firmware lists, OTA lists and bulk-action selectors. The React skill kept this
  a direct render-only change without introducing derived state. All 46 core and
  21 Turso tests, the required/nonnullable API schema regression, frontend
  typecheck and all 35 frontend tests pass. The API generator compiled with both
  database adapters. Browser and full PostgreSQL adapter runtime checks remain
  pending, along with the other cross-system migration gates.

- Made the firmware blob read contract's object-storage key required across
  core and both adapters, matching the fresh database baselines. Removed the
  keyless download branch and optional-key deletion branch; blob deletion now
  always attempts object cleanup when its backend matches. The download test
  checks object bytes, backend mismatch and size mismatch without a keyless or
  inline-data fixture. That regression, the fresh-baseline Turso scoped-key and
  object-firmware create/read/delete test, and backend compilation with both
  database adapters pass. Required firmware revision read fields remain pending.

- Replaced PostgreSQL's 31-migration chain (62 SQL files) with one fresh
  blueprint baseline. It excludes device types, fixed telemetry/rollups/latest
  state, cached coordinates, observed network hosts and inline firmware data.
  API-key scope references blueprints; firmware requires a blueprint revision
  and object storage; assignment/event/sample foreign keys include tenant and
  device identity. Removed the retired rule target and permission seeds.
  PostgreSQL blob models/inserts no longer contain inline data and require an
  object key. Deleted old upgrade/backfill tests, replacing them with baseline
  exclusions and fresh-schema constraint assertions. The baseline applied to an
  empty isolated PostgreSQL 17 database, its explicit down script succeeded,
  and corrected reapplication plus SQL auth-epoch/zone-uniqueness assertions
  passed. Adapter compilation and the source regression pass. The new Diesel
  lifecycle integration test is explicitly ignored unless invoked with a
  disposable database URL; it has not yet run. Legacy rule-zone handoff state,
  generic metric rollups/retention, required firmware read-model fields and full
  adapter runtime verification remain outstanding. Baseline consolidation is
  progress, not proof that the complete schema is free of legacy behavior.

- Connected the fleet map to contract-derived batch location reads. Requests
  deduplicate IDs, cap batches at 500 and concurrency at three, support query
  cancellation and stop scheduling after a failed batch. The cache key includes
  blueprint revision identities. Map markers/panel navigation use nested location
  records with event/contract provenance; deleted the optional cached-coordinate
  cast and remaining frontend `latest_latitude`/`latest_longitude` fields. Added
  loading/error states, telemetry permission gating and observation timestamps.
  Unit tests cover origin, invalid/foreign coordinates, missing observations,
  batching/concurrency and failures. Build and 35 frontend tests pass. Browser
  verification, client-side expiry between ten-second refreshes and fleet-load
  testing remain pending; query-time freshness is enforced by the backend.

- Added a contract-derived batch location repository operation in both adapters
  and `POST /api/v1/devices/locations/latest`, with explicit nonempty device IDs,
  a 500-ID request bound, deduplication and device/telemetry authorization. Each
  adapter uses one SQL query with current assignment, tenant/device/event identity,
  declared stream/paths/units, freshness and coordinate bounds. Missing locations
  are omitted. OpenAPI/frontend types are regenerated. Core input-bound and HTTP
  decoding regressions pass; the Turso query regression covers origin coordinates,
  missing IDs, stale/future/invalid observations, tenant isolation and reassignment.
  Turso JSON-array membership exposed planner failures; parameterized ID lists
  pass the same test without per-device queries. PostgreSQL runtime verification,
  multi-device/load coverage and frontend map integration are still pending.
  The complete 21-test Turso suite and regenerated frontend typecheck pass.

- Migrated CLI device decoding and JSON/table output to blueprint identity,
  including revision/key and optional presentation metadata. JSON retains
  declared connections and no longer emits cached coordinates or type fields.
  Corrected the API-key table heading and root-help fixture, and replaced old
  device response fixtures in creation/provisioning regressions. Added JSON
  identity and list/detail table assertions. No legacy device fields remain in
  CLI source. Fleet-map locations, iOS consumers and complete runtime validation
  remain outstanding.

- Migrated web device tables, command palette, overview, fleet graph labels,
  toolbar and CSV export to the new blueprint response fields. Renamed the tag
  component/styles to blueprint terminology, retained optional presentation
  metadata with generic visual defaults, and exposed the assigned revision in
  the overview. Removed the graph's unused hard-coded family abbreviations.
  Frontend typecheck/build and all 32 tests now pass. Browser verification,
  generic graph presentation naming and contract-derived fleet-map locations
  remain pending. In particular the map still reads removed cached coordinates
  through an optional-field cast; a green typecheck does not validate that path.

- Replaced core/device HTTP read identity with blueprint ID, revision ID, key,
  name and optional revision presentation metadata. PostgreSQL and Turso now
  join devices through tenant/device-bound assignments/contracts to revisions
  and blueprints for get/list/search/selection and write responses. Removed
  core device-type records and PostgreSQL device-type model/schema declarations,
  type IDs and cached coordinates. Device location is no longer in the response.
  Backend both-adapter compilation and HTTP response/request tests pass.
  A fresh-baseline Turso regression passes device create/get/list/search/update,
  assignment reads, tenant isolation, invalid-revision rollback and deletion.
  It exposed contract-dependent deletion ordering; single/bulk Turso deletes now
  remove assignments/events before cascading contracts, in the same transaction.
  OpenAPI/frontend types are regenerated: frontend typecheck currently fails at
  old device-type display consumers, which must now be migrated. The map's loose
  cached-coordinate projection also requires replacement with contract location
  reads; do not replace removed fields with null compatibility placeholders.
  CLI/iOS device response consumers and PostgreSQL runtime/baseline verification
  remain outstanding. This is an intentionally incomplete cross-consumer change.

- Removed retired device-type permissions from the core catalog/implication
  rules, backend authorization policy and frontend permission model. Analytics
  and fleet graph navigation no longer require the deleted domain's read
  permission. Runtime owner seeding through `Permission::all()` now excludes
  these keys. Added regressions proving old keys grant no blueprint permissions;
  blueprint management still implies blueprint read. Frontend build and 32 tests
  pass. Old PostgreSQL SQL seed references await deletion with the old migration
  chain. Device identity/read-model replacement remains outstanding.

- Removed provisioning's default-device-type resolver, the complete device-type
  application, and type IDs from core/PostgreSQL create records and both adapters'
  device inserts. Deleted unused device-type CRUD ports/DTOs, both CRUD adapters,
  composition wiring and PostgreSQL insert/update type models. Provisioning now
  prepares the requested blueprint contract directly with no fallback type.
  Device read records/joins and PostgreSQL's old baseline still depend on the
  retired schema: fresh-baseline provisioning is NOT yet end-to-end functional.
  Replace those reads and schema definitions next; do not add a default type.
  Both-adapter backend compilation and all 44 core unit tests pass after removal;
  these do not verify the unfinished device read/provisioning path.

- Deleted the final device-type HTTP listing module, router/domain registration,
  OpenAPI handler/schema/tag, core list application method and public application
  accessor. The API regression now requires every device-type route and catalog
  schema to be absent. Regenerated OpenAPI/frontend types; the regression and
  frontend typecheck pass. The internal provisioning resolver, repository ports,
  permissions and device read-model joins remain removal work, not compatibility
  support. A stale CLI root-help fixture also still mentions the removed command.

- Removed analytics device-type filtering from core/HTTP requests, PostgreSQL
  and Turso SQL/bindings, and the web query controls. Blueprint identity remains
  explicit in the metric selector; fleet/device filters and contract-based
  eligibility remain. Unknown scope fields are rejected. Deleted the last web
  device-type query hook/API/cache key and unused catalog type alias. Regenerated
  OpenAPI/types. Both-adapter compilation, HTTP rejection test, fresh-baseline
  Turso analytics test, frontend build and 31 frontend tests pass. The Turso test
  covers arbitrary metrics, unassigned devices, scope filters and tenant/blueprint
  isolation without a device-type table. PostgreSQL runtime and browser checks
  remain pending, as do the device-type HTTP list and device read identities.

- Removed the fleet graph's device-type catalog request and graph-builder input.
  External connection visuals no longer look up legacy catalog records, and
  catalog refreshes no longer rebuild graph state. Frontend typecheck/build and
  all 31 existing tests pass; browser verification is pending. Managed-device
  response labels and static connection visual heuristics still need review
  during the device identity migration. Analytics is the remaining consumer of
  the device-type list hook.

- Removed device-type requests and target controls from the rule editor/list.
  Rule lists resolve blueprint names through the blueprint query; device options
  use name/ID rather than device-type labels. Corrected the frontend rule target
  union and typed request target, with regression coverage for blueprint IDs and
  rejection of unsupported targets. Fleet graph and analytics remain consumers
  of the old device-type list; rendered/browser rule verification is pending.

- Removed device-type management HTTP writes, request DTOs, OpenAPI operations,
  core management methods and the web settings editor/navigation/mutation hooks.
  The list endpoint/read hook is still used by fleet graph, rules and analytics;
  it remains explicitly pending along with the internal device-creation lookup
  and device read joins. No replacement device-type management surface exists.

- Removed device-type selectors from public device create/update DTOs and the
  core provisioning input. Requests reject retired selectors and fixed sensor
  fields rather than ignoring them. Removed device-type mutation from the core
  update record and both adapters. Device creation still internally looks up a
  default device type and device read models/joins still depend on it; those must
  be replaced before device provisioning works against the new Turso baseline.
  Delete that lookup; do not restore its seed or introduce a placeholder type.

- Deleted the remaining fixed telemetry maintenance application/repository,
  adapter modules and SQL, worker scheduling and composition ports. Removed
  retired PostgreSQL Diesel declarations for fixed raw/latest/hourly telemetry
  and dated partitions. Database checkpoint scheduling now uses an explicit
  local-engine capability, not the retired telemetry partition capability.
  Generic event/metric rollup and retention replacement is still required:
  there is currently no device-metric retention worker. This is an incomplete
  implementation state, not a completed maintenance acceptance gate.

- Removed the hard-coded built-in device-type catalog, bootstrap repository
  seed methods in both adapters, and startup/service initialization calls.
  Startup no longer tries to insert device types into the new Turso baseline.
  Blueprint publication remains explicit rather than inventing default families.
  A fresh-baseline Turso regression verifies first-owner idempotency, role
  assignment, auth epochs and stable JWT-secret storage without a device-type
  table or automatically created blueprint.
  Retired device-type permissions and the remaining device creation/CRUD domain
  are still outstanding.

- Consolidated Turso's fourteen migrations into one fresh baseline and deleted
  the thirteen incremental scripts. The baseline no longer creates device types,
  fixed telemetry/rollups/maintenance state, cached device GPS fields, retired
  network-host storage or database firmware bytes. It directly defines blueprint
  key scope, revision-scoped firmware uniqueness, object-only blobs, current auth
  epochs and outbox idempotency. Contract/event foreign keys include device and
  tenant identity. Initial creation, repeated migration and reopening are tested.
  Updated logical backup/export to omit retired tables, and object-firmware
  queries to stop selecting/inserting the removed inline byte column.
  PostgreSQL baseline consolidation, generic rollups, device/provisioning queries,
  bootstrap and remaining maintenance/fixture migration are still outstanding.

- Replaced API-key device-type scope with optional blueprint identity in core,
  both adapters, HTTP, CLI and web settings. Unknown/foreign blueprint scopes
  are rejected; request decoding rejects retired fields. The web selector blocks
  creation if blueprint loading fails and generated types define key DTOs.
- Removed firmware creation's compatibility-device-type allocation and insert
  columns. Prepared artifacts retain their validated revision identity. CI now
  requires a published revision, checks its blueprint against the key scope,
  and derives compatibility/update strategy from the revision document. Both
  adapters check tenant-owned revisions, and successful CI inserts update key
  usage. A blueprint-only Turso fixture covers key CRUD, tenant/scope isolation,
  multiple revisions, duplicate versions and derived firmware metadata.
- IMPORTANT: PostgreSQL model/schema declarations and adapter SQL now target
  blueprint-only firmware/API-key columns, but checked-in database migration
  chains have not yet been replaced. The application is not ready to initialize
  or run against those old schemas. No transitional database migration was
  added. Finish clean baselines, full adapter fixtures and PostgreSQL runtime
  transaction/concurrency tests before deployment.

- Removed the fixed telemetry subscriber, transport decoder, core ingress
  application and input DTO. Old telemetry publication is no longer ingested.
- Removed fixed telemetry write DTOs, repository write methods and both adapter
  write implementations, including the obsolete PostgreSQL latest-state upsert.
- Removed built-in sensor fields and their fallback compiler enum from rule
  inputs. Webhooks carry observed metrics; missing/nonfinite metrics cannot fire
  rules, and unrelated stream observations cannot resolve active alerts.
- Removed device-type rule targets, cache buckets, evaluation arguments and
  ingress context fields. Both adapters now refresh rule targeting from blueprint
  and fleet membership only. Rule creation rejects the retired target kind.
- Rule observations now retain integer/float types; large integer counters stay
  exact in comparisons and alert/webhook values. Stream field keys preserve
  exact JSON pointers, and fixed-name rule validation is removed. Structured
  public rule DTOs and contract-aware selector validation remain outstanding.
- Added validated blueprint location bindings, compiled freshness limits and
  same-event WGS84 coordinate extraction for contract-event geofence evaluation.
- Replaced shared Rust `TelemetrySource` with native `EventSource` sampling;
  Linux and Raspberry Pi now require a contract and have no legacy publisher.
  Contract heartbeat timing is authoritative. Raspberry Pi omits unavailable
  readings instead of fabricating zero values. Its provisioning script still
  needs replacement, along with the independent ESP32 client.
- Migrated the simulator to per-device verified contracts and schema-validated
  scenario events, with contract-derived heartbeat timing. Added a complete
  simulator blueprint and pre-network provisioning validation.
- Migrated macOS publication to typed system contract events, mandatory contract
  configuration and contract heartbeat timing. Installation validates contract
  identity. Removed its simulated-location fallback and absent-battery zeroes;
  real fixes expire according to the declared location freshness limit.
- Extracted the verified contract codec into `clients/rust/contract`, shared by
  the native runtime and standalone ESP32 without desktop runtime dependencies.
  ESP32 now embeds a mandatory provisioned response, validates its hash/schema,
  derives identity/endpoint/heartbeat timing from it, synchronizes UTC over SNTP
  and emits native environment events. Removed its fixed telemetry publisher.
  Runtime contract replacement, device resource budgets and hardware validation
  remain outstanding; the reference currently requires contract reprovisioning.
  The ESP32 cross-check reached the ESP-IDF build but its Python dependency
  checker rejects installed `ruamel.yaml` in the generated Python 3.9 environment;
  complete firmware compilation is not yet verified. Native client compilation
  and shared codec tests pass after extraction.
- Replaced the Pi provisioning script's device-type creation and inferred endpoint
  with authenticated published-revision provisioning and contract download.
  Added an offline `--check-contract` preflight before service installation,
  restricted token/contract files, and refusal to overwrite an installation.
  Real Pi/systemd provisioning and TLS credential distribution remain unverified.
- CLI device creation now sends only the published revision and ordinary device
  options, without device-type lookup/creation. Removed device-type CRUD commands,
  selectors and their dedicated response/formatting code. Provision output uses
  revision identity. CLI firmware/key scoping, device list/detail fields, contract
  delivery and ESP NVS generation remain outstanding.
- CLI `provision` now requires a contract output path, downloads/verifies the
  assigned response, checks device/revision/envelope identities and saves without
  overwriting existing files. Output and the NVS endpoint input derive from the
  verified contract; removed the provisioning endpoint override/default. Private
  same-directory temporary files protect incomplete writes. NVS format migration
  and contract recovery/download for existing devices remain outstanding.
- CLI firmware upload/list now use blueprint revision selectors exclusively.
  Removed their device-type request fields and response/display dependencies;
  firmware JSON retains contract compatibility/update-strategy metadata. Backend
  firmware persistence and API-key/device response migrations remain outstanding.
- Removed the device-type next-firmware-version HTTP route, core/repository
  methods, PostgreSQL/Turso queries and unused frontend fetch/hook/cache key.
  Firmware mutations now invalidate the blueprint-version cache (previously
  they invalidated only the retired key). The separate firmware list device-type
  filter and storage dependencies still require replacement.
- Removed firmware list device-type selectors from HTTP/core/adapter queries.
  HTTP query decoding rejects retired fields explicitly. The device OTA tab
  waits for its contract before querying revision-scoped firmware, with no
  device-type fallback; contract failure cannot expose an unscoped selector.
  Firmware response/storage joins and schema columns still need removal.
- Removed the OTA device-type compatibility fallback in both adapters. Deployment
  now requires matching nonempty blueprint revisions on the artifact and current
  device assignment; missing revision metadata cannot deploy. Shared policy and
  Turso transaction tests cover missing/mismatched revisions and tenant rejection
  before deployment/shadow writes. Publishing's compatibility-type allocation
  and the underlying schema still remain to be removed.
- Removed device-type fields from firmware read records, HTTP responses and
  generated API types, and removed the firmware read joins in both adapters.
  Firmware UI labels no longer fall back to device types. A Turso fixture with
  no device-type table verifies decoded metadata and tenant-safe blob selection;
  PostgreSQL list ordering now has an ID tie-breaker. Publishing/storage and
  global deployment identity still require migration.
- Deleted the obsolete firmware database-blob migration worker, its core
  application, repository methods, adapter queries and deterministic legacy-key
  support. Downloads now require object storage and cannot fall back to inline
  database bytes. Retired blob columns and optional storage metadata still need
  removal when replacing the database baselines.
- Global deployment records/API/UI now identify the firmware artifact's
  blueprint revision instead of device type. Removed device-type joins in both
  adapters, made relation joins tenant-explicit and added deterministic ID
  ordering to PostgreSQL deployment lists. Frontend deployment types now derive
  directly from OpenAPI. Firmware creation, CI ingestion and API-key scope
  remain coupled to the old write schema and require coordinated replacement.
- Web telemetry now renders only the assigned contract. Removed the legacy
  telemetry component, fixed device profiles and its unused full-history hook;
  shared time ranges are kept in a generic module.
- Web location now reads contract metric samples and pairs coordinates by event,
  device, stream and originating contract. Metric HTTP responses include contract
  provenance from both adapters. Removed the remaining frontend raw telemetry
  API, hooks and aliases. History under older assignments needs revision-aware
  binding resolution; the current map deliberately displays only its assignment.
- Removed the fixed telemetry history/latest/hourly HTTP routes and response
  DTOs. The contract metric route remains; an OpenAPI regression test guards
  against exposing the retired surface again. The separate legacy location
  endpoint and underlying read/maintenance ports still require replacement.
- Removed the now-unused fixed history/latest/hourly application and repository
  methods from core and both adapters, including their query/rollup DTOs,
  PostgreSQL read SQL and rollup read model. The fixed record remains solely for
  the old location path; legacy rollup writes/retention still await replacement.
- Replaced latest-location HTTP reads with contract-bound metric pairs in both
  adapters: current assignment, tenant/device/stream/path/event identity,
  observation time, coordinate bounds and freshness are enforced. Reads now
  require telemetry permission and expose event/contract provenance. Removed
  the fixed core/PostgreSQL record, location decoders, old read application and
  frontend speed/altitude/heading response assumptions. A Turso query fixture
  exercises origin coordinates, isolation, stale/future/partial/mismatched data,
  freshness boundaries, deterministic ties and reassignment. PostgreSQL runtime
  query verification remains outstanding, as does generic maintenance.
- Remaining work includes structured typed rule selectors, generic latest and
  rollup persistence, removal of legacy read repository models, device-type
  removal, clean database baselines, all client/UI migrations and full-system QA.
  These partial changes do not constitute completion of any full acceptance gate.

## Objective and scope

Make published blueprints and materialized contracts the sole device-family
model. Remove the fixed telemetry pipeline and every device-type compatibility
dependency across the backend, databases, public APIs, provisioning tools, SDKs,
web console, iOS app, tests, generated artifacts and supported examples.

Temperature, humidity, battery and position remain valid measurements when a
blueprint declares them. They must cease to be built-in device fields, required
SDK parameters, fixed database columns or fallback rule values. Platform
identity, tenant isolation, presence, timestamps, command correlation and other
universal control-plane concepts remain platform responsibilities.

The final release has no legacy decoder, dual-write path, device-type API,
fallback UI, or compatibility feature flag. Do not build conversion tools,
backfills, upgrade bridges or staged compatibility releases. Replace the SQL
migration chains with clean blueprint-only baselines for PostgreSQL and Turso;
remove obsolete migrations rather than keeping legacy schema history in the
repository. Git history remains the archive for removed implementations.

## Verified inventory

| Surface | Current dependency | Required destination |
| --- | --- | --- |
| Core telemetry | `crates/backend-core/src/telemetry.rs` and `application/telemetry.rs`: fixed input/write/read fields, GPS, hourly aggregates and maintenance | Contract events, typed metrics, generic metric history/latest/rollups and maintenance |
| Event ingress | `backend-core/src/application/events.rs` extracts contract metrics but populates zero-valued legacy rule fields | Typed contract-only rule input |
| Rules | `crates/rule-engine/src/{types,model,compiler,evaluate,cache}.rs`, core rule snapshots and adapter rule runtime | Structured metric selectors, blueprint targets, explicit missing values and contract-derived location |
| Persistence | Both `backend-postgres` and `backend-turso`: telemetry, devices, bootstrap, device ingress, analytics, rules, firmware, API keys and CI ingest | Equivalent blueprint-only schemas, queries and transactional behavior |
| Transport | `backend/src/zenoh_handler/subscriber.rs`, telemetry handler and common topics/protocol | Contract-routed events only for device measurements |
| HTTP | Backend telemetry/device-types domains, device/analytics/identity/firmware DTOs and OpenAPI | Generic event/metric endpoints and blueprint-based filtering, targeting and authorization |
| Rust clients | `clients/rust/runtime/src/lib.rs` samples `DeviceTelemetry`, with optional contract conversion and legacy publish fallback; platform clients implement it | Contract-native sampling and mandatory provisioned contract |
| C/Arduino | `clients/c/sdk-c` fixed telemetry API/nanopb output, C examples, Arduino `Extrittio.h/.cpp` and custom Protobuf writer | Generic bounded contract-event APIs and blueprint-backed examples |
| CLI/provisioning | `apps/extrittio` device-types commands, devices/provision, API keys and firmware; provisioning scripts | Published revision selection, contract delivery and blueprint scope |
| Web | Telemetry fallback/profiles, device-type settings/hooks/API/tag, location, graph, rules, analytics, OTA and API-key creation | Contract-driven fields and blueprint labels/selectors throughout |
| iOS | Device/device-type/telemetry entities, creation/editing, telemetry/settings/firmware views, repositories and persisted caches | Blueprint/contract models, generic telemetry and clean cache models |
| Documentation/build | Architecture guides, client READMEs, `api/openapi.json`, frontend generated types, wire fixtures and protocol tooling | Only supported blueprint workflows and generated contracts |

This inventory is based on source inspection. No production-data audit is needed.
Before implementation, enumerate remaining references repository-wide, including
hidden CI configuration, deployment assets, fixtures and external client build
manifests. Record each as replace, remove, or legitimate blueprint-defined data.

## Target design decisions

1. Every active device has an assigned materialized contract referencing an
   immutable published revision. Remove `device_type_id`, `DeviceType` and
   device-type targeting rather than renaming them. Blueprint identity groups
   families; revision identity determines schema and firmware compatibility.
2. Reuse `DeviceEventRepository`, `RecordDeviceEvent` and `DeviceMetricSample`.
   Extend these where needed instead of adding another telemetry store. Metric
   identity is blueprint + stream + exact JSON field path; retain event contract
   provenance to distinguish incompatible revisions, units and value types.
   Do not merge historical incompatible revisions merely because paths match.
3. Rule selectors use structured identity, not lossy dotted strings or reserved
   sensor names. Blueprint semantics are validated metadata; ambiguous aliases
   fail validation. Missing/null/non-numeric values never become zero. Preserve
   integers without silently losing precision during numeric comparison.
4. Add blueprint-declared location bindings for the map and geofence consumers,
   using validated field references, coordinate system, units and freshness.
   Derive position from one coherent observation; never pair coordinates from
   unrelated events. A valid `(0, 0)` is a location, not an absence marker.
   Universal map geometry types may remain; fixed device telemetry GPS fields
   and the `has_location` wire convention do not.
5. Generic hourly aggregates are keyed by tenant, device, revision compatibility,
   stream, field path and bucket. Store count/sum/min/max and timestamped latest
   where supported by declared aggregates. Preserve weighting, empty buckets,
   retention boundaries and incomplete-hour protection. Do not average averages
   without their counts. Raw events, typed samples and rollups get coordinated
   retention and indexes in both databases.
6. API keys use explicit blueprint scope. Firmware, CI publishing, rules and
   analytics consume blueprint identity/revisions. Enforce tenant isolation,
   authorization and firmware eligibility directly in the new model.
7. Change server, APIs, SDKs and maintained clients together. Remove old
   interfaces directly, without deprecation periods, maintenance windows or
   support for previous releases. Retired telemetry routes and request fields
   must not be accepted. Initialize development/test databases from the new
   baselines and create fixtures through blueprint-based provisioning.

## Implementation sequence and acceptance gates

Each work package includes its affected unit and integration tests. Merge or
release dependent packages together when removing an interface breaks callers.

### 1. Freeze the contract and inventory all consumers

- Enumerate APIs, database objects, protocol symbols, generated sources and
  device-type policy dependencies from the inventory above.
- Specify generic latest/history/location/rollup responses and structured rule
  selectors; define revision compatibility and contract reassignment behavior.
- Specify location bindings in `crates/device-contract`, including schema,
  compiler and validation behavior. Define blueprint scope for API keys and
  firmware explicitly.
- Specify clean PostgreSQL and Turso baseline schemas and blueprint-native
  bootstrap/test fixtures. No legacy record mappings or preflight tool.

Gate: every active legacy consumer has a named replacement or removal, and the
new schemas contain only the target model.

### 2. Complete generic backend capabilities

- Add generic latest/history and durable rollup/retention capabilities to core
  ports and both adapters. Extend existing analytics instead of creating an
  independent metric identity/catalog.
- Replace fixed rule input, fallback field resolution and fixed alert payloads.
  Migrate geofence evaluation and device location projections to declared
  bindings; retain cooldown, zone dwell/handoff and durable delivery behavior.
- Make event persistence, metrics, latest projections, presence and rule action
  enqueueing atomic where they form one accepted-observation operation.
  Preserve idempotency and tenant isolation; handle contract/fleet reassignment
  races without evaluating against stale targeting.
- Ensure historical queries resolve the event's revision rather than assuming
  the latest published blueprint describes all history.

Gate: generic tests pass for arbitrary fields, absent fields, zero values,
multiple streams, incompatible revisions, late/out-of-order events, duplicate
delivery, reassignment and maintenance on PostgreSQL and Turso.

### 3. Remove device-type dependencies from the domain and HTTP API

- Replace device-type fields in core context, device records, ingress, bootstrap,
  repositories, rule snapshots, API keys, firmware and CI ingestion.
- Make provisioning/creation consume published revisions atomically with
  contract assignment. Validate update/reassignment semantics and preserve
  identity, certificates and configuration overlays.
- Replace list filters, rule targets, analytics scope, firmware selectors and
  key policies with blueprint-based equivalents. Replace RBAC permissions and
  role seeds that refer to the retired device-type domain.
- Remove device-type CRUD and fixed telemetry response DTOs/endpoints once all
  consumers have replacements. Regenerate OpenAPI and frontend API types.

Gate: API and authorization tests demonstrate that removed fields/routes are
rejected, required contracts are enforced, and blueprint scope correctly limits
device access and firmware eligibility.

### 4. Move every supported producer to native contract events

- Replace the Rust runtime's `DeviceTelemetry` sampling interface and optional
  `contract_event` hook with native contract events. Require a valid provisioned
  contract and remove no-contract startup/publish fallback.
- Update Rust platform clients and simulator, C SDK, C ESP32 examples, Arduino
  library/examples and all provisioning scripts. Keep sensor-specific readings
  in blueprint payloads and sample-device implementations.
- Add bounded contract envelope/schema/route handling to constrained clients;
  verify payload limits and memory budgets on their supported toolchains.
- Remove `DeviceTelemetry`, its topic helpers, decoder/encoder implementations,
  generated nanopb artifacts and legacy fixtures. The shared `telemetry.proto`
  also contains heartbeat/commands/shadow/log messages: move retained universal
  messages to an appropriate protocol file and regenerate all consumers.
  Audit common exports, build scripts and `tools/xtask/src/protocol.rs`.
- Update CLI blueprint publishing/selection and provisioning workflows; remove
  device-type commands and arguments rather than retaining aliases.

Gate: all supported clients can provision, emit a validated event, reconnect and
acknowledge contract changes; none can publish fixed telemetry. Verify universal
control-plane interoperability between the updated server and updated clients;
previous client versions are not a compatibility requirement.

### 5. Migrate web and iOS consumers

- Make the existing contract telemetry view the sole web telemetry view. Remove
  fallback on contract 404 and static device-family telemetry profiles.
- Update settings, device tables/tags/editors, fleet graph, analytics, rules,
  map/location, OTA and API-key flows to blueprint identities and declarations.
- Replace iOS fixed telemetry and device-type domain/repository/view models;
  update dependency injection, creation/editing, firmware and cached models.
  Delete obsolete cache models and reset development caches as needed; do not
  implement legacy cache decoding or data conversion.
- Render missing values and unsupported capabilities explicitly. Use contract
  labels/units/precision, including revision-aware historical display.

Gate: browser and simulator checks cover two different blueprints, one with no
temperature/battery/location, missing observations, multiple streams, historical
revision changes, provisioning, rules, firmware and blueprint-native caching.

### 6. Replace database baselines and fixtures

- Replace both migration chains with clean baselines that create only the final
  blueprint-only schema. Remove legacy telemetry/device-type tables, columns,
  FKs, indexes, triggers and maintenance functions from all checked-in SQL.
- Preserve unrelated platform constraints and behavior when consolidating the
  schema: tenancy, authentication, durable actions, object storage, zones and
  other current features must still work on a fresh database.
- Update migration registration, generated PostgreSQL schema/models, Turso
  queries/bootstrap, schema patches and migration-specific tests.
- Rewrite seeds, adapter fixtures and development setup to publish blueprints,
  provision contracts and insert valid contract events. Create multi-revision
  historical test data directly in the new model.
- Initialize isolated empty databases for verification. Document how to recreate
  obsolete development databases; no importer or upgrade path is required.

Gate: each backend initializes from empty storage, restarts successfully and
passes schema/fixture/conformance checks without creating legacy objects at any
point. Removed migrations and their registrations have no remaining references.

### 7. Delete remaining compatibility code and verify the complete system

- Delete the legacy core telemetry ports/applications, adapter modules, ingress
  subscriber/handler wiring, device-type modules, old frontend/iOS/CLI code,
  dead dependencies, seeds and compatibility tests. Retain generic maintenance
  under an accurate module name.
- Replace architecture compatibility guidance and all maintained setup examples
  with the supported blueprint-only workflow. Update deployment/bootstrap tools
  for fresh databases and updated clients.

Gate: no runtime route, schema object, client method, generated public type or
fallback can consume or produce the retired model.

## Verification and completion checklist

- [ ] Core, rule-engine, contract and shared adapter conformance tests pass.
- [ ] PostgreSQL and Turso fresh initialization, concurrency, retention, tenant isolation,
  atomic event/outbox and historical-query tests pass.
- [ ] HTTP/CLI regression tests and generated API drift checks pass.
- [ ] Frontend typecheck, tests, build and browser verification pass.
- [ ] iOS builds/tests and device-detail/provisioning/cache simulator checks pass.
- [ ] Rust/C/Arduino supported client builds and event interoperability pass;
  hardware-only checks have recorded device evidence before release.
- [ ] Old telemetry messages and retired API arguments are rejected explicitly.
- [ ] A complete provision → acknowledge → ingest → history/latest/rollup →
  rule/location → firmware workflow succeeds on both deployment modes.
- [ ] Source scans find no active `DeviceTelemetry`, `TelemetryField` fixed enum,
  `device_type_id`, `DeviceType`, fixed telemetry DTO or legacy topic fallback.
- [ ] Review all remaining temperature/humidity/battery/GPS references: allow
  blueprint documents, sensor implementations, generic geometry and tests of
  declared data; disallow platform fields, reserved-name branches and defaults.
- [ ] Fresh database introspection finds no retired objects, and checked-in SQL
  contains no legacy schema definitions or obsolete migration chains.
- [ ] No compatibility settings, obsolete permissions, dead generated code,
  stale cache models or maintained legacy documentation remain.
- [ ] No conversion tool, backfill, dual-write path, old-client adapter or
  compatibility release mechanism has been introduced.

## Implementation boundaries

No production migration, maintenance window, existing-data preservation or
rollback rehearsal is required. Do not let those concerns delay removal of the
old model. All maintained client families still need an updated implementation;
backward compatibility with their previous releases is not required.

Historical queries, revision changes and retention remain features of the new
system and must be tested with blueprint-native data. They do not imply legacy
data conversion. Use isolated development/test databases for verification and
keep hardware-only validation requirements explicit.
