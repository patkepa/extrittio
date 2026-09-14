# Backend refactor continuation — 2026-09-14

## Starting point

Branch: `refactoring/backend-continuation`, created from freshly fetched
`origin/main` at `d292304` (merged PR #94, `refactoring/backend`).
The working tree was clean before this planning update.

Continue the modular-monolith ownership refactor defined in
[the architecture plan](backend-crate-architecture-plan.md), using its existing
[decisions](backend-crate-refactor-decisions.md) and
[persistence inventory](backend-persistence-contract-inventory.md).
This document records inspected implementation evidence and the next work order;
it does not mark unexecuted test suites as passing.

## What is already implemented

Continuation update: the user requested skipping tests and proceeding with the
extraction. Step 2's ownership work is now implemented: the existing CI firmware
endpoint uses core `CiIngestApplication`, both adapter crates implement its combined
repository operation, and the old host service, ingest method, and API-key
compatibility helpers are removed. ADR-009 records preserved transaction behavior.
Step 1 and behavioral acceptance tests remain deferred. Certificates/key protection
is the next implementation slice; the work order below records the original plan.

- Core, PostgreSQL, Turso, and shared adapter-test packages exist. Zones, roles,
  users/passwords, and API-key management have core application boundaries and
  adapter-owned implementations. `backend-core/src/repositories.rs` shows the
  current migrated surface.
- The users security correction is substantially implemented despite the older
  ledger: both adapters have epoch migrations and generate fresh principal
  epochs; PostgreSQL credential/session reads use repeatable-read transactions;
  Turso uses read transactions. Core checks session epochs. Shared user contracts
  exercise delete/recreate epochs, and host API tests exercise issued JWTs and
  replacement principals. These are source findings, not fresh execution proof.
- Commit `be9179e` extracted API-key create/list/delete, firmware creation helpers,
  and frontend fleet-graph/rule-dialog components. API-key authentication and
  firmware CI ingest still cross legacy boundaries.
- PR #94 subsequently fixed OTA authorization, command reservation, concurrency,
  stale reports, boot confirmation/rollback, desired-command cleanup, shadow
  subscription ordering, and durable native version restoration. Preserve those
  regressions when the firmware slice moves. Host helper extraction alone does
  not complete the core firmware work package.

## What remains and why this order

### 1. Restore verification coverage and reconcile the status ledger

Make this the first implementation PR on the continuation branch.

- Include all feature-gated adapter contracts in both database CI lanes.
  `.github/workflows/ci.yml` explicitly runs zones, roles, and users, but omits
  `postgres_api_keys` and `turso_api_keys`. The contract package defaults to no
  adapter features, so a default workspace test is insufficient.
- Update `tools/xtask/src/verify.rs` so backend verification includes both shared
  contract suites with their required features. Prefer whole-package contract
  runs over enumerating individual test targets, preventing future omissions.
- Require a disposable configured PostgreSQL database for verification. The
  PostgreSQL API-key test currently returns success after skipping when
  `DATABASE_URL` is absent; record actual execution, not only an exit code.
- Audit and run ADR-008's closure evidence: epoch backfill and non-empty/unique
  enforcement, new-principal epochs, Turso ID reuse, JWT missing/mismatched epoch,
  permission-version invalidation, and same-ID/same-version replacement rejection.
- Add any missing deterministic concurrency proof that credential/session reads
  cannot mix principal revisions. Existing owner/role concurrency tests do not
  by themselves prove snapshot consistency of authentication reads.
- Then update the architecture status, ADR-008 evidence, and PI-17 together.
  Do not repeat existing epoch implementation or rewrite applied migrations.
  PI-16 (embedded-NUL username parity) remains a distinct decision and test task.

Acceptance: both engines actually execute the contracts; closure claims link to
tests and results; each remaining proof gap is named explicitly.

### 2. Finish the API-key boundary, including firmware CI authentication

Start with `domains/identity/api_key_repo.rs`,
`domains/firmware/ci_pipeline_service.rs`, the firmware repository port, and both
legacy `persistence/*/firmware.rs` implementations.

- Characterize key lookup, tenant/device-type scope, revocation, `last_used_at`,
  duplicate firmware versions, errors, and concurrent use/delete behavior.
- Decide and record the transaction contract before splitting `ingest_ci`:
  Turso currently combines authentication/touch/insert in a transaction, while
  PostgreSQL uses separate statements and ignores the touch error. Preserve the
  known behavior during extraction unless an explicit correctness change is
  included with its tests and rollout implications.
- Move authorization/orchestration to core using narrow ports; keep hashing,
  secret generation, HTTP parsing, and concrete storage integrations in the host.
  Preserve the atomic storage operation required by the selected contract.
- Expand the shared API-key contract beyond management to scope, authentication,
  revocation, and selected concurrency semantics. Preserve the documented
  tenant/name uniqueness difference unless separately resolved.
- Delete superseded host helpers and shrink architecture allowances in the same
  slice. Keep the rest of firmware migration as P3.6.

The old plan mentions API-key nonce/rotation semantics. Inspection found no
implemented API-key nonce/rotation flow in the host/core; the concrete nonce use
is certificate encryption. Confirm intended scope and record deferral or a
separate feature decision rather than inventing a new API during refactoring.

Acceptance: management and existing key-authenticated ingest reach explicit
application operations, both engines satisfy the chosen contract, public key
format and HTTP responses remain compatible, and old paths are removed.

### 3. Complete the remaining identity slices

1. Certificates/key protection: move policy and orchestration behind core ports;
   retain Ring/rcgen, configuration, randomness, and encryption in host outbound
   implementations. Exercise the existing encrypted-key compatibility fixture.
2. Idempotent bootstrap: separate business seed operations from migrations and
   maintenance, prove retries on both adapters, and remove owned legacy bridges.
3. Resolve PI-16 with an explicit portable input contract and create/login/error
   fixtures; avoid an incidental validation change inside another extraction.

### 4. Resume the existing P3–P6 sequence

Continue device catalog/provisioning (including atomic rollback), rules/alerts/
durable actions, device state/commands/configuration, ingress/time series,
firmware, and projections. Thin HTTP/Zenoh/workers as each slice moves, then
finish composition/CLI/state and remove migration bridges. Keep unrelated
frontend refactoring in separate PRs; the latest frontend extraction does not
block backend identity completion.

## Delivery process

Use one reviewable vertical slice per PR: characterize behavior, define core
policy and ports, implement both adapters, route callers through the application,
delete old paths, reduce verifier allowances, and update the ledger. Follow
`CONTRIBUTING.md` for migrations and generated API contracts.

For each implementation slice, record architecture checks, core tests, affected
host/compatibility tests, both adapter contracts, and the existing feature/build
matrix. Representative contract commands are:

```sh
cargo xtask architecture
cargo test -p extrittio-backend-core
cargo test -p extrittio-backend-adapter-tests --features postgres
cargo test -p extrittio-backend-adapter-tests --features turso
cargo xtask verify backend
```

PostgreSQL requires a disposable database; `verify backend` needs the coverage
fix above before it can stand in for the separate adapter commands. Do not widen
architecture exceptions to accommodate new direct repository access.

## Verification performed for this planning change

`cargo xtask architecture` passed, with 12 tracked migration exceptions. It
reported 103 direct handler-to-repository accesses across 22 files, plus remaining
process-shell OpenThread/Turso composition and host rule-engine ownership. These
are measurable continuation targets, not new failures.

The figures above describe the initial planning checkpoint. After the CI-ingest
extraction, architecture checks passed with 102 direct accesses across 21 files
and the same 12 tracked exceptions. Host compilation passed with no adapter,
PostgreSQL only, Turso only, and both adapters. Formatting and whitespace checks
also passed.
Full backend/database suites were not run, as explicitly requested; their execution
and evidence reconciliation remain deferred.
