# Backend refactor execution plan

- **Baseline:** `9e8a529`, pushed on `refactoring/backend-continuation`.
- **Created:** 2026-09-14.
- **Purpose:** Executable work packages for completing the remaining backend ownership refactor.
- **Current execution preference:** Skip running, compiling, adding, and repairing tests for now. Remove tests made invalid by the refactor; keep unaffected tests. Continue production compilation, formatting, and architecture checks.
- **Next package:** R10 — projections, audit, and operational metrics. R01–R09 are implemented; behavioral verification remains deferred.

## 1. Scope and authority

This plan turns the remaining backlog into ordered implementation packages. It
supersedes the work ordering in [the continuation note](backend-refactor-continuation.md)
while using the existing [architecture plan](backend-crate-architecture-plan.md),
[ADRs](backend-crate-refactor-decisions.md), and
[persistence inventory](backend-persistence-contract-inventory.md) as design inputs.
When an older inventory describes already removed code, inspect the current source
and record the correction rather than reimplementing completed work.

The final ownership model is:

| Layer | Owns | Must not own |
| --- | --- | --- |
| `backend-core` | Domain values, authorization, application orchestration, business ports, pure transformations | Axum, JWT decoding, database engines, Zenoh, environment reads, concrete crypto or object-store clients |
| `backend-postgres` / `backend-turso` | Queries, transactions, row conversion, engine migrations, engine maintenance | HTTP responses, application policy duplicated between engines, host imports |
| `backend` | Composition, HTTP and messaging translation, configuration, concrete outbound integrations, worker scheduling, lifecycle | Business decisions hidden in handlers or broadly exposed repositories |
| `apps/extrittio` | Top-level argument parsing, process setup, exit-code mapping | Adapter construction, Turso maintenance implementation, OpenThread orchestration |

Already migrated: zones, roles, users/passwords ownership, API-key management, and
the API-key-authenticated firmware CI ingest operation. User security closure still
needs evidence reconciliation. At the baseline, architecture checks reported
**102 direct handler-to-repository accesses across 21 files and 12 tracked migration
exceptions**. These are starting measurements, not a complete count of all debt.

This plan does not add microservices, a generic unit-of-work framework, a new API,
or new API-key nonce/rotation features. Stronger command delivery, crash-safe
object workflows, compliance audit, and partition-safe rule propagation remain
separate reliability projects unless their scope is explicitly expanded.

## 2. How to execute this plan

Execute one vertical slice at a time. A package can use several PRs, but each PR
must connect its migrated behavior through core and both adapters, switch callers,
and remove the superseded path. Do not leave one database permanently on the old
implementation or create a second pool/engine to support the new one.

For each package:

1. Read its anchors and search all callers, including startup, workers, CLI, and maintenance paths.
2. Record current authorization, tenant scope, response/error precedence, time precision, ordering, retry behavior, and transaction boundaries.
3. Resolve the package's named design decisions. Preserve documented engine differences if changing them is outside the slice; do not silently assert parity.
4. Define core values, application operations, business ports, and typed outcomes. Keep aggregate operations together wherever an invariant requires one transaction.
5. Implement PostgreSQL and Turso using the existing shared engine handles.
6. Move callers to the application operation and map transport inputs/errors in the host.
7. Delete old implementations, helpers, and compatibility exports whose callers are gone. Lower architecture allowances in the same change.
8. Compile affected profiles, check formatting and architecture, and update the execution record below.
9. Remove tests made invalid by changed or deleted interfaces/behavior instead of repairing or replacing them now. Record what was removed and why; do not run or add tests during this implementation phase.

### Test policy for the current implementation phase

This policy overrides test requirements in the linked plans and each package below
for the current refactor. Continue implementation without waiting for deferred tests.

- Do not run or compile tests, add new suites, repair obsolete tests, or expand test coverage now.
- When a refactor removes or changes an interface, behavior, fixture, or mock that an existing test depends on, remove the affected invalid test. Remove its dedicated fixture/mock/helper if nothing else uses it.
- Keep unaffected tests. Remove a whole test file or target only when all of its tests are obsolete; otherwise remove only the invalid cases.
- A broken shared setup helper alone does not make every case obsolete. Inspect the cases individually, preserve coverage of unchanged supported behavior, and defer setup repairs without compiling or running the tests.
- Remove obsolete test-target declarations and explicit CI/task references when deleting their targets. Do not leave commands pointing to deleted files, and do not disable unrelated CI checks or suites.
- Record deleted tests and the reason in the package/PR ledger. Do not treat a test that still covers supported behavior as obsolete merely because it exposes an implementation defect.
- Do not make implementation completion depend on behavioral test coverage. The acceptance lists below are future verification notes, not tasks to execute now.
- R14 is postponed in full, including new test coverage and runner expansion. Resume that work only when the user asks to return to tests.

Use two distinct completion states:

- **Implemented:** new ownership is connected, old path removed, applicable compilation/static checks pass, and known behavior/limitations are documented.
- **Verified:** behavioral, database, migration, compatibility, and operational evidence also satisfies the package's deferred acceptance list.

Implementation may continue with deferred verification. The final release must not
be described as verified while that evidence remains outstanding. Reaching a test
gate is not permission to override the user's instruction or repeatedly ask to run tests.

## 3. Order and dependencies

| Package | Deliverable | Depends on | Suggested PR boundaries |
| --- | --- | --- | --- |
| R01 | Certificates and key protection | Existing identity/core infrastructure | One certificate slice; separate data correction if necessary |
| R02 | Business bootstrap | R01 where CA setup participates | Bootstrap orchestration and adapters |
| R03 | Device types and fleets | Existing tenant context | Device types; fleets |
| R04 | Blueprints and provisioning | R01, R02, R03 | Blueprint lifecycle; atomic device provisioning |
| R05 | Rules and immutable snapshots | R03/R04 catalog contracts | Rule CRUD and snapshots |
| R06 | Alerts and durable actions | R05 | Alert transitions; outbox and action mapping |
| R07 | Commands, shadows, configuration | R04; R06 for shared action dispatch | Shadow/configuration; command state machine and bus |
| R08 | Ingress and time series | R04, R05, R06, R07 | Events/telemetry; logs/presence/maintenance |
| R09 | Remaining firmware and OTA | R01, R04, R07 | Metadata/blob workflow; OTA/deployment workflow |
| R10 | Projections, audit, metrics | Stable domain contracts from R03–R09 | Analytics/dashboard/activity; audit/metrics |
| R11 | Transport and worker completion | Applied incrementally; closes after R01–R10 | HTTP cleanup; messaging and worker cleanup |
| R12 | Runtime, CLI, feature cleanup | R11 and all business ports migrated | Composition/state; CLI/OpenThread; bridge removal |
| R13 | Identity and API-key closure | Can be addressed alongside R01–R12 | Evidence reconciliation; approved input corrections |
| R14 | Future verification work — postponed | Explicit resumption of test work | Runner/CI coverage; behavioral closure |
| R15 | Release and documentation closure | R12, R13, R14 for verified release | Runbooks, artifacts, compatibility/performance evidence |

Follow the table order for implementation. R13 is tracked throughout; R14 remains postponed. Neither requires restarting
completed identity/CI-ingest extractions. R11 work
must happen with each slice, not accumulate as another large handler rewrite.

## 4. Detailed work packages

### R01 — Certificates and key protection

**Anchors:** `crates/backend/src/domains/identity/{cert_service,certificates,certificate_repository,certificate_types}.rs`,
legacy `persistence/postgres` and `persistence/turso` certificate implementations,
`init.rs`, and existing certificate/key compatibility fixtures.

**Implementation:**

- Move certificate records and business outcomes into core. Separate tenant device operations from explicitly system-scoped CA initialization and stored-key maintenance.
- Add a certificate application façade for status, material download/consumption, replacement, and CA initialization where appropriate. Perform permission and identity checks before mutations.
- Introduce narrow host-implemented ports for certificate issuance and key protection. Keep Ring, rcgen, randomness, PEM handling, configuration loading, and concrete encryption outside core.
- Preserve the encrypted envelope and plaintext-legacy read/migration behavior. Keep compare-and-set replacement of stored encrypted material so maintenance cannot overwrite a concurrently rotated key.
- Move certificate persistence into both adapter crates. Preserve consume-once transactional semantics and typed missing/already-consumed outcomes.
- Resolve PI-06: document the canonical stored-key behavior on rotation before making either engine clear or retain material differently. Resolve PI-14's global device-ID assumption at the certificate/ACL boundary; use tenant-qualified identity when uniqueness is not guaranteed.
- Switch HTTP, startup, provisioning, and ACL callers. Remove the host certificate service/repository implementation after its last caller moves.

**Implemented exit:** core owns certificate decisions; concrete crypto remains host-owned; both adapter implementations are wired; no domain code reads encryption configuration from the environment.

**Deferred acceptance:** existing encrypted data decrypts; one-time material cannot be consumed twice under concurrency; rotation/consume races preserve the chosen contract; wrong-tenant access fails; failed encryption/persistence does not expose key material.

### R02 — Idempotent business bootstrap

**Anchors:** `crates/backend/src/init.rs`, `app/boot.rs`, `persistence/bootstrap.rs`,
both bootstrap adapters, and `apps/extrittio/src/commands/service.rs` callers.

**Implementation:**

- Move business seed values and orchestration into core: built-in device types, first owner, and required application configuration records.
- Reuse the existing `DatabaseRuntime` lifecycle separation. Migration, checkpoint, health, backup, and restore must stay operational; do not put them back into a core bootstrap port.
- Load environment/configuration and generate secrets in the host, then pass explicit values or narrow outbound ports into core.
- Resolve PI-15: current owner setup checks whether users exist globally. Preserve this until an explicit change is selected; do not casually turn it into per-tenant owner creation.
- Specify role prerequisites consistently across both adapters, including ordering relative to owner creation and fresh authentication epochs.
- Make each seed operation retry-safe. Define built-in data as insert-only or reconciling before changing existing rows; preserve current insert-only behavior by default.
- Ensure concurrent configuration initialization converges on one persisted value and concurrent owner initialization cannot create multiple initial owners.

**Implemented exit:** host startup invokes explicit application bootstrap after schema initialization; business bootstrap repositories live in adapters; runtime handles remain in the host.

**Deferred acceptance:** empty and partially initialized databases, retries, concurrent startups, existing users in another tenant, role preconditions, and configuration-secret convergence.

### R03 — Device types and fleets

**Anchors:** `domains/device_types`, `domains/fleets`, their repositories under
`persistence/*`, and device handlers that hydrate their values.

**Implementation:**

- Extract device-type and fleet types, validation, authorization, CRUD orchestration, and repository ports into core.
- Keep tenant filtering and tenant-qualified joins in every lookup/mutation, including name-based lookups and relationships returned in device views.
- Define list ordering, pagination limits, exact name handling, duplicate errors, and in-use deletion outcomes from current behavior and the inventory.
- Implement both adapters, remove SQL/row conversions from the host, and replace each route's repository access with its application façade.
- Reuse migrated catalog types for later provisioning; avoid parallel host/core models that survive the slice.

**Implemented exit:** both catalog families are owned end-to-end by core/adapters; remaining legacy callers use explicit compatibility conversion only when a later package owns their removal.

**Deferred acceptance:** cross-tenant same-name records, duplicates, deterministic ordering, in-use deletion, and missing/wrong-tenant mutation outcomes.

### R04 — Blueprints and atomic device provisioning

**Anchors:** `domains/device_blueprints`, `domains/devices`, relevant adapter
implementations, certificate issuance, initial configuration/shadow creation.

**Implementation:**

- Extract blueprint draft validation, publication policy, revision values, and device-contract assignment into core.
- Resolve PI-13's concurrent publish/lost-response retry contract. Preserve draft-version checks; select stable revision/idempotency behavior explicitly before adding a schema key.
- Extract device catalog/create/update/delete/bulk operations. Resolve PI-12 by defining duplicate input normalization and affected-row counts for bulk operations.
- Define one transaction-sized provisioning persistence operation for device identity, initial contract assignment, configuration, shadow, and credential records that must appear together.
- Perform external crypto work through R01 ports. Pass prepared values into the transaction; do not hold a database transaction over a remote call or invent cross-system atomicity.
- Define cleanup and retry behavior for preparation failures, duplicate devices, and persistence failure. Preserve immutable published revisions and tenant relationships.
- Route HTTP and provisioning CLI entry points through the host/core façade and remove the corresponding legacy implementations.

**Implemented exit:** no handler assembles a device by calling several repositories; related persistent records are created atomically on both engines.

**Deferred acceptance:** failure at each persistence step rolls back the aggregate; concurrent publishers and provisioning retries; duplicate IDs; missing/cross-tenant dependencies; unchanged wire/device contract.

### R05 — Rules and immutable snapshots

**Anchors:** `domains/rules`, `domains/rules/rule_engine`, `app/boot.rs`,
`state.rs`, and all rule-cache reads and writes.

**Implementation:**

- Move rule definitions, validation, permission checks, pure compilation/evaluation coordination, and repository contracts into core.
- Persist definitions through adapter-owned queries with stable ordering; reuse migrated zones and catalog contracts.
- Implement ADR-005's host `RuleSnapshotStore`: complete initial load before readiness, immutable snapshots, single-flight reloads, post-commit local invalidation, and independent polling in every process.
- Preserve the specified default interval of 5 seconds and accepted 1–60 second range. Keep the last good snapshot on failure and degrade readiness when its age exceeds two intervals.
- Expose snapshot age/reload metrics and avoid unnecessary compilation when the revision/content fingerprint is unchanged.
- Resolve PI-11's assembled-snapshot consistency contract. Do not treat local cache replacement as authoritative cooldown, alert, or duplicate-action state; those move in R06.

**Implemented exit:** every evaluator obtains definitions through the snapshot abstraction; committed rule changes no longer depend solely on local handler refresh.

**Deferred acceptance:** two instances observe healthy changes within ADR-005's two-interval target; initial failure/readiness; failed reload preservation; no overlapping reloads; representative rule-set cost.

### R06 — Alerts, durable actions, and outbox

**Anchors:** `domains/alerts`, `domains/rules/rule_engine/actions.rs`,
`domains/operations/{outbox,outbox_repository,outbox_types,rule_action_outbox_repo}.rs`, workers.

**Implementation:**

- Move alert transitions, cooldown policy, zone-entry state decisions, and rule-action mapping into core.
- Make mutable duplicate-prevention state database-authoritative through explicit atomic repository operations. Eliminate reliance on a process-local cache for correctness.
- Move durable action/outbox ports and both implementations into their owning layers. Define claim ordering, ownership token/attempt version, retry classification, and conditional completion/failure updates.
- Resolve PI-10: a stale worker must not finish or fail a newer claim. Use the same compare-and-set meaning on both adapters.
- Introduce a versioned persisted action envelope with tolerant reads for existing payloads. Preserve current serialized fields and persisted-format compatibility; keep unaffected fixtures and remove only obsolete test material. Never make queued work unreadable through a Rust type move.
- Resolve PI-02's retention timestamp and tenant scope. Retention becomes explicitly system-scoped or enumerates tenants instead of silently using the default tenant. Resolve relevant PI-03/PI-04 interval and ordering gaps.
- Keep scheduling, webhook/Zenoh delivery, cancellation, and concrete clients in the host; invoke core for claims and transitions.

**Implemented exit:** core owns action intent and alert policy; adapters own durable transitions; workers cannot duplicate or override work solely through in-memory state.

**Deferred acceptance:** concurrent claimers; stale completion/failure; retry exhaustion; serialized legacy actions; simultaneous alert creation; multi-tenant retention and exact cutoffs.

### R07 — Commands, shadows, and configuration

**Anchors:** `domains/commands`, `domains/shadows`, `domains/configuration`,
message consumers, timeout worker, and OTA callers sharing these operations.

**Implementation:**

- Move shadow merge/delta policy, existing configuration reads/atomic JSON merges, command validation, and command transition rules into core.
- Preserve atomic shadow/configuration mutations through their existing transaction locks, and use conditional command-state updates. Preserve PostgreSQL shadow locking added by the recent OTA fixes; do not introduce a configuration version or acknowledgement protocol.
- Implement ADR-003: authorize and validate, persist a `sent` dispatch attempt, publish through host `DeviceBus`, retain the row and return the existing 502 mapping on synchronous publish failure. Do not add automatic republishing.
- Resolve PI-01 using the approved status vocabulary: `sent`, `delivered`, `succeeded`, `failed`, `timed_out`. Keep historical Turso statuses readable through adapter mapping or an additive migration.
- Keep generated wire messages, topic construction, subscriptions, and Zenoh transport errors in the host. Pass domain values through the bus port.
- Share the resulting transitions with OTA/action callers rather than creating a second command/shadow mutation path.

**Implemented exit:** handlers and consumers invoke application operations; database failure prevents publication; state transitions and atomic mutation semantics are adapter-independent where ADR-003 requires convergence.

**Deferred acceptance:** concurrent desired/configuration updates, stale OTA reports, terminal-state transitions, timeout eligibility, legacy states, retained dispatch row on 502, and no publication on validation/database failure.

### R08 — Device ingress and time-series behavior

**Anchors:** `domains/devices/device_ingress_service.rs`, `domains/events`,
`domains/telemetry`, log repositories/types, messaging consumers, maintenance workers.

**Implementation:**

- Convert authenticated transport identity into explicit core device/tenant context. Decode malformed wire input in the host; apply domain validity and relationship checks in core.
- Move event/telemetry/log ingestion and presence updates behind application operations. Keep record insertion, latest-state updates, rule transitions, and durable action intent in the required transaction-sized operations.
- Preserve event-ID deduplication. Inspect global event-ID assumptions and document them; do not silently introduce telemetry deduplication without a usable message identity and selected compatibility contract.
- Resolve PI-07 with separate observed/occurred/received timestamps. Specify who supplies each instant and preserve UTC precision at adapter boundaries.
- Resolve PI-03 read-window inclusivity and PI-04 tie ordering. Document each endpoint's interval rather than applying a broad search-and-replace to comparisons.
- Move hourly rollup and retention orchestration behind an explicit system application façade; keep PostgreSQL partition mechanics in its adapter.
- Resolve PI-09 maintenance partial failure: select and document transaction/retry behavior rather than assuming both engines can use identical physical operations.

**Implemented exit:** message consumers decode/authenticate/translate only; ingestion decisions reach core; time-series storage and partition code are adapter-owned.

**Deferred acceptance:** duplicate events, disappearing devices, tenant/type/fleet mismatch, action-enqueue rollback, exact time boundaries, deliberately different observed/received instants, and repeatable rollup/retention.

### R09 — Remaining firmware, object storage, and OTA

**Anchors:** `domains/firmware/{firmware_creation,firmware_service,firmware_updates,download,firmware_store}.rs`,
remaining firmware repository methods, OTA status consumers, native durable journal integration.

**Implementation:**

- Reuse the completed CI-ingest slice and ADR-009. Do not split its combined persistence operation or silently normalize its documented transaction differences.
- Extract firmware metadata, version suggestions, listing, blob locations, and deployment domain values/ports into core and both adapters.
- Move upload/delete orchestration into the firmware application. Follow ADR-004: put object before metadata; best-effort compensation on metadata failure; delete metadata before best-effort object cleanup; preserve original error precedence and delete response.
- Keep key construction, filesystem/S3 SDK integration, streams, and hashing in host outbound implementations as appropriate. Preserve object keys and the current public download contract.
- Move OTA planning and deployment transitions through core while preserving scoped firmware grants, valid-artifact requirements, command reservations, per-attempt identity, stale-report rejection, boot-health/rollback handling, and atomic desired-command cleanup from PR #94.
- Retain subscription-before-pending-shadow-request ordering and durable native version restoration. These are existing correctness behavior, not cleanup candidates.
- Keep legacy blob migration as an operational workflow using narrow business operations; avoid a new broad adapter escape hatch.

**Implemented exit:** all remaining firmware business operations are adapter-owned and application-driven; the legacy firmware aggregate is deleted; recent OTA behavior is preserved by design.

**Deferred acceptance:** object failures/compensation, backend mismatch, object keys, download grants, stale deployment reports, overlapping attempts, rollback, desired-command cleanup, and durable version recovery.

### R10 — Projections, audit, and operational metrics

**Anchors:** `domains/{analytics,dashboard,activity,audit}` and
`domains/operations/{metrics_repository,server_metrics_service,server_metrics_repo}.rs`.

**Implementation:**

- Define narrow read-model ports and core authorization/filtering for dashboard, analytics, activity, and audit. Avoid replacing the old broad repository collection with a broad query service locator.
- Move SQL, row hydration, grouping, and database numeric conversions into adapters. Define pagination, empty results, ordering, time buckets, and timezone behavior per projection.
- Resolve PI-05's counter/delta aggregation differences, PI-03 boundaries, PI-04 ordering, and applicable PI-09 metrics-deletion atomicity.
- Follow ADR-002: keep access audit best-effort, retain the original HTTP outcome when audit storage fails, and never invent a tenant for unauthenticated requests. Structured unauthenticated access logging stays host-owned.
- Separate operational health/runtime metric collection from tenant business reads. Core must not pull in system inspection or telemetry exporter dependencies.

**Implemented exit:** read endpoints reach narrow application operations; all projection queries are adapter-owned; audit tenant attribution and failure policy are explicit.

**Deferred acceptance:** numeric golden data, empty/boundary buckets, cross-tenant reads, stable pages, metrics deletion failure, audit failure preserving responses, and secret redaction.

### R11 — Finish transport and worker boundaries

**Anchors:** `crates/backend/src/api`, domain route files, messaging consumers,
`app/http.rs`, `app/workers.rs`, `background.rs`, and outbound integrations.

**Implementation:**

- Audit every entry point after each domain slice: allow parsing, authentication extraction, rate limits, DTO mapping, application invocation, and transport response mapping only.
- Move residual orchestration into the owning application rather than into another host service wrapper.
- Preserve middleware order, public error categories, cookies, route/schema names, wire encoding, and subscription lifecycle.
- Give workers narrow application façades and outbound dependencies. Keep scheduling, readiness, cooperative cancellation, and shutdown in the host.
- Move concrete webhook policy/client behavior to the outbound implementation, preserving destination checks across redirects/resolution and safe error mapping.
- Delete direct cache/repository accessors and reduce each handler allowance to zero as its last access disappears.

**Implemented exit:** zero direct handler-to-repository accesses; all worker/message business coordination is application-driven; core dependency checks reject concrete transport/outbound SDKs.

**Deferred acceptance:** HTTP/OpenAPI compatibility, malformed messages, worker shutdown/cancellation, retry classification, readiness, and outbound failure handling.

### R12 — Composition, CLI, feature graph, and bridge removal

**Anchors:** `state.rs`, `database`, `persistence/{factory,runtime}.rs`, `app`,
`apps/extrittio/src/commands/service.rs`, workspace/package manifests, architecture verifier.

**Implementation:**

- Compose the complete core `RepositorySet` in the host from one selected adapter configuration and the already shared engine handles. Remove the old host business aggregate after its final consumer moves.
- Retain lifecycle/maintenance capabilities in operational state; replace broad general `AppState` fields with private, narrow application/runtime substates.
- Expose a small host façade for service, provisioning, and maintenance commands. Move direct Turso/PostgreSQL and OpenThread orchestration out of the app shell.
- Preserve CLI commands, flags, environment variables, output, unsupported-capability errors, and exit codes. Remove any remaining duplicate executable ownership only after inspecting current manifest targets.
- Retain already implemented no-adapter/default-feature behavior. Complete dependency isolation for PostgreSQL, Turso/edge, both-adapter development, and no-adapter tooling builds.
- Delete host-local adapter foundations, row helpers, migration bridges/features, temporary re-exports, and unused dependencies after their references reach zero.
- Tighten verifier rules from bounded exceptions to final forbidden edges; do not replace deleted exceptions with new blanket allowances.

**Implemented exit:** core owns all business ports; adapters own all SQL/rows; host is the composition root; app is a process shell; EX-002 through EX-015 are removed. Resolve EX-001 according to ADR-001, preserving only its explicitly permitted final mapping if applicable.

**Deferred acceptance:** CLI goldens, capability errors, backup/restore, no-adapter OpenAPI generation, production/edge artifact dependency closure, and clean-checkout builds.

### R13 — Reconcile unfinished identity and API-key work

**Implementation:**

- Reconcile ADR-008/PI-17 against existing epoch migrations, session checks, consistent security reads, and replacement-principal fixtures. Do not implement epochs again.
- Identify missing evidence for same-ID/same-version replacement, missing/empty/mismatched epochs, migration backfill, and security-read snapshot consistency. Keep the closure status pending while tests are skipped.
- Resolve PI-16's embedded-NUL username contract in its own focused change. Select a stable rejection or portable representation and record API/error implications before editing validation.
- Retain API-key name-uniqueness differences and ADR-009 ingest transaction differences unless a dedicated correctness slice selects convergence.
- Reconcile the older nonce/rotation plan language with actual product behavior. Record it as deferred/not implemented unless a real existing flow is found; certificate encryption nonces are not an API-key rotation feature.

**Implemented exit:** every remaining identity gap has a current source-backed status and explicit decision; no historical task is incorrectly counted as new implementation.

**Deferred acceptance:** full ADR-008 closure, NUL create/login behavior if changed, key scope/revocation/concurrency, and CI-ingest failure behavior on both engines.

### R14 — Future verification coverage and evidence (postponed)

This entire package is outside the current implementation phase. Do not add or
repair tests, expand test runners, or execute suites now. The following is a future
backlog to revisit only when the user resumes test work. Removing obsolete test
target references during a refactor is routine cleanup and does not activate R14.

**Future implementation after test work is resumed:**

- Update `.github/workflows/ci.yml` and `tools/xtask/src/verify.rs` to include all feature-gated shared adapter contracts, including API keys and new slices. Prefer package-level contract runs over manually maintained target lists.
- Make PostgreSQL contract execution use an explicitly disposable configured database and expose missing configuration as missing evidence, not a passing skipped contract.
- Keep contract fixtures isolated by group/backend so concurrent runs do not delete another suite's data. Never point migration/destructive fixtures at user or production databases.
- Add behavior-oriented shared suites for the cases listed under each package, plus missing core/host compatibility coverage. Rebuild only the useful coverage removed during refactoring, using the final APIs; avoid tests that merely mirror implementation.
- Register each suite in the matching verification lane and record commands, environment prerequisites, revision, result, and unresolved failures.

**Verified exit once tests are resumed:** both engines actually execute all applicable contracts; failure/concurrency/migration evidence is present; identity closure is demonstrated; no success claim depends on tests silently returning early.

### R15 — Release, documentation, and operational closure

Complete documentation and static artifact inspection while R14 is postponed.
Defer smoke tests, runtime benchmarks, migration rehearsals, and other executable
behavioral verification; these do not block implementation or documentation closure.

**Implementation:**

- Update contributor instructions, architecture diagrams, migration/backup/restore guides, deployment profiles, environment documentation, and CLI runbooks to final paths.
- Remove stale plans or label their snapshots historical; link completed work to decisions and verification evidence.
- Regenerate OpenAPI/client types only if their source contracts changed; inspect any diff as a compatibility change, not routine refactoring noise. Preserve protocol artifacts unless protocol input changed.
- Compare dependency graphs and final artifacts with the recorded baseline, using the same profile/platform/toolchain where possible. Record binary size, linkage, startup and representative runtime costs; investigate changes rather than inventing an arbitrary pass threshold.
- Document rollback per slice. Preserve old serialized readers and use additive migrations; if rollback needs a minimum application version or forward recovery, record that explicitly.

**Deferred release evidence:** previous-schema upgrades on both engines, Turso backup restoration, HTTP/device/rule/action/firmware/CLI/shutdown smoke coverage, clean-checkout builds, and artifact/performance comparisons.

**Final verified exit:** all packages are implemented, deferred proof is closed, no temporary migration bridge remains, dependency isolation is demonstrated, and operational documentation matches the shipped behavior.

## 5. Decision register

Reinspect each gap before editing; the inventory is partly historical.

| Existing gap | Owning package | Required disposition |
| --- | --- | --- |
| PI-01 command states | R07 | Apply ADR-003 with readable legacy states |
| PI-02 alert retention | R06 | Select timestamp and explicit system/tenant scope |
| PI-03 time boundaries | R06/R08/R10 | Specify and implement each endpoint's interval |
| PI-04 ordering | R03–R10 | Retain completed identity/zone decisions; finish remaining stable order |
| PI-05 metric aggregation | R10 | Select counter/delta math explicitly |
| PI-06 private-key rotation | R01 | Select one-time stored material semantics |
| PI-07 ingest times | R08 | Distinguish observed/occurred/received instants |
| PI-08 role invalidation | Already implemented | Preserve ADR-007; do not reopen without new evidence |
| PI-09 transaction differences | R08/R09/R10 | Record intentional partial failure; preserve ADR-009 ingest exception |
| PI-10 outbox claims | R06 | Claim ownership and stale-attempt CAS |
| PI-11 snapshots | R05/R06 | ADR-005 plus database-authoritative mutable state |
| PI-12 bulk counts | R04 | Duplicate normalization and affected-count contract |
| PI-13 blueprint publication | R04 | Concurrency and lost-response retry contract |
| PI-14 global device identity | R01/R04/R08 | Explicit uniqueness invariant or tenant-qualified boundary |
| PI-15 bootstrap | R02 | Global emptiness and owner-role preconditions |
| PI-16 username NUL | R13 | Portable validation/storage decision |
| PI-17 session identity | R13/R14 | Reconcile implementation and prove ADR-008 closure |

## 6. Compilation and static checks while tests are skipped

Use the affected subset after each slice; use the complete matrix when composition,
features, or core ports change. Serialize Cargo commands to avoid build locks.

```sh
cargo check -p extrittio-backend-core
cargo check -p extrittio-backend-postgres
cargo check -p extrittio-backend-turso
cargo check -p extrittio-backend --no-default-features
cargo check -p extrittio-backend --no-default-features --features postgres
cargo check -p extrittio-backend --no-default-features --features turso
cargo check -p extrittio-backend --no-default-features --features postgres,turso
cargo xtask architecture
git diff --check
```

Format changed Rust files using repository configuration. Include app/profile
compilation when CLI or deployment features change. Compilation proves type and
feature consistency, not runtime transaction, migration, or compatibility behavior.
Do not run `cargo xtask verify backend` under the current preference: it runs tests.
Do not use `cargo test`, `cargo test --no-run`, or `cargo check --tests/--all-targets`
as a substitute. Check production targets only; identify obsolete tests by inspecting
their dependencies on changed code, then remove only the invalid cases and unused support.

## 7. Execution ledger

Update this table in every implementation PR; do not replace deferred evidence
with a generic “done.” List individual PRs if a package is split.

| Package | Implementation | Behavioral verification | Commit/PR and next action |
| --- | --- | --- | --- |
| Baseline CI ingest | Complete at `9e8a529` | Deferred | Four host profiles and architecture passed |
| R01 | Implemented | Deferred | Core façades and both adapters; ADR-010 |
| R02 | Implemented | Deferred | Core system bootstrap and both adapters; ADR-011 |
| R03 | Implemented | Deferred | Core catalogs and both adapters; ADR-012 |
| R04 | Implemented | Deferred | Core blueprints/devices, atomic provisioning, both adapters; ADR-013 |
| R05 | Implemented | Deferred | Core rules/evaluation, consistent reads, immutable host snapshots, polling/readiness/metrics; ADR-005 |
| R06 | Implemented | Deferred | Core policy/runtime, adapter-owned outbox and durable transitions; ADR-014 |
| R07 | Implemented | Deferred | Existing shadow/configuration behavior, commands/DeviceBus, shared OTA shadow mutation; no new config protocol |
| R08 | Implemented | Deferred | Core ingress/read/maintenance policy, both adapters, explicit timestamps and durable pruning boundary |
| R09 | Implemented | Deferred | Core firmware/object/OTA applications, both adapters, source preservation audit; host signing/storage/transport |
| R10 | In progress | Deferred | Activity application and adapter SQL extracted; remaining projections/audit/metrics next |
| R11 | Partial through completed slices | Deferred | Continue thinning each migrated entry point |
| R12 | Foundations already exist | Deferred | Finish after last legacy domain moves |
| R13 | Existing implementation needs reconciliation | Deferred | Inspect proof and decide remaining gaps |
| R14 | Postponed in full | Deferred by user | No test additions, repairs, runner expansion, or execution |
| R15 | Not started | Deferred | Final docs/artifacts/release evidence |

For each completed PR append: moved behavior, deleted paths, decisions made,
remaining compatibility bridges, architecture count before/after, compilation
results, removed tests and reasons, deferred verification, migration/rollback notes,
and the next package. This makes
the plan resumable from the repository without relying on conversation history.

## 8. R01/R02 implementation record

- Certificate types, policy, tenant and system façades, and crypto ports moved to core. Both adapter crates now own certificate persistence; the host supplies configured encryption/issuance and filesystem/TLS integration.
- Provisioning preparation now calls the certificate application. All four certificate HTTP routes use core, and their direct repository allowance is deleted. The device route loses its certificate repository access as well.
- ADR-010 resolves PI-06 with consume-on-rotation on both engines and records PI-14's schema-enforced global device ID contract. No encryption envelope, HTTP schema, or migration bytes changed.
- Core owns bootstrap policy, built-in seeds, password hashing orchestration, and explicit local setup compatibility. Both adapters own bootstrap persistence; startup and CLI use application façades. ADR-011 records PI-15 and PostgreSQL owner-role creation/permission convergence.
- Removed obsolete host certificate service/helper/type/port files and certificate/bootstrap engine implementations. The legacy business aggregate still holds core ports for host composition until R12.
- Removed invalid core application/repository composition tests and their now-unused CI-ingest mock implementation; removed `backend/tests/cert_tests.rs`, whose cases/setup imported deleted host certificate service/helpers. Removed only obsolete certificate/bootstrap setup/assertion portions of the broad legacy Turso foundation test. Unaffected tests remain; the encryption-envelope fixtures moved with their implementation and were not run.
- R01 compilation passed for independent core/adapters and all four host adapter profiles. Architecture after R01 reports 97 direct accesses across 20 files and 11 tracked exceptions (baseline: 102 / 21 / 12). R02 additionally reduces the default-tenant bootstrap allowance from two occurrences to one.
- R02 independent core/adapter checks, all host profiles, the combined-adapter CLI build, and architecture checks passed. Behavioral tests, concurrency/migration runs, and new test coverage remain deferred. Changes are in the working tree; no implementation commit/push has been requested for these packages.
- R01/R02 next package was R03; see its execution record below.

## 9. R03 implementation record

- Device-type and fleet policy, records, and repository ports moved to core. Both database adapters now own their catalog implementations, using the same shared pool/engine handles.
- Catalog routes plus device/firmware compatibility-type resolution call core applications. Preserved validation/default/whitespace rules and public errors; added transport-independent `InvalidOperation` mapped to the existing 422 response category.
- ADR-012 selects binary name/ID ordering for both engines and records the retained PostgreSQL firmware lookup bridge. Removed every unused helper from that bridge; R09 owns its final deletion.
- Removed invalid device-type/fleet service tests. Deleted the remaining legacy Turso catalog, OTA, blueprint, and analytics test cases whose setup invoked removed catalog implementations; their execution/replacement is deferred under the user policy. Kept the unaffected activity-pagination case and its required setup. Earlier R01/R02 removed parts of the mixed foundation case; R03 makes that entire case obsolete and removes it.
- Independent core/PostgreSQL/Turso crate compilation, all four host adapter profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks pass. Architecture is now 86 direct accesses across 18 files with 11 tracked exceptions, down from baseline 102 / 21 / 12.
- All R01–R03 changes remain uncommitted. No test suites ran. The full plan remains active, with R04 next.

### R04 execution notes (2026-09-14)

- Added core blueprint and device catalog values, ports, and applications; moved both SQL implementations to adapter crates using shared existing handles. Blueprint handlers and firmware revision lookup now call the application façade.
- `DeviceApplication::provision` owns UUID allocation, blueprint compilation, compatibility type resolution, and certificate preparation. The HTTP handler passes translated inputs and its configured endpoint; CLI creation already calls this HTTP route. The database operation requires a compiled contract and atomically creates identity, shadow, contract, assignment, optional certificate, and declared initial configuration.
- ADR-013 resolves PI-13: unchanged latest document/hash publication returns the original revision; changed drafts and stale compatibility baselines return conflicts. PostgreSQL parent/draft locking uses consistent order. No schema key or migration was added.
- ADR-013 resolves PI-12: normalize duplicate IDs before selection limits; both adapters count unique matching tenant-owned rows. Deletion retries count only newly deleted rows. Configuration now materializes compiled defaults/overrides in `device_configs`, in the provisioning transaction.
- Moved update-name validation and timestamps to core. Ingress is split into the explicit host `DeviceIngressRepository` until R08; existing commands retain a narrow core catalog dependency until R07. Removed legacy blueprint and device catalog services/types/SQL.
- Removed the invalid device catalog service test module. Removed `test_device_creation_materializes_and_assigns_contract` and `test_device_ingress_resolves_the_persisted_tenant_identity` from the mixed API suite because they directly invoke ingress methods removed from the catalog port. Removed their dedicated `contract_blueprint_document` helper and unused Prost trait import. Other API cases and endpoint-derivation tests remain unchanged; none were run or repaired.
- Independent core/adapter compilation, all four host feature profiles, and the combined-adapter CLI build passed. Formatting, whitespace, and architecture checks passed. Direct handler-to-repository accesses fell from 86 across 18 files to 61 across 17 files; 11 tracked migration exceptions remain. Behavioral, concurrency, rollback, and migration verification remain deferred by instruction.
- Changes remain uncommitted; R05 is next. R14 remains postponed in full.

### R05 execution notes — application and persistence ownership (2026-09-14)

- Added core rule records, repository contract, and `RuleApplication` with existing authorization, validation, IDs, and update/toggle timestamps. HTTP rule CRUD now calls the application. A host `PublicWebhookUrlPolicy` implements the core URL-policy port, preserving URL/literal-address validation without introducing Reqwest or DNS dependencies into core.
- Moved rule SQL and PostgreSQL helper queries into their adapter crates, using shared existing handles. The host retains only `rule_repo::upsert_cooldown` for the unmigrated alert transaction (R06 owner); obsolete helpers and the unused host active-alert loader are removed.
- Resolved PI-11's source-read contract: PostgreSQL uses one read-only repeatable-read transaction; Turso uses one transaction on a dedicated read connection. Rules, conditions, actions, zone definitions, cooldown hints, and active-alert hints all come from that snapshot. Zone SQL/decoding is reused within the transaction and geometry conversion lives in core. The host no longer combines a rule read with a separate zone read.
- Enabled-rule traversal is tenant/ID ordered. Conditions use group/ID order, actions use ID order, and active-alert duplicate hints choose the last row in created-at/ID order on both engines. Public list ordering retains the prior engine difference (PostgreSQL newest-created first, now with ID ties; Turso name/ID). This does not claim cross-engine list-order parity.
- Removed the obsolete host rule service, record/port modules, and its zone-snapshot conversion test, whose enclosing service/helper boundary was removed. Other rule-engine and zone-adapter tests remain untouched and were not run.
- **Still required for R05:** replace the mutable host cache with `RuleSnapshotStore`; move all evaluators behind snapshot access; add single-flight invalidation and independent polling; validate the 5-second default and 1–60-second interval; preserve the last good definitions; expose age/reload metrics and readiness degradation after two intervals; avoid rebuilding unchanged definitions. Mutable runtime-state authority closes in R06, not through snapshot replacement.
- The repository's transitional `build_cache` result still includes mutable runtime hints, and HTTP still performs local refresh. Those are explicitly intermediate implementation paths, not completion of ADR-005. No schema migration or stored action/wire change was introduced. Transaction/consistency behavior remains unverified while tests are deferred.
- Removed the unused separate zone-snapshot dependency from the core application aggregate; the adapter zone port remains available for its existing consumers and unaffected tests.
- Independent core/adapter compilation, all four host profiles, the combined-adapter CLI build, formatting, whitespace, and architecture checks passed. Direct handler-to-repository accesses fell from 61 across 17 files to 55 across 16 files; 11 migration exceptions remain. No tests ran. Changes remain uncommitted; continue R05 with the immutable snapshot store and core evaluation integration.

### R05 execution notes — immutable snapshots and lifecycle (2026-09-14)

This closes the implementation items left open in the preceding R05 notes.

- `RuleSnapshotStore::initialize` loads a full consistent snapshot before server construction. Every process starts a supervised, cancellable polling worker; the configured default is 5 seconds, with 1–60 whole seconds accepted for all profiles. The environment sample documents `RULE_SNAPSHOT_REFRESH_INTERVAL_SECS`.
- Core `RuleChangeNotifier` fires only after successful create/update/delete/toggle commits. The host uses a coalescing notification, so reload failure cannot change the committed API result. Invalidations received during a reload retain a pending permit. A single-flight mutex prevents overlapping reloads and missed polling ticks are skipped/coalesced.
- Adapters now return unindexed `RuleSnapshotRecords` from the consistent read. Core owns record-to-index construction and tenant-qualified telemetry/status/geofence evaluation through `RuleEvaluationSnapshot`. The host compares a deterministic SHA-256 definition fingerprint before compilation; unchanged content retains the existing index and still refreshes observation freshness.
- Rule definitions live in a shared immutable index. Readers hold immutable snapshot handles; runtime hint updates use copy-on-write and cannot alter already-issued snapshots. A changed definition snapshot preserves current runtime hints, including updates made while the database read was in flight. These hints are explicitly transitional: authoritative cooldown, alert, zone-entry, claim, and idempotency handling remains R06 work.
- Failed reloads retain the last good definitions. Readiness computes freshness on every request and degrades after two intervals, independently of whether the reload worker is waiting on the database. Age is measured from the successful read's start, including its load time. Current metrics expose snapshot age, interval, reload duration, success/failure counters, definition changes, and snapshot freshness.
- Removed the direct host-to-rule-engine dependency, old handler cache refresh helper, and repository bridge. All live evaluator entry points now obtain a snapshot from the host store and call core evaluation. The architecture legacy-edge allowance and its invalid assertion were removed; other verifier assertions remain.
- Removed the remaining `backend/tests/api_tests.rs` target because every case uses shared setup constructing the deleted mutable-cache `AppStateInput` boundary. Its target-specific architecture allowance was removed; no CI/task command references remained. Other rule-engine, configuration, zone-adapter, endpoint, and unrelated suites remain untouched. No tests were executed, added, or repaired.
- Regenerated `api/openapi.json` and frontend OpenAPI types for the additive `rule_snapshots` metrics field, and updated the frontend compatibility type. Generation also incorporates R01's already-corrected certificate-download error description (400 when the key is gone). Existing protocol/action payload shapes and database schemas are unchanged.
- Runtime, multi-process propagation, failure, concurrency, and representative-load measurements remain deferred under the test policy. R05 implementation is complete; verification remains deferred. Changes are uncommitted; continue with R06.
- Validation passed: independent core/adapters, no-adapter/PostgreSQL/Turso/combined host compilation, combined-adapter CLI compilation, OpenAPI generation, frontend type generation and type-check, formatting, whitespace, and architecture checks. Source review confirmed notifications occur only after the four successful mutation paths. Architecture reports 55 direct handler-to-repository accesses across 16 files and 9 tracked migration exceptions (down from 11). No test suites ran.

### R06 execution record — outbox and alert extraction (in progress)

- Moved outbox ports/records, action mapping and versioned serialization, replay authorization/validation, claim validation, and retry classification into core. Both adapters now own claim/retry/replay queries. Workers use the core worker façade and returned claim tokens.
- PI-10 uses fresh opaque tokens on every claim, conditional processing/token updates, and an additional attempt check on failures. PostgreSQL rechecks base-row eligibility while claiming and explicitly orders returned rows; both engines share tenant-ranked batch selection and microsecond lease precision.
- Added backward reads of raw actions and version-1 writes. ADR-014 records the required rollout/rollback reader compatibility and the distinction between claim ownership and exactly-once side effects.
- Moved alert records/port and tenant application operations to core, and alert/cooldown SQL into both adapter crates. Removed the obsolete host alert service/repository/type modules and the final host rule cooldown SQL helper. Alert/outbox handlers no longer access `state.persistence`; their architecture allowances were removed.
- PostgreSQL transition preconditions now run under a row lock. Resolve/reactivate update cooldowns in the alert transaction on both engines, replacing handler-spawned persistence. Bulk transition IDs are deduplicated and locked in sorted order. Core owns transition preconditions; adapter changesets translate the transitions to storage.
- PI-02: retained the explicit all-tenant resolved-at retention operation and deleted the unused creation-time retention helper. Alert PI-03/PI-04: `[since, before)` boundaries and `created_at DESC, id DESC` ordering; PostgreSQL's `since` boundary deliberately becomes inclusive. Summaries explicitly order status/severity.
- No tests were added, repaired, or executed in this slice. Source search found no remaining Rust references to the removed alert/outbox interfaces; no additional test deletion was needed.
- Remaining R06 work: database-authoritative alert creation and rule runtime state, atomic cooldown/zone-entry decisions and action intent, and removal of process-local correctness dependencies. Host ingress still owns its transactional enqueue helper pending the ingress extraction. This is not R06 completion.
- Validation passed: independent core and both adapters, all four host feature profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks. Direct handler-to-repository access fell from 55 across 16 files to 41 across 14 files; 9 tracked migration exceptions remain. No tests ran. All implementation changes remain uncommitted.

### R06 execution record — atomic alert creation and receipts

- Added a core system `AlertWorkerApplication` and explicit rule-alert intent. The durable action ID supplies the new alert ID. Replaced the generic insert port with transactional create-or-reuse semantics and removed the worker's local-map reservation/early return.
- PostgreSQL serializes creators on the tenant-scoped device row. Turso obtains a database writer transaction. Both atomically reuse the newest active/acknowledged alert or insert one, recording which alert fulfilled the delivery.
- Added adapter-owned `rule_alert_deliveries` migrations and PostgreSQL schema declarations. Receipts outlive alert retention and cascade with outbox deletion. Retried creation can return successful absence when its previously delivered alert was removed. Turso logical archives include the receipt table.
- ADR-014 records rollout, no inferred historical backfill, archive-version handling, and receipt loss on PostgreSQL downgrade. No database migration or backup/restore operation was executed.
- Removed the invalid fixed-count Turso `migration_order_and_checksums_match_the_compatibility_manifest` test and PostgreSQL `embeds_the_expected_postgres_migration_chain` / `migration_assets_match_the_compatibility_manifest` tests, plus their now-unused checksum fixtures and dedicated helper/imports. Their frozen migration counts/manifests became obsolete with the new migration. Other migration/backup tests remain untouched; no tests were added, repaired, or executed.
- Still required for R06: authoritative evaluation/cooldown/zone-entry state and action-intent coupling, plus integration of manual reactivation with the duplicate-prevention rules. Creation is no longer gated by local hints, but the overall evaluator is still transitional.
- Validation passed: independent core/adapter compilation, all four production host profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). No tests or migrations were run. Changes remain uncommitted; the full plan remains active.

### R06 execution record — manual reactivation serialization

- Core now identifies transitions requiring an active rule/device slot and exposes an explicit competing-alert outcome. Single reactivation returns HTTP 409 when another alert is active or acknowledged for the same tenant/rule/device. Bulk reactivation skips conflicts and returns its existing updated count; rule-less alerts retain previous behavior.
- PostgreSQL transitions acquire the same device lock as creators before locking/reloading the alert. Bulk transitions lock all target devices in sorted order first, then process sorted alert IDs. Turso obtains its database write transaction before the precondition/conflict check. Failed reactivation leaves alert and cooldown state unchanged.
- Updated the HTTP OpenAPI annotations for the conflict response and bulk skip semantics. Existing historical duplicate alerts are left intact; no data cleanup or migration was performed in this slice.
- Remaining R06 work is authoritative evaluation/cooldown/zone-entry state and action-intent coupling. Manual reactivation now participates in storage-backed active-alert exclusion. Tests remain deferred; no additional obsolete tests were found for this change.
- Validation passed: independent core/adapters, all four host feature profiles, OpenAPI generation and response inspection, frontend type generation/type-check, formatting, whitespace, and architecture checks. Architecture remains at 41 direct accesses across 14 files and 9 migration exceptions. No tests ran, and changes remain uncommitted.

### R06 execution record — explicit evaluation time

- Added explicit-time rule-engine entry points for telemetry, status change, geofence, and cooldown checks. Within an evaluation, webhook timestamps, cooldown writes, and zone-entry timestamps now use the same supplied time instead of independently reading the wall clock.
- Core snapshot entry points require that observation time. Telemetry and heartbeat share it with the corresponding ingress write; contract events share their received timestamp; an offline sweep uses one evaluation timestamp across its candidates.
- Kept the existing rule-engine convenience functions as compatibility wrappers that capture time once and delegate. Existing tests and callers using those functions remain valid; no test changes were needed or executed.
- This removes hidden clock reads from the decision functions used by core. It is groundwork for re-evaluation under transaction locks, not completion of database-authoritative runtime state. The current ingress paths still evaluate against local runtime hints before persistence; that remaining dependency must be removed in R06.
- Validation passed: independent core/adapters, all four production host feature profiles, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). No tests ran or changed. Changes remain uncommitted, and R06 remains in progress.

### R06 execution record — transactional status-rule evaluation

- Heartbeat and offline preparation now capture immutable definitions and evaluation input, not actions computed from local runtime hints. The ingress write carries an optional core `StatusRuleEvaluation`; unchanged statuses have no evaluation.
- Both adapters load active/acknowledged alerts, cooldowns, and current device targeting inside the existing ingress transaction after its device mutation/lock. Core builds an evaluation cache from shared definitions plus those device-scoped runtime rows; it never inherits the snapshot's local runtime maps.
- Core separates cooldown mutations from delivery intents. Adapters persist cooldown changes immediately; the host's remaining transaction participant enqueues returned deliveries before the same transaction commits. Failures roll back status/log writes, cooldown changes, and outbox inserts together. New status evaluations no longer queue `UpdateCooldown` actions.
- Adapter-owned SQL is exposed through narrow migration-feature transaction participants, called only through host `database` composition. They do not create connections or commit transactions. These temporary participants move inside adapter-owned ingress implementations during R08; they are not a new general transaction framework.
- PostgreSQL legacy cooldown writes now acquire the same sorted device locks as ingress and alert transitions. Both adapters preserve the maximum existing cooldown timestamp on upsert; the remaining local hint update also avoids timestamp regression. This does not fence an old queued update against a deliberately cleared/deleted row; legacy action retirement/runtime fencing is still open.
- Offline transitions sort by tenant/device before mutation on both engines, and PostgreSQL candidate reads use the same explicit order. Actual adapter device type/fleet/blueprint targeting is reloaded in the transaction instead of relying on the host's earlier catalog read.
- Removed the obsolete core snapshot status-change entry point that evaluated against local hints. Source inspection found no remaining callers or tests requiring it. No tests were added, repaired, or run.
- R06 is still incomplete: telemetry/contract-event/geofence decisions continue to use local runtime hints; zone-entry persistence and atomic decisions, retirement/fencing of legacy runtime-update actions, and final local-map removal remain required.
- Validation passed: independent core/adapters, all four host profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). No tests ran. No new migration was needed for this status-only slice; changes remain uncommitted.

### R06 execution record — telemetry, events, and durable zone entries

- Generalized the core transaction request to `DeviceRuleEvaluation` with status or telemetry input. Telemetry may include geofence evaluation; contract events keep their previous telemetry-only behavior. All current ingress paths carry definitions/input into the transaction instead of evaluating local runtime hints in the host.
- Both adapter participants read device-scoped alert, cooldown, and zone-entry rows and current targeting, then invoke core. Core separates cooldown/zone-entry mutations from delivery intents. Ingestion, state mutations, and queued deliveries commit together; new live paths no longer queue `UpdateCooldown` or `UpdateZoneEntry` actions.
- Added PostgreSQL `20260914020000_rule_zone_entries` and Turso `0011_rule_zone_entries.sql`, tenant-scoped rule/device foreign keys, a device lookup index, and Turso logical archive inclusion. PostgreSQL adds redundant tenant-qualified unique parent indexes to enforce the same foreign-key scope as Turso. No migrations were applied.
- Contract-event PostgreSQL ingestion now locks its device before writing the event, matching other rule-state writers. Duplicate event inserts still exit without evaluation/state mutation. Turso evaluates after obtaining its existing write transaction. Raw telemetry retains its existing optimistic device-type/fleet checks.
- Removed the remaining core snapshot telemetry/geofence methods that could read local runtime hints. `DeviceRuleEvaluation` uses shared definitions plus transaction-provided runtime state only. Host runtime-map updates remain temporarily for legacy actions, but no live evaluator consumes those maps.
- Zone-entry state starts empty on upgrade because old process-local entries cannot be reliably backfilled. The first valid location observation establishes persisted entry state. Subsequent restart behavior uses the database. Downgrading removes zone-entry history; use schema-compatible backup/restore tooling as described in ADR-014.
- Remaining R06 work: handle legacy queued cooldown/zone-entry actions without undoing newer resets/state, remove unused local runtime-map loading/writes, and reconcile final retention/ownership documentation. Tests remain deferred; no new invalid tests were found in this slice.
- Validation passed: independent core/adapters, all four host profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). No tests or migrations ran. Changes remain uncommitted; R06 is not complete.

### R06 execution record — definition-only snapshot loading

- Removed alert/cooldown fields from `RuleSnapshotRecords` and deleted both adapters' global runtime scans during snapshot loading. PostgreSQL's unused global cooldown loader was removed. Snapshot transactions now assemble definitions (rules, children, zones); device runtime rows are read only by ingress evaluation or their dedicated operations.
- Removed local alert/cooldown-map writes from HTTP alert transitions and action delivery. Snapshot reload no longer copies those maps. Database mutations already own this state, and live evaluators never consume local runtime maps.
- The only remaining `runtime_mut` caller is legacy `UpdateZoneEntry` delivery. Its temporary sink and the legacy cooldown delivery path still require reset-safe handling before final removal and R06 closure. No queued action was discarded by this cleanup.
- Validation passed: independent core/adapters, all four host feature profiles, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). Source search confirmed the sole remaining runtime-map caller. Tests were neither run nor changed; no additional invalid tests were found. Changes remain uncommitted.

### R06 execution record — cooldown reset markers

- Added `rule_cooldown_resets` migrations (PostgreSQL `20260914030000`, Turso 12), tenant-scoped parent references, a device index, and logical archive inclusion. Reset markers retain one maximum timestamp per tenant/rule/device until the parent is deleted; ordinary cooldown retention does not erase them.
- Reactivation records the reset marker and deletes the cooldown in the same transaction. Existing device/write locks serialize it with both live evaluation and legacy delivery. An old legacy cooldown at or before that reset becomes an acknowledged no-op, even when the cooldown row no longer exists.
- Live transaction decisions and resolve operations use a distinct current-decision helper; they do not get mistaken for legacy delivery when timestamps collide within a microsecond. Both paths still preserve the maximum existing cooldown timestamp. Turso's three callers share one storage helper.
- No historical reset times are guessed: only resets recorded by this implementation receive this protection. Old worker binaries must be stopped during rollout because they do not consult the marker. Downgrade removes marker history and therefore removes this replay protection. Migrations were not applied.
- Legacy queued zone-entry handling and removal of its final local sink remain open; R06 is not yet complete. No tests were added, repaired, or run, and no additional invalid tests were found.
- Validation passed: independent core/adapters, combined-adapter production host compilation, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). Source review confirmed the separate legacy/current helper call sites and all SQL parameter bindings. Changes remain uncommitted.

### R06 execution record — legacy zone handoff and final local-map removal

- Added the core legacy zone-entry operation and both adapter implementations. Legacy updates apply to the database in a device/write transaction, ordered by their immutable outbox creation time and binary event ID. Equal/replayed or older updates are acknowledged no-ops.
- Added `rule_zone_handoffs` migrations (PostgreSQL `20260914040000`, Turso 13), tenant-scoped parent references, a device index, and logical archive inclusion. Existing persisted zone entries are marked live during migration. No migrations were applied.
- A valid live location observation permanently takes ownership of each applicable geofence rule/device pair. Core records observations even when they produce no entry/exit mutation, so an old entry action cannot undo a newer outside observation. Invalid/missing coordinates do not take ownership. Handoff and state/output writes commit atomically.
- Removed `runtime_mut`, its mutation guard, all snapshot runtime-map copying, and snapshot wiring from the action worker. A full host source search also found and removed the obsolete rule-update branch that cleared local alert hints. Database alert state now consistently governs rule edits; no local trigger-type override remains.
- Legacy zone-entry delivery uses the core system façade and retained outbox metadata. Live observation wins over any later-delivered legacy update regardless of clock skew; old producers must be quiesced during rollout. Before handoff, the legacy ordering policy is deterministic on both databases.
- R06 still needs a final ownership audit (including worker transition/retention façades) and review of stale definition snapshots versus new runtime foreign keys before it is marked complete. Tests remain deferred; no new invalid tests were found in this slice.
- Validation passed: independent core/adapters, all four host profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 migration exceptions). Source search found no host runtime-map API or field access. No tests or migrations ran. Changes remain uncommitted.


### R06 final ownership audit

- Core worker operations now own alert update/resolve and legacy cooldown application. Core maintenance computes checked retention cutoffs and explicitly prunes resolved alerts across all tenants; the host owns scheduling and delivery clients.
- Transaction participants filter snapshot candidates against surviving enabled rules before evaluating. PostgreSQL holds key-share locks on surviving rule identities through commit; Turso uses its existing write transaction. Deleted snapshot rules cannot introduce runtime foreign-key violations. Definition edits still follow the documented snapshot freshness window.
- Outbox insertion SQL now lives in both adapter crates. Host ingress wrappers only delegate inside the existing outer transaction; their removal belongs to R08. No second connection or independent commit was introduced.
- The seven R06 implementation requirements are connected: core decisions, database-authoritative runtime state, adapter-owned outbox, claim-token conditional outcomes, versioned legacy-readable payloads, explicit retention/time/order semantics, and host transport/scheduling. External delivery remains at least once; migration and behavioral evidence remain deferred.
- Validation passed: independent core/adapters, all production host feature profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks. Architecture reports 41 direct accesses across 14 files and 9 tracked exceptions. No tests or migrations ran. R06 is implemented, not behaviorally verified.

### R07 initial shadow boundary

- Moved shadow records, the repository port, checked version increments, desired/reported merge and delta decisions, and reset policy into core. Existing host imports temporarily re-export these definitions while application and adapter ownership migrate.
- Preserved shallow patch semantics: null removes a key, nested values replace whole values, and delta contains only desired keys differing from reported. Core contains the pure JSON operations so it does not depend on the transport/common crate; common's public helpers remain available to existing consumers.
- Existing pure mutation tests moved unchanged with the implementation; they were not compiled or run. No tests became obsolete in this move. PostgreSQL row locking and Turso transaction behavior are unchanged.
- R07 remains in progress: move shadow application operations and SQL ownership next, then configuration and command transitions/dispatch under ADR-003.

- Shadow repository SQL moved into `backend-postgres::PostgresShadowRepository` and `backend-turso::TursoShadowRepository`; composition uses the existing pool/shared handles. Deleted the superseded host adapter modules. PostgreSQL still locks the shadow row through mutation/commit; Turso still serializes its writer transaction. Host service authorization/publication and legacy OTA transaction helpers remain to migrate in R07.
- Validation passed for this R07 slice: independent core/adapters, all four production host feature profiles, formatting, whitespace, and architecture checks (41 direct accesses across 14 files, 9 tracked exceptions). No tests were compiled or run. Changes remain uncommitted.


### R07 shadow applications and configuration extraction

- Core `ShadowApplication` owns read/manage authorization, reserved `ota` desired-key rejection, missing-record outcomes, and clock-based mutations. `DeviceShadowApplication` offers only tenant-scoped reads/reported updates for authenticated ingress. HTTP and device message handlers now invoke these operations. The host publishes desired deltas only after a committed result, preserving best-effort publication and metrics.
- Removed the four obsolete shadow service tests (`passes_tenant_identity_to_reads_reports_and_resets`, `reported_update_requires_manage_permission_before_persistence`, `every_user_shadow_mutation_requires_manage_permission`, `shadow_management_cannot_inject_or_remove_ota_commands`), which called deleted service/authorization functions. Pure shadow mutation tests remain unchanged and unexecuted. The remaining host shadow service contains delta transport and OTA report translation pending shared OTA migration.
- Core configuration now owns records, outcomes, shallow merge policy, read/manage authorization, missing-device errors, and the clock. Both adapters own configuration SQL and retain device-row/write-transaction serialization for atomic read/merge/upsert. HTTP uses the application façade; the old host service and adapter modules were removed.
- Removed the obsolete configuration service tests `passes_tenant_identity_to_get_and_atomic_merge` and `authorization_happens_before_persistence`, whose entry points were deleted. Pure configuration merge tests moved unchanged to core. No tests were run, compiled, added, or repaired.
- Source inspection found no configuration version or acknowledgement message in the current common protocol or configuration records/port. The user subsequently confirmed that configuration must preserve existing behavior; no versioning or acknowledgement work is required. At this stage, shadow OTA mutation unification and command validation/state/dispatch remained open; see the R07 completion entry below.
- Validation passed: independent core/adapters, all four production host profiles, combined-adapter CLI, formatting, whitespace, and architecture checks. Direct handler-to-repository access decreased from 41 across 14 files to 35 across 12 files; 9 tracked exceptions remain. No tests or migrations ran. Changes remain uncommitted.


### R07 command persistence and response transitions

- Moved command records/query/creation values and the business repository port into core; PostgreSQL and Turso command SQL now live in their adapter crates using existing shared handles. Removed superseded host adapter modules and the unused PostgreSQL `command_repo` helper module/exports.
- Core `CommandWorkerApplication` interprets device responses using the approved PostgreSQL behavior: `succeeded` and `failed` are terminal, while `ack` and all other strings mean `delivered`. It captures one clock time for each response or timeout pass and rejects unrepresentable timeout durations instead of wrapping an unsigned value.
- Both adapters apply tenant/device/ID-scoped responses through conditional SQL updates restricted to active states; terminal outcomes cannot be overwritten. PostgreSQL no longer performs a redundant pre-read. Timeout remains system-scoped and uses strict `created_at < cutoff`. PostgreSQL history gains an ID descending tie-breaker matching Turso.
- PI-01 correction: Turso writes `sent`, `delivered`, `succeeded`, `failed`, and `timed_out`. Its adapter maps historical `pending`/`completed`/`timeout` on reads and filters; pending remains eligible for responses/timeouts, historical terminal rows remain terminal. No data migration is needed. Previously unrecognized PostgreSQL stored statuses are now excluded from response transitions rather than treated as active; no supported status is affected.
- Response consumers and timeout scheduling now invoke core directly. User dispatch/list authorization, validation, `DeviceBus`, shared rule-action dispatch, and OTA integration remain pending. In particular, rule-action command retries currently call non-idempotent creation with a stable delivery ID; resolve that in shared dispatch without adding automatic retries to ordinary user sends.
- Source inspection found no tests directly using the changed command repository/worker interface. No tests were removed, added, repaired, compiled, or run for this slice. R07 remains in progress.
- Validation passed: independent core/adapters, all four production host profiles, formatting, whitespace, and architecture checks (35 direct accesses across 12 files, 9 tracked exceptions). No tests or migrations ran. Changes remain uncommitted; continue with core command dispatch and the host DeviceBus.


### R07 core dispatch and host DeviceBus

- `CommandApplication` now owns user authorization, nonempty command/object input checks, assigned-contract decoding, command/schema/route validation, dispatch creation, publication sequencing, and command history access. Single command, restart, and bulk restart routes call the application; the old command service was removed. Empty-command HTTP error precedence remains before authorization; contract failures retain 422 semantics.
- Core's `DeviceBus` receives domain command values plus an optional contract address. The host `ZenohDeviceBus` owns default topic construction, string-valued protobuf parameter conversion, encoding, publication, and transport metrics/logging. Core asks the bus which contract protocols it supports; it does not import Zenoh or generated wire types.
- ADR-003 is connected end to end: persistence returns a recorded `sent` attempt before publication, storage failure prevents publication, and synchronous bus failure maps through the existing HTTP 502 `device_communication_error` response without deleting/updating the row or exposing raw transport errors. Ordinary user sends have no automatic republish path.
- Rule actions invoke the same validation and bus through a separate durable-ID operation. Exact matching existing rows can be reused, including after a competing insertion; tenant/device/command/parameter mismatch cannot publish. Delivered/terminal records skip publication, while `sent` records may republish under the outbox's existing retry policy. Historical rule-action string-valued parameter storage is preserved. Concurrent deliveries may still publish twice: external delivery remains at least once. A retry of a sent action validates against the currently assigned contract, so changed/incompatible contracts can prevent retry publication.
- Added tenant-scoped adapter command lookup for durable retry identity checks. No database migration or HTTP response shape changed. The removed service had no inline tests; source searches found no additional invalid command fixtures. No tests were added, repaired, compiled, or run.
- At this stage, R07 remained open for shared OTA/shadow mutation. Configuration versions/acknowledgements are excluded by the subsequent user scope correction. The full R01–R15 objective is not complete.
- Validation passed: independent core/adapters, all four production host profiles, combined-adapter CLI compilation, formatting, whitespace, and architecture checks. Direct handler-to-repository access fell from 35 across 12 files to 28 across 11 files; 9 tracked exceptions remain. No tests or migrations ran. All changes remain uncommitted.


### R07 shared OTA/shadow mutation

- OTA trigger and terminal cleanup now use the same core desired-patch/version/delta policy as ordinary shadow changes. Core `clear_ota_for_deployment` removes the reserved command only when its deployment ID matches, so late terminal reports cannot clear a newer command.
- PostgreSQL shadow row locking/storage and Turso shadow decoding/storage are shared adapter transaction participants. Both the ordinary shadow repositories and legacy OTA transactions call those helpers using their existing connections; no new pool, independent commit, or separately committed OTA shadow mutation was introduced. PostgreSQL retains shadow-before-deployment locking, and Turso retains the enclosing writer transaction.
- Removed duplicate host shadow SQL and the unused PostgreSQL `shadow_repo` module. The firmware persistence modules now contain no direct shadow queries or separate merge/delta implementation. Their wider OTA orchestration/firmware SQL still moves in R09.
- Turso terminal cleanup now checks the supported i32 shadow version before incrementing, matching core and PostgreSQL behavior; overflow or out-of-range persisted versions fail and roll back the whole OTA status transaction instead of writing an unusable version. Core object normalization now also persists normalized reported state through the shared store when existing state is non-object. No schema/wire change was introduced.
- No tests became invalid by source inspection, and none were run, compiled, added, or repaired. The subsequent user scope correction excludes configuration versions/acknowledgements and closes R07; the remaining packages remain active.
- PostgreSQL shared storage returns the persisted row, preserving database timestamp precision for ordinary shadow mutation responses. The full production feature matrix and architecture checks passed before this precision review; the affected adapter/combined-host builds also passed after the precision adjustment. Architecture remains 28 direct accesses across 11 files and 9 tracked exceptions.


### R08 device logs (independent work while R07 scope is clarified)

- At this stage, configuration scope was awaiting clarification because no versioning/acknowledgement flow exists in the current API/protocol. The user subsequently chose to preserve existing behavior, as recorded below. No new configuration feature or protocol change is required.
- Moved log records, query/ingress/retention ports, user read authorization, query limit/level normalization, severity fallback, and checked retention arithmetic into core. The host handles timestamp parsing, protobuf/topic/identity checks, scheduling/backoff, and logs outcomes.
- Both adapter crates now own direct device-log insertion, listing, and global retention SQL. PostgreSQL inserts from a tenant/device-qualified, key-share-locked device selection in one statement, eliminating the prior separate existence-check/insert window. Turso keeps its serialized writer insert-if-device-exists statement. Core supplies ingress observation time rather than selecting a separate database/adapter clock.
- Resolved log PI-03/PI-04: both engines use inclusive `since` and descending creation time/ID ordering. Retention remains global with strict `created_at < cutoff`. Duplicate delivery still creates duplicate logs because the wire protocol has no event ID; this is not an exactly-once log feature.
- HTTP, device-message handling, and retention scheduling invoke the core operations. Removed host log service/adapter modules and unused PostgreSQL `log_repo` helpers. Presence/heartbeat log inserts remain part of their atomic transactions and will move with those operations; this slice does not split their write sets.
- Source inspection found no affected repository mocks or direct service tests to remove. Tests were not added, repaired, compiled, or run. R08 remains in progress; the R07 configuration decision was subsequently resolved below.
- Validation passed: independent core/adapters, all four production host profiles, formatting, whitespace, and architecture checks. Direct handler-to-repository access fell from 28 across 11 files to 27 across 10 files; 9 tracked exceptions remain. No tests or migrations ran. Changes remain uncommitted.


### R07 scope correction approved by the user

- The user chose: “Keep existing behavior; correct the plan.” Configuration scope is existing reads and atomic JSON merges. Do not add configuration versions, device acknowledgements, transport messages, or endpoints. Earlier open-item notes about those features are superseded by this decision.
- Updated R07 implementation/deferred acceptance and the architecture plan accordingly. Existing device-contract assignment convergence on valid events remains existing behavior and belongs to R08 event ingestion; it is distinct from a new configuration acknowledgement protocol.
- R07 implementation audit: core owns shadow merge/reset/conditional OTA cleanup, existing configuration policy, command validation/status/dispatch; adapters own their persistence; HTTP/consumers/workers invoke core; DeviceBus owns wire/Zenoh publication; durable rule actions reuse matching command identities; terminal/active outcomes use conditional writes. Shared OTA shadow participants retain their enclosing transaction and PostgreSQL row locking. Old paths are removed, with documented transaction bridges awaiting full firmware migration in R09. Production builds/static checks passed in the recorded slices; behavioral verification remains deferred. R07 is implemented under the clarified scope.


### R08 typed-event persistence and metric reads

- Moved typed event/metric records, the event repository port, and both persistence implementations into core/adapter ownership. Event insert, existing contract-assignment convergence, typed metric inserts, rule runtime decisions, and outbox insertion remain in the same enclosing transaction. Duplicate event IDs still exit without repeating those writes.
- Adapter rule-runtime/outbox transaction participants now compile internally for standalone adapter builds as well as the host migration bridge. Their public bridge exports remain feature-gated; event persistence directly calls its own adapter modules rather than host aliases or helpers.
- Core `EventApplication` owns typed-metric read authorization, limit normalization, and missing-device outcomes. The HTTP handler retains timestamp parsing and response conversion. Both adapters preserve `[since, before)` timestamps and deterministic occurred-time/event/stream/field ordering.
- Removed the obsolete host event service and event adapter modules. Event envelope/contract validation and typed extraction remain in the host ingress handler and must move next; R08 is not complete. Raw telemetry and presence transactions are also still pending.
- No tests became invalid by source inspection. No tests were added, repaired, compiled, or run. No migrations were introduced or applied. The prior R07 configuration scope question is resolved by the user's instruction to preserve existing behavior.
- Validation passed: independent core/adapters, all four production host profiles, formatting, whitespace, and architecture checks. Direct handler-to-repository access fell from 27 to 26 across 10 files; 9 tracked exceptions remain. No tests or migrations ran. Changes remain uncommitted.


### R08 core event validation and extraction

- `EventIngressApplication` now owns envelope version/UUID validation, assigned-contract/hash checks, contract identity and size checks, route/schema checks, payload validation, typed metric extraction, numeric rule metric keys/semantic aliases, observation time, and construction of the atomic event write. Core receives parsed domain values rather than transport bytes or generated protocol types.
- The host retains JSON envelope decoding, device identity/context lookup, snapshot acquisition, and outcome logging. All contract/schema/metric decisions moved out of the event handler. The remaining context lookup is a narrow legacy ingress dependency to migrate with presence; adapters still reload authoritative device targets inside the transaction before evaluating rules.
- Existing event semantics are preserved: absent metric paths are skipped, type mismatch rejects the event, only numeric values enter rule metrics, contract events do not run geofence evaluation, duplicate IDs do not repeat writes, and valid events participate in existing contract-assignment convergence. No configuration acknowledgement feature was added.
- The host now acquires context/snapshot before core validation, so the diagnostic emitted when both snapshot acquisition and input validation would fail may change; both cases still drop the event without writes. Transaction boundaries and wire format are unchanged.
- No affected inline event tests or mocks were found by source inspection. Tests were not added, repaired, compiled, or run. R08 remains in progress for presence, raw telemetry, and remaining ingress composition.
- Validation passed: independent core/adapters, all four production host profiles, formatting, whitespace, and architecture checks (26 direct accesses across 10 files, 9 tracked exceptions). No tests or migrations ran. Changes remain uncommitted.


### R08 presence and identity ownership

- Moved the tenant-qualified device identity, ingress records, and ingress repository port into core. The identity constructor preserves the established 1–128-byte ASCII alphabet and tenant validation. Its existing two unit tests moved unchanged with the type; tests were not compiled or run. Host imports remain compatibility re-exports of the same concrete type.
- Both adapters now own identity lookup, heartbeat and offline persistence, including the existing atomic status/log/rule-runtime/outbox write sets. PostgreSQL device row locks, sorted offline lock order, and conditional status checks are preserved; Turso retains its writer transaction and conditional updates. Raw telemetry's remaining outbox helper now delegates directly to the adapter instead of the deleted host device module.
- Core `DeviceIngressApplication` owns severity-independent presence policy: accepted status/fallback, the three-attempt optimistic heartbeat loop, status-rule evaluation preparation, global offline candidate processing, and checked timeout arithmetic. Existing accepted-status casing and uptime narrowing remain unchanged. Host workers retain cadence/backoff and cancellation.
- Added the core `RuleSnapshotProvider` port, implemented by the host snapshot store. Core requests snapshots when a transition needs evaluation; it does not own polling or in-memory store lifecycle. Existing per-transition snapshot acquisition is preserved.
- Event ingestion now acquires device context inside core using the migrated ingress port. Host event handling no longer coordinates device-context and event repositories; the interim `EventRuleContext` transport parameter was removed. The core event clock sample again precedes the context lookup, matching the earlier event sequence.
- Removed the legacy host ingress service and PostgreSQL/Turso device-ingress modules. No tests became obsolete other than moving existing identity tests with their implementation; none were added, repaired, compiled, or run. No migrations or wire changes were introduced. R08 remains open for raw telemetry and final ownership/contract audit.
- Validation passed: independent core/adapters, all four production host profiles, combined-adapter CLI, formatting, whitespace, and architecture checks (26 direct accesses across 10 files, 9 tracked exceptions). No tests or migrations ran. All changes remain uncommitted; continue with raw telemetry.


### R08 raw telemetry persistence, reads, and maintenance ownership

- Core owns telemetry records/ports and read applications, including authorization, limits, and missing-device outcomes. HTTP timestamp parsing/DTO conversion remains in the host. Latest-location keeps its existing authenticated tenant-scoped behavior, which lacked a separate telemetry permission gate; this extraction does not silently add a new authorization requirement.
- Both adapter crates now own raw telemetry writes, reads, latest-state/location persistence, hourly rollups, and retention. PostgreSQL partition functions/SQL helpers moved into its adapter. Atomic record/device/rule-runtime/outbox transactions remain intact. Removed the old host telemetry service/adapters/helpers and final ingress outbox wrapper; no host caller remains for the rule-runtime/outbox transaction bridge aliases.
- Raw/history PI-03/PI-04: PostgreSQL now uses inclusive `since` and exclusive `before`, matching Turso. Raw lists and latest location use descending receive-time/ID tie order. Hourly lists retain `[since, before)` and descending unique bucket order.
- Core `TelemetryMaintenanceApplication` owns one clock sample, UTC closed-hour calculation, the existing 25-hour lookback, and checked retention arithmetic. Host maintenance retains cadence/backoff and logging. PostgreSQL still commits rollup/partition/row-pruning stages separately, while Turso uses one write transaction; PI-09's final retry/partial-failure audit remains open, especially when retention overlaps the recomputation window.
- PI-07 remains open: PostgreSQL raw insertion still obtains receive time from the database, while Turso stores the supplied observation time. Raw ingress normalization/evaluation construction is still in the host and must move before R08 closes. No migrations or wire changes were introduced.
- Source inspection found no tests requiring removal for this slice. Tests were not added, repaired, compiled, or run. Changes remain uncommitted.
- Validation passed: independent core/adapters, all four production host profiles, combined-adapter CLI, formatting, whitespace, and architecture checks. Direct handler-to-repository access fell from 26 across 10 files to 22 across 9 files; 9 tracked exceptions remain. No tests or migrations ran. Changes remain uncommitted.

### R08 raw telemetry ingress application

- Added core `TelemetryInput` and `TelemetryIngressApplication`. Core now owns metadata normalization, compatibility for location presence (including flagged `(0, 0)`), rule evaluation intent, and the existing three attempts to commit against device targeting changes. The injected clock and snapshot provider replace host policy dependencies.
- The host telemetry consumer now decodes protobuf, checks topic/device agreement, maps transport values, and reports the core outcome. Persistence still commits raw telemetry, latest state, device projections, rule runtime, and durable actions together in the adapters.
- Preserved missing-device drops, per-attempt observation times, geofence evaluation, raw payload storage, and existing optional location/motion behavior. Exhausted targeting retries return the existing core busy error and are logged/dropped by the consumer. No transport acknowledgement or new protocol was introduced.
- Production checks passed for the host with no default features and with combined PostgreSQL/Turso features, plus formatting, whitespace, and architecture checks (22 direct accesses across 9 files; 9 tracked exceptions). Source inspection found no tests referencing the changed handler internals; no tests were run, compiled, added, repaired, or removed.
- R08 remains in progress for the timestamp contract (PI-07), maintenance partial-failure policy (PI-09), and final ingress ownership audit. R09–R15 remain outstanding.

### R08 timestamp and telemetry maintenance contracts

- PI-07: raw telemetry now receives an explicit core-clock `received_at` at application entry, before device-context lookup, fixed across targeting retries and truncated once to UTC microseconds. Both adapters persist that value. This intentionally replaces PostgreSQL's database-default timestamp and Turso's per-attempt observation timestamp; no historical records are rewritten. Latest/history ordering and hourly buckets use receipt time.
- `observed_at` remains the server evaluation instant for each attempt and drives device presence and rules. Device occurrence time is distinct: typed events retain their validated envelope `occurred_at`; raw protobuf telemetry retains its existing device `timestamp` in the stored payload, without adding a database column, interpreting its units, rejecting older clients, or changing rules to device-clock ordering. Server receipt time is not a claim about when the device measured a sample.
- PI-09 telemetry: PostgreSQL now wraps rollup upserts, partition maintenance, and raw deletion in one transaction. Inspected partition functions use transactional table creation/drop and row movement, with no internal commit. Failure rolls back the entire pass, matching Turso's logical all-or-nothing maintenance behavior; physical partition work remains PostgreSQL-only. Retrying after a failed pass repeats the whole operation. A successful retry can report different counts as new samples arrive. Longer PostgreSQL lock duration is the tradeoff for atomicity.
- Remaining maintenance audit: the fixed 25-hour recomputation window can overlap already-pruned raw data for short retention settings. That aggregate preservation issue still needs resolution before R08 closes. PI-09 also tracks metrics retention and firmware behavior owned by later packages; this change does not claim to resolve those.
- Validation for this slice: PostgreSQL adapter production compilation passed after the transaction change; combined PostgreSQL/Turso host production compilation passed after the timestamp change. Formatting, whitespace, and architecture checks passed. No tests were run, compiled, added, repaired, or removed; no migrations were added or applied. Behavioral rollback and timestamp fixtures remain deferred.

### R08 contract routing and event identity audit

- Moved assigned-contract address lookup and decoder classification into core `ContractIngressApplication`. It preserves device-to-cloud matching, command response precedence over JSON event handling, unknown-address fallthrough, and recognized-but-unsupported route outcomes. The host retains decoding and logging. The remaining firmware call in shadow ingress belongs to R09.
- Event IDs remain globally unique stored strings: PostgreSQL conflicts on `device_events.id`; Turso retains its existing insert-ignore behavior. UUID validation does not canonicalize string spellings. A repeated stored ID, including a cross-tenant collision or different content under that ID, produces no additional metrics, assignment convergence, runtime changes, or outbox actions. This preserves the existing first-insert behavior and does not promise content-equivalence validation or telemetry deduplication.
- Typed metric reads retain `[since, before)` on occurrence time, descending occurrence/event ID and ascending stream/field tie keys. Added explicit PostgreSQL C and Turso BINARY collations for the text tie keys so database locale does not change pagination ordering.
- R08 remains open for retention/rollup overlap resolution and final package validation. No test work was performed.
- Retention audit finding for the next slice: clamping recomputation only to today's cutoff is insufficient when retention is later increased, because older raw buckets may already be partially deleted. The solution must account for prior committed pruning (for example, a durable pruning boundary) and preserve strict raw deletion cutoff semantics. Comparing sample counts alone cannot establish aggregate completeness when late samples arrive.
- Validation: combined PostgreSQL/Turso host production compilation passed for routing extraction; independent PostgreSQL/Turso production compilation passed after explicit ordering changes. Formatting and whitespace checks passed. Tests were neither compiled nor run, added, repaired, or removed.

### R08 durable pruning boundary

- Added additive PostgreSQL migration `20260914050000_telemetry_maintenance_boundary` and Turso migration 14. A global singleton records the greatest committed raw deletion cutoff. Maintenance loads it under the PostgreSQL row lock or Turso writer transaction, applies core `rollup_recompute_start`, then advances it in the same transaction as rollups, partition work, and strict `< cutoff` raw deletion. Failed passes cannot advance the boundary.
- Core rounds a non-hour-aligned previous pruning boundary up to the next UTC hour and intersects it with the existing 25-hour recomputation window. Hours already exposed to pruning remain frozen even if retention increases, the server clock moves backward, or a later pass uses an earlier cutoff. Retried passes cannot replace a complete historical rollup with a partial raw-data aggregate. Late receipts falling in frozen hours remain raw data until retention removes them; they do not revise frozen rollups.
- Legacy databases have no reliable pruning history. When telemetry or rollups already exist, migrations initialize a conservative boundary at migration time; old aggregates remain unchanged and no automatic historical backfill is attempted. The hour containing that boundary is also frozen if it is partial. Fresh databases initialize without a boundary. This cannot repair aggregates damaged before migration. Deploy the migrations with old maintenance workers quiesced; rolling back the migration loses the recorded boundary and restores the old risk.
- Raw retention keeps its exact cutoff, including zero-day retention; the implementation does not promise complete hourly aggregates after raw samples have been removed before their hour closes. Hour buckets are explicitly UTC in PostgreSQL regardless of connection timezone; Turso uses mathematical floor for negative epoch timestamps as well as positive timestamps.
- Removed the invalid `previous_schema_snapshot_upgrades_without_losing_device_data` test and its private fixture helper/imports: it asserted schema version 9 after applying all migrations. No test was repaired, added, compiled, or run. No migrations were executed.

### R08 implementation audit

- Identity, contract classification/validation, typed field extraction, raw telemetry normalization, log normalization, heartbeat/offline policy, and retry intent are core-owned (`application/{device_ingress,events,telemetry,logs}.rs`). Host consumers decode/authenticate/map/log and use the applications. OTA report translation remains explicitly assigned to R09.
- PostgreSQL/Turso own events, telemetry, logs, presence, atomic rule/outbox participants, read SQL, and maintenance storage. Raw ingestion continues using device/write transactions; typed events preserve their existing deduplication behavior. No deduplication or configuration acknowledgement protocol was added.
- Read windows are raw receipt-time `[since, before)`, hourly bucket `[since, before)`, typed occurrence-time `[since, before)`, and log creation-time inclusive `since`. Raw/latest/location use descending receipt time and numeric ID; metrics use explicit binary text tie ordering; hourly bucket keys are unique per device; logs use descending creation time and ID.
- PI-07 has explicit raw receipt versus evaluation times and device occurrence semantics; typed occurrence and receipt times now normalize to microseconds in core. PI-09 telemetry has atomic maintenance and a durable pruning boundary, with migration and historical completeness limitations documented above. Other PI-09 domains remain assigned to later packages.
- Deleted superseded host services/SQL/adapter modules in the recorded R08 slices. Compatibility type re-exports and composition aliases remain for final R12 cleanup. R08 behavioral acceptance remains deferred in full under the user's test policy; compilation does not prove database rollback, migration execution, or concurrency behavior.

- Final R08 production checks passed: host without default features, PostgreSQL-only, Turso-only, and combined PostgreSQL/Turso; independent adapters; formatting and whitespace. Architecture passed with 22 direct accesses across 9 files and 9 tracked exceptions. No tests or migrations were run. R08 implementation is complete under the deferred-verification policy; R09 is next and the full objective remains active.


### R09 firmware boundary and Turso persistence

- Moved firmware/blob/deployment values and the existing `FirmwareRepository` port into core. Core owns artifact validation and forward-only OTA transition/terminal-state policy with the same accepted values and case handling. Host types/port re-export the core definitions during migration.
- Moved Turso firmware metadata, blob, version, deployment, and legacy blob migration SQL into `backend-turso::TursoFirmwareRepository`, composed from the existing shared connection handles. Existing writer transactions, compatibility checks, stale-report rejection, supersession, and conditional shadow cleanup are preserved. No CI-ingest transaction or object key changed.
- OTA shadow participants now call adapter-internal shadow helpers directly in the enclosing Turso transaction. Removed the unused Turso shadow transaction exports and host composition aliases; PostgreSQL equivalents remain until its firmware extraction.
- R09 remains in progress for PostgreSQL SQL extraction and core application ownership of metadata, object orchestration, download grants, OTA planning/status, and legacy migration operations. No tests were added, repaired, compiled, run, or removed for this slice; no migrations were added or executed.
- Validation passed: independent Turso adapter production compilation and combined PostgreSQL/Turso host production compilation, including removal of the transaction exports. Formatting, whitespace, and architecture passed (22 direct accesses across 9 files; 9 tracked exceptions). These checks do not establish runtime OTA or object-storage behavior; that acceptance remains deferred.


### R09 PostgreSQL firmware persistence

- Moved remaining PostgreSQL firmware metadata/blob/deployment/legacy-migration persistence to `PostgresFirmwareRepository` and adapter-private `firmware_sql`. Composition reuses the existing pool. Replaced host error dependencies with a local transaction error preserving database/persistence mapping.
- Preserved the enclosing create/delete/OTA transactions, tenant-scoped lookups, blueprint/device-type compatibility, artifact checks, supersession, and stale/terminal report rejection. Shadow locking still precedes deployment mutation; shared shadow helpers now run directly inside the adapter transaction.
- Deleted the superseded host firmware adapter and SQL module, domain/repository aliases, and PostgreSQL shadow transaction bridge exports. Removed the unused `find_firmware_blob` SQL helper; active reads retain the optional blob lookup and original not-found mapping at application level. No CI-ingest behavior changed.
- Both adapters now own firmware persistence. R09 remains open for core application policy and object/download/OTA orchestration; these host services have not yet been claimed complete. Existing port signatures and host type re-exports remain compatible, and source inspection found no test directly calling the removed SQL helper. No tests were added, repaired, compiled, run, or removed; no migrations were introduced or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation, independent PostgreSQL adapter production compilation, formatting, whitespace, and architecture (22 direct accesses across 9 files; 9 tracked exceptions). Runtime transaction/OTA acceptance remains deferred.

### R09 firmware metadata application

- Added core `FirmwareApplication` and wired its narrow repository through core composition. Moved firmware/deployment listing, metadata creation/deletion, blob lookup, and version suggestion authorization/error mapping from the host service into core. HTTP handlers invoke the application directly; upload/delete object wrappers now call it for metadata operations.
- Preserved permission choices (`ReadFirmware`, `ManageFirmware`, and `ReadDevices` for device deployment history), duplicate-version conflict text, not-found outcomes, pagination inputs, and object-before-metadata / metadata-before-cleanup ordering. Object storage, blueprint preparation, grants, and OTA dispatch remain host workflows pending the next R09 slices.
- Removed obsolete wrapper functions and the three invalid firmware creation tests: `upload_failure_removes_the_object_and_preserves_the_database_error`, `successful_upload_retains_object_until_metadata_deletion`, and `backend_mismatch_keeps_the_object_but_does_not_fail_metadata_deletion`, together with their private repository mock/support. Their old repository-argument entry points no longer exist. No tests were repaired, added, compiled, or run.
- Tightened architecture allowances: removed firmware HTTP repository access entirely and reduced device handlers to the two remaining OTA dispatch accesses.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture. Direct handler-to-repository accesses fell from 22 across 9 files to 11 across 8 files; 9 tracked migration exceptions remain. Behavioral verification remains deferred. R09 and the full objective remain in progress.

### R09 object upload/delete orchestration

- Added the core `FirmwareObjectStorage` capability with backend identity, key allocation, object write, and delete operations. The existing host `FirmwareObjectStore` implements it using unchanged filesystem/S3 storage and key construction; storage errors are logged there.
- `FirmwareApplication::upload` now owns object-first write, metadata creation, and best-effort compensation. Metadata/authorization failure remains the returned error even if cleanup fails. The host wrapper retains SHA-256 calculation, as allowed by the plan. Existing upload authorization ordering and file-size conversion are preserved in this extraction.
- `FirmwareApplication::delete_stored` commits metadata deletion before best-effort cleanup. A configured-backend mismatch returns only a diagnostic for host logging; cleanup failure does not turn successful metadata deletion into an HTTP error. The host wrappers now translate/log rather than orchestrating storage and persistence.
- Blueprint preparation, download grants/reads, OTA planning/status, and legacy blob migration orchestration remain for subsequent R09 slices. No tests were added, repaired, compiled, run, or removed; no migrations were added or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture (11 direct accesses across 8 files; 9 tracked exceptions). Runtime storage-failure/compensation acceptance remains deferred. R09 remains in progress.

### R09 blueprint preparation and legacy migration

- Moved `PreparedBlueprintFirmware` and blueprint preparation into core `FirmwareApplication`. Both firmware HTTP creation paths call it directly. Existing blueprint/type application permissions and call order remain; missing firmware behavior maps through core `InvalidOperation` to the existing HTTP 422 response. Compatibility, update strategy, and record defaults are unchanged.
- Added narrow global `FirmwareMigrationApplication::migrate_next`. It reads one legacy blob, writes the deterministic host-generated object key, and conditionally marks metadata migrated. On metadata failure, the object remains for retry; concurrent workers retain the existing conditional-update behavior. No object deletion or new migration transaction was introduced.
- The host worker now schedules that operation: 300 seconds when idle, 30 seconds after failure, immediate continuation after a migrated/already-migrated result. Storage SDK/key construction and logging remain host-owned. No direct legacy firmware repository operations remain in the host worker.
- Downloads/grants and OTA orchestration remain outstanding in R09. No tests were added, repaired, compiled, run, or removed; no database migrations were added or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation after both changes, formatting, whitespace, and architecture (11 direct accesses across 8 files; 9 tracked exceptions). Runtime migration/retry and firmware compatibility checks remain deferred. The next download slice must also remove the special `firmware_download_repository()` host accessor, which bypasses the ordinary handler access counter.

### R09 firmware download application

- Added core download operations for authenticated users and verified device grants. Core now selects legacy inline bytes before object storage, rejects backend mismatch/missing locations, checks the stored byte count, and preserves the existing user/device not-found messages and safe storage errors.
- Added a host object-read capability and a core `VerifiedFirmwareDownload` domain scope. The host must verify the token signature, audience, and expiry before constructing that scope; construction validates tenant and positive firmware ID but is explicitly not raw-token verification. The existing JWT format, audience, expiry checks, route, and keys remain unchanged.
- Both HTTP download handlers now call core. Removed `AppState::firmware_download_repository()` and its direct repository path. Filename/header sanitization, private/no-store headers, content length, and HTTP body construction remain host-owned and unchanged.
- Grant issuance policy and OTA orchestration remain outstanding in R09. Existing JWT tests still target unchanged signing/verification functions and are retained; none were run, compiled, added, repaired, or removed. No migrations were added or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture (11 counted direct accesses across 8 files; 9 tracked exceptions). The removed special download accessor was outside that count. Runtime download/grant behavior remains deferred; R09 is still in progress.

### R09 OTA initiation and download-grant policy

- Core `FirmwareApplication::trigger_ota` now authorizes deployment, creates the scoped signing request, builds the existing download URL, invokes atomic persistence, and maps all deployment outcomes to the existing application/HTTP errors. Single and bulk device handlers use it through the host publication wrapper.
- Core owns the unchanged download audience and seven-day lifetime through an injected clock. The host `JwtFirmwareDownloadSigner` keeps JWT encoding/key handling. Expiry arithmetic is checked for unsupported clock values. Existing grant helper/signature remains available and delegates to the same policy; verification and its existing tests remain intact and unexecuted.
- The host publishes the committed delta using the existing best-effort Zenoh helper. Persistence failure still prevents publication; no republish loop was added. Removed the last two direct device-handler repository accesses and their architecture allowance.
- R09 remains open for OTA report policy extraction, final OTA persistence-policy audit, and package validation. No tests were added, repaired, compiled, run, or removed; no migrations were added or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture. Counted handler accesses fell from 11 across 8 files to 9 across 7 files; 9 tracked exceptions remain. Behavioral OTA/grant verification stays deferred.

### R09 OTA report application and shared version policy

- Added core `FirmwareReportApplication`, injected with the firmware port and clock. It interprets reported OTA JSON, normalizes status, ignores missing/invalid positive deployment IDs as before, carries the reported firmware ID/error, and supplies a completion time only for terminal status. The host shadow consumer invokes it with authenticated identity after the existing shadow report operation.
- Deleted host OTA report interpretation; the shadow service now only publishes transport deltas. Adapter stale-report/terminal checks and atomic conditional desired-command cleanup remain unchanged.
- Consolidated identical PostgreSQL/Turso version increment logic in core. This preserves the existing three-component patch increment/fallback behavior, including its existing unsigned overflow limitation; this extraction does not claim to repair unsupported maximum patch values.
- R09 still needs its final persistence-policy/OTA invariant audit and package feature-matrix validation before it can close. No tests were added, repaired, compiled, run, or removed; no migrations were added or executed.
- Validation passed after both extractions: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture (9 direct accesses across 7 files; 9 tracked exceptions). Behavioral verification remains deferred.

### R09 shared OTA artifact preparation

- Moved URL selection and desired-shadow OTA payload construction from both adapters into core `PreparedOtaArtifact`. It preserves external HTTPS URL precedence, signed-download fallback, artifact limits, mandatory SHA-256, exact shadow keys, and deployment/firmware identity values.
- Adapters validate the prepared artifact before deployment mutation and supply the generated attempt ID when constructing the patch. Existing PostgreSQL shadow locking and Turso writer transactions still enclose supersession, deployment insertion, and shadow storage; no publication occurs inside persistence.
- R09 remains open for full native/client invariant evidence and production feature-matrix checks, plus any remaining policy found by that audit. Initial source inspection located native rollback/watchdog/boot restoration in `clients/rust/sdk/src/native_ota.rs`; no client code was changed or runtime behavior claimed verified.
- No tests were added, repaired, compiled, run, or removed. No migrations were added or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture (9 direct accesses across 7 files; 9 tracked exceptions). Follow-up client audit anchors are `clients/rust/runtime/src/lib.rs` and `clients/rust/macos/src/main.rs` for native boot initialization and shadow subscriptions. Behavioral verification remains deferred.

### R09 completion source audit

- Core `application/firmware.rs` owns metadata permissions/error mapping, blueprint preparation, upload/delete ordering, download selection/integrity, grant issuance policy, OTA initiation/report interpretation, and one-step legacy migration. `firmware.rs` owns values/ports, transition/artifact/version policy, and OTA patch construction. Both adapter firmware modules own persistence and preserve transaction-sized deployment mutations. The old host SQL/aggregate implementations and shadow transaction bridge exports are removed; host wrappers perform hashing, signing, publication, diagnostics, and scheduling.
- Reservation and identity: `application/shadows.rs::update_desired` rejects any user patch containing `ota`, including null. `PreparedOtaArtifact` includes firmware/deployment IDs and SHA-256. `FirmwareReportApplication` consumes the fresh report passed by the shadow handler, not merged historical state. Both adapters reject firmware-ID mismatch and disallowed transitions; `clear_ota_for_deployment` clears only the matching desired attempt inside the transaction. PostgreSQL locks the shadow before deployment changes; Turso retains its writer transaction.
- Grant/download behavior: seven-day purpose-specific audience, tenant/object scope, existing JWT signing/verification, external HTTPS precedence, user permission checks, response headers, and object keys are preserved. Object failures keep original error precedence. Legacy migration retains its deterministic object for retry after metadata failure.
- Native preservation evidence: `clients/rust/runtime/src/lib.rs` and `clients/rust/macos/src/main.rs` call `native_ota::boot` and use its returned version, declare shadow subscribers before sending `ShadowGet`, and reserve OTA with `compare_exchange(false, true, ...)` before spawning work. `clients/rust/sdk/src/native_ota.rs` persists journal/image state, fences rollback by attempt identity, verifies the previous image digest, restores previous version on rolled-back boot, and emits terminal reports only from confirmed/rolled-back journal states. These client files and the CI-ingest implementations have no worktree changes from this refactor. Source evidence establishes preservation, not successful runtime boot/rollback execution.
- Behavioral acceptance remains deferred by user instruction. No tests or migrations were executed. Remaining host compatibility aliases belong to R12; broad final entry-point/documentation review belongs to R11/R15.

- R09 production matrix passed: no-default host, PostgreSQL-only host, Turso-only host, combined host (preceding unchanged-code slice), independent core/adapters, and combined PostgreSQL/Turso CLI. Formatting, whitespace, and architecture passed (9 direct accesses across 7 files; 9 tracked exceptions). No tests/migrations ran. R09 implementation is complete under the deferred-verification policy; R10 is next. The full objective remains active.


### R10 activity projection extraction

- Moved activity records/read port and authorization/filter normalization into core `ActivityApplication`; HTTP calls the application directly. Preserved `ReadLogs`, accepted filters, trimming/lowercasing, 256-byte search limit, and reversed-time-window rejection. The legacy lowercasing of device-ID filters is preserved here and requires explicit review with the remaining projection semantics.
- Both adapter crates now own the activity union queries, row hydration, and count/page reads using existing pool/shared handles. Deleted the host activity service and adapter modules. Kept the inclusive `[since, until]` activity interval; added explicit PostgreSQL C/Turso BINARY text-ID tie ordering for equal timestamps.
- Moved the two existing pure filter tests unchanged with their functions. No tests were added, repaired, compiled, run, or removed. Source-level preservation is not behavioral parity evidence: search collation, audit-message formatting, count/page consistency, and remaining projection differences still need the R10 audit.
- Removed the activity handler's repository allowance. R10 remains in progress; dashboard, analytics, audit, metrics, and PI-05/PI-09 decisions are outstanding. No migrations were added or executed.
- Validation passed after extraction and ordering changes: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture. Counted direct accesses fell from 9 across 7 files to 8 across 6 files; 9 tracked exceptions remain. Behavioral verification remains deferred.

### R10 dashboard projection

- Moved dashboard summary/read port and `DashboardApplication` into core. HTTP invokes the application, preserving `ReadDevices`, online-to-active response naming, and existing response fields. Both adapter crates own the projection SQL and numeric hydration; old host service/adapter modules are deleted.
- PostgreSQL now computes all counts in one statement, matching Turso's existing query shape and preventing a response from mixing four successive database snapshots. Zero devices yields zero counts; online/offline comparisons retain exact lowercase status matching. `total_messages` still counts retained raw telemetry, not lifetime traffic or typed contract events.
- Removed invalid host-service tests `passes_validated_tenant_identity_to_the_repository` and `authorization_happens_before_persistence` and their private mock/context support because the replaced host entry point no longer exists. No tests were added, repaired, compiled, or run.
- Removed the dashboard HTTP repository allowance. R10 remains open for analytics, audit, operational metrics, and the recorded projection semantic audits. No migrations were added or executed.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture. Direct handler accesses fell from 8 across 6 files to 7 across 5 files; 9 tracked exceptions remain. Behavioral projection verification remains deferred.

### R10 analytics projection extraction

- Moved analytics domain values, repository port, and `AnalyticsApplication` into core. Core owns numeric blueprint-field catalog discovery, scope/range validation, automatic/explicit bucket policy, device/point limits, per-device and fleet weighting, coverage, statistics, and response assembly. Preserved `ReadTelemetry` and existing 400/422 distinctions through `InvalidInput`/`InvalidOperation`.
- Both adapters now own analytics catalog/sample SQL, grouping inputs, and numeric row hydration using the existing pool/shared handles. Deleted host analytics service and adapter modules. Both HTTP endpoints invoke core; removed their two direct-repository allowances.
- Moved existing pure analytics tests with their implementation without changing their cases or running them. No tests were added, repaired, compiled, or removed. No migrations were added or executed.
- This extraction preserves current query behavior; the remaining R10 audit must resolve text ordering, negative-epoch bucket parity, catalog/device/sample snapshot consistency, and numeric semantics rather than treating compilation as parity evidence. Sample windows currently use `[start, end)` in both engines. Audit, operational metrics, PI-05/PI-09, and prior activity audit items remain outstanding.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture. Direct handler accesses fell from 7 across 5 files to 5 across 4 files; 9 tracked migration exceptions remain. Behavioral analytics verification remains deferred.

### R10 audit application and persistence

- Moved audit values/port and `AuditApplication` into core. Reads preserve `ReadServerMetrics`, tenant scope, and pagination inputs. Both adapters own insertion/list SQL and row hydration; deleted the host audit service, PostgreSQL SQL helper, and legacy adapter modules.
- Middleware now invokes `record_access` with its authenticated tenant attribution. It still handles failure as a diagnostic and returns the original HTTP response. Unauthenticated mutations remain structured host logs without invented tenant scope. Existing OTA URL redaction and path-only audit metadata remain in middleware; no signing token or request body is added to audit data.
- PostgreSQL audit listing now orders equal timestamps by ID with C collation; Turso explicitly uses BINARY for the same tie-breaker. Existing storage-time timestamp sources are preserved pending the complete R10 time-contract audit.
- Removed audit endpoint/middleware direct-repository allowances and the obsolete middleware `DEFAULT_TENANT_ID` allowance. No tests were added, repaired, compiled, run, or removed. No migrations were added or executed.
- Operational metrics and cross-adapter projection semantics remain outstanding in R10.
- Validation passed: combined PostgreSQL/Turso host production compilation, formatting, whitespace, and architecture. Counted direct accesses fell from 5 across 4 files to 3 across 2 files; 9 tracked exceptions remain. Behavioral audit failure/tenant verification remains deferred.
