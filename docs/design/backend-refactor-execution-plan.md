# Backend refactor execution plan

- **Baseline:** `9e8a529`, pushed on `refactoring/backend-continuation`.
- **Created:** 2026-09-14.
- **Purpose:** Executable work packages for completing the remaining backend ownership refactor.
- **Current execution preference:** Skip running, adding, and repairing tests for now. Remove tests made invalid by the refactor; keep unaffected tests. Continue production compilation, formatting, and architecture checks.
- **Next package:** R01 — certificates and key protection.

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

- Do not run tests, add new suites, repair obsolete tests, or expand test coverage now.
- When a refactor removes or changes an interface, behavior, fixture, or mock that an existing test depends on, remove the affected invalid test. Remove its dedicated fixture/mock/helper if nothing else uses it.
- Keep unaffected tests. Remove a whole test file or target only when all of its tests are obsolete; otherwise remove only the invalid cases.
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

- Move shadow merge/delta policy, configuration versions/acknowledgements, command validation, and command transition rules into core.
- Define atomic shadow mutation and command/configuration transitions with expected-version or equivalent compare-and-set operations. Preserve PostgreSQL shadow locking added by the recent OTA fixes.
- Implement ADR-003: authorize and validate, persist a `sent` dispatch attempt, publish through host `DeviceBus`, retain the row and return the existing 502 mapping on synchronous publish failure. Do not add automatic republishing.
- Resolve PI-01 using the approved status vocabulary: `sent`, `delivered`, `succeeded`, `failed`, `timed_out`. Keep historical Turso statuses readable through adapter mapping or an additive migration.
- Keep generated wire messages, topic construction, subscriptions, and Zenoh transport errors in the host. Pass domain values through the bus port.
- Share the resulting transitions with OTA/action callers rather than creating a second command/shadow mutation path.

**Implemented exit:** handlers and consumers invoke application operations; database failure prevents publication; state transitions and atomic mutation semantics are adapter-independent where ADR-003 requires convergence.

**Deferred acceptance:** concurrent desired updates, stale acknowledgements, terminal-state transitions, timeout eligibility, legacy states, retained dispatch row on 502, and no publication on validation/database failure.

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

## 7. Execution ledger

Update this table in every implementation PR; do not replace deferred evidence
with a generic “done.” List individual PRs if a package is split.

| Package | Implementation | Behavioral verification | Commit/PR and next action |
| --- | --- | --- | --- |
| Baseline CI ingest | Complete at `9e8a529` | Deferred | Four host profiles and architecture passed |
| R01 | Not started | Deferred | Start certificate call-site and PI-06/PI-14 review |
| R02 | Not started | Deferred | Follow R01 |
| R03 | Not started | Deferred | Device types, then fleets |
| R04 | Not started | Deferred | Blueprints, then provisioning aggregate |
| R05 | Not started | Deferred | Rules and snapshot ownership |
| R06 | Not started | Deferred | Alerts, then durable actions/outbox |
| R07 | Not started | Deferred | Shadows/configuration, then commands |
| R08 | Not started | Deferred | Ingress and time-series contracts |
| R09 | CI ingest only | Deferred | Remaining metadata/blob/OTA work |
| R10 | Not started | Deferred | Projections, audit, metrics |
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
