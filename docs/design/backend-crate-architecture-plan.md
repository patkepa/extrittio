# Backend Crate Architecture and Implementation Plan

- **Status:** In progress — P0, the P2 zones walking skeleton, and the P3.1 roles/permissions and users/passwords slices are implemented
- **Scope:** Refactor the current backend into a modular monolith with explicit compile-time boundaries
- **Primary packages:** `extrittio-backend-core`, `extrittio-backend-postgres`, `extrittio-backend-turso`, and `extrittio-backend`
- **Migration rule:** Preserve externally observable behavior unless a work package explicitly says otherwise

## Implementation status (2026-08-31)

This ledger describes the checked-in implementation, not the target state described by the rest of this document.

| Work package | State | Implemented and remaining work |
| --- | --- | --- |
| P0.1, P0.3 | Completed | The build/dependency baseline, known-failure record, and the 25-port/139-operation persistence inventory are checked in. |
| P0.2 | Completed | Golden coverage locks CLI behavior, device wire bytes, persisted rule actions, firmware object keys, public API errors/OpenAPI, encrypted certificate/key compatibility, the exact Turso backup manifest, PostgreSQL migration preflight, and Turso previous-snapshot upgrades. |
| P0.4 | Completed | `cargo xtask architecture` enforces package edges, forbidden core/adapter dependencies and imports, bounded legacy exceptions, and dependency-closure rules in CI. |
| P1.1–P1.2 | Partial | Core-owned errors and mandatory tenant/actor/permission context exist; the HTTP JWT compatibility mapper owns the legacy missing-tenant fallback. Zones use this boundary. Remaining domain, API-key, device, worker, and transport-error paths move with their slices. |
| P1.3 | Completed | Host business repositories are separated from `DatabaseRuntime` lifecycle/maintenance capabilities, schema migration is distinct from application bootstrap, and business persistence errors no longer contain migration failures. |
| P1.4 | Partial | `AppState` fields are private or `pub(crate)`, it stores the core `Application`, and the architecture verifier prevents growth in direct handler-to-repository access. Narrow substates and removal of the tracked legacy accesses remain. |
| P1.5 | Partial | PostgreSQL schema/models and initial Turso row/foundation types are adapter-owned. The remaining legacy row types, UTC/pagination conversions, and dependency-feature reductions move with their domain slices. |
| P2.1 | Completed | Core, PostgreSQL, Turso, and adapter-contract workspace packages exist; the host has no default database feature and supports no-adapter, single-adapter, and both-adapter builds. |
| P2.2–P2.3 | Partial | The adapters own their migration assets and cloneable engine handles; core owns the initial application/error/identity/zone contracts and a private, lifecycle-free `RepositorySet` consumed by `Application`. Unmigrated repositories still use host-local connection/row bridges; adapter ownership of the remaining foundations plus core pagination/time types and slice-driven outbound ports remain. |
| P2.4 | Completed | Zone CRUD runs through the core application façade and both adapter implementations, and rule-zone snapshot loading uses the adapter-owned system port rather than a legacy host query. The shared contract covers tenant isolation, deterministic binary ordering, uniqueness, not-found, in-use deletion, and CRUD behavior; PostgreSQL includes duplicate preflight plus the canonical uniqueness index. |
| P2.5 | Completed | Package rules are fatal and CI checks core, no-adapter host, PostgreSQL-only host, Turso-only host, both-adapter host, and the extracted packages directly. |
| P3.1 roles/permissions | Completed | Core owns the permission catalog, role types, authorization/orchestration, and `RoleRepository`; both adapter-owned implementations pass a separate shared role contract. HTTP routes use `Application`, user hydration uses the core tenant-aware `Role`, and all legacy host role services, ports, and implementations are deleted. ADR-007 records ordering, conflict, invalidation, and timestamp semantics. |
| P3.1 users/passwords | Completed | Core owns user/password policy and orchestration, typed user/credential models, pagination, the password/clock outbound ports, and `UserRepository`. PostgreSQL and Turso own the implementations and share one user contract; HTTP login/session/user routes use `Application`, host Argon2 remains injected, and the legacy host user services, ports, and repositories are deleted. ADR-008 records the canonical behavior and PI-16 records the remaining embedded-NUL parity gap. |
| P3.1 remaining; P3.2–P6 | Not started | API keys/nonces are the next P3.1 sub-slice, followed by certificates/key protection and bootstrap. Most handler orchestration, process-shell composition, compatibility bridges, and release cleanup remain. |

The next checkpoint is the P3.1 API-keys/nonces sub-slice. Continue removing broad compatibility access with each vertical slice; do not add a new handler-to-repository path.

## 1. Executive decision

Proceed with the four-crate split, but treat it as an ownership refactor rather than a file move.

The target architecture is:

- `extrittio-backend-core` owns domain types, use cases, authorization policy, business repository ports, and pure transformations.
- `extrittio-backend-postgres` implements those business ports with Diesel/PostgreSQL and owns PostgreSQL-specific schema and operational code.
- `extrittio-backend-turso` implements the same business ports with libSQL/Turso and owns Turso-specific schema and operational code.
- `extrittio-backend` is the runtime host. It owns HTTP, Zenoh, workers, configuration, adapter selection, database lifecycle orchestration, and concrete outbound integrations.
- `apps/extrittio` becomes a thin process shell that parses the top-level command and invokes the host. It must not construct database adapters directly.

The refactor should be delivered as vertical slices that work on both database adapters. Do not first move every PostgreSQL implementation and only later discover that the shared contract does not fit Turso.

The principal architectural rule is:

> Transports and adapters translate; the application layer decides; the database enforces durable invariants.

## 2. Why the current backend needs this split

Repository inspection found several boundaries that are currently implicit:

- `AppState` publicly exposes broad runtime and persistence dependencies.
- `Persistence` is a large aggregate with roughly two dozen public repositories plus backend metadata.
- authorization context is coupled to JWT claims and permits an optional tenant with default-tenant fallbacks.
- persistence errors mix business failures with migrations, corruption, and database implementation strings.
- bootstrap repositories mix health checks, migrations, checkpoints, and domain seed data.
- CLI code constructs adapters directly, including Turso-specific types.
- HTTP and Zenoh handlers contain application orchestration.
- the action worker and the pure action-to-outbox mapping are colocated.
- the persisted pending-action representation is not explicitly versioned.
- rule cache refresh is local to the process handling a rule mutation.
- some background and audit paths substitute the default tenant.
- command dispatch and firmware storage use dual writes without an explicit consistency contract.
- certificate code reads encryption configuration directly from the environment.

A crate split that leaves those ownership choices unchanged would make navigation different without making the architecture safer. This plan addresses those seams before and during extraction.

Primary implementation anchors are `crates/backend/src/state.rs`, `crates/backend/src/persistence/mod.rs`, `crates/backend/src/persistence/bootstrap.rs`, `crates/backend/src/auth/context.rs`, `apps/extrittio/src/commands/service.rs`, `crates/backend/src/domains/rules/rule_engine/actions.rs`, `crates/backend/src/domains/rules/rules.rs`, `crates/backend/src/background.rs`, `crates/backend/src/domains/commands/command_service.rs`, `crates/backend/src/domains/firmware/firmware_updates.rs`, and `crates/backend/src/domains/identity/cert_service.rs`. These paths are evidence for the plan, not target module names.

## 3. Goals

The completed refactor must:

1. Make the domain and application layer compile without Axum, Diesel, libSQL/Turso, Zenoh, object-store SDKs, or process environment access.
2. Make each database adapter compile and test independently.
3. Allow production builds to exclude Turso/libSQL from the dependency closure.
4. Allow edge builds to exclude Diesel/PostgreSQL from the dependency closure.
5. Use one business persistence contract and one shared semantic contract suite for both adapters.
6. Keep HTTP routes, JSON schemas, status codes, CLI behavior, wire protocols, migrations, backup formats, and object keys compatible.
7. Preserve tenant isolation, authorization, transactional behavior, ordering, time precision, and idempotency.
8. Make handlers and message consumers invoke application use cases instead of repositories.
9. Make architecture violations fail in CI rather than relying only on review.
10. Leave each migration pull request buildable, testable, and reversible.

## 4. Non-goals

This project does not introduce:

- microservices or separately deployed domain services;
- one crate per domain;
- CQRS, event sourcing, or a generic unit-of-work abstraction;
- a dependency-injection framework;
- a new public HTTP API or device protocol;
- a database schema redesign except where an explicitly approved correctness work package requires it;
- a generic abstraction over every library or pure helper;
- separate crates for every outbound integration;
- a rewrite of all domain services into object-oriented service structs;
- guaranteed atomicity across the database, Zenoh, and object storage without a separately designed durable workflow.

## 5. Terminology and ownership

### 5.1 Process shell

`apps/extrittio` owns process-level concerns only:

- top-level CLI parsing;
- tracing/process initialization that must happen before the host starts;
- selecting a supported runtime profile through Cargo features;
- mapping the host's final result to an exit code.

It does not import concrete database adapter crates, OpenThread, or persistence implementation types.

### 5.2 Runtime host

`extrittio-backend` owns composition and runtime behavior:

- configuration loading and validation;
- adapter selection and construction;
- HTTP routing, middleware, and OpenAPI generation;
- JWT parsing and transport identity extraction;
- Zenoh decoding, publishing, and subscription lifecycle;
- supervised workers;
- database migration, health, checkpoint, backup, and restore orchestration;
- outbound implementations such as object storage, webhooks, password hashing, key protection, and device messaging;
- shutdown, readiness, and liveness.

### 5.3 Core

`extrittio-backend-core` owns business meaning:

- domain entities, identifiers, value objects, and commands;
- application use cases and authorization decisions;
- business repository ports;
- explicit transactional operations required by invariants;
- pure mappings, including rule actions to a durable outbox representation;
- stable business and application errors;
- test builders that do not depend on a concrete adapter.

### 5.4 Storage adapters

Each storage adapter owns:

- connection and query implementation;
- schema/model/row translation;
- transaction implementation;
- migrations for that engine;
- database-specific health and maintenance operations;
- translation from implementation failures to the stable persistence error vocabulary.

An adapter does not know about HTTP, JWT, Zenoh, runtime configuration parsing, or another adapter.

## 6. Target dependency graph

An arrow means “may depend on.” No reverse edge is permitted.

```text
apps/extrittio
    └──> extrittio-backend
             ├──> extrittio-backend-core
             ├──> extrittio-backend-postgres  [feature = postgres]
             ├──> extrittio-backend-turso     [feature = turso]
             ├──> extrittio-common            [wire/runtime features only]
             ├──> extrittio-device-contract
             └──> extrittio-openthread-runtime [only if runtime integration needs it]

extrittio-backend-postgres
    └──> extrittio-backend-core

extrittio-backend-turso
    └──> extrittio-backend-core

extrittio-backend-core
    ├──> extrittio-device-contract            [domain-safe types only]
    ├──> extrittio-rule-engine
    └──> extrittio-common                     [core-safe, non-wire features only]
```

Core should initially use `extrittio-common` with `default-features = false` and the existing `alloc` feature for device-ID/status/OTA primitives. The current `std` feature pulls Prost and generated messages, so it is forbidden in core. If core needs the pure JSON shadow helpers, add a narrowly named JSON feature that does not enable Prost/`prost-build`, or move those helpers into core. Do not create a new shared dumping-ground crate.

Generated `extrittio_common::extrittio::*` messages are host wire types. Command and shadow use cases pass domain command/delta values to a host implementation, which chooses topics and performs Prost encoding.

Forbidden edges include:

- core to host or either adapter;
- one adapter to another adapter;
- adapters to Axum, JWT middleware, Zenoh, OpenThread, or host configuration;
- the process shell directly to an adapter;
- domain modules to transport error or request types.

## 7. Package responsibilities and public surfaces

### 7.1 `extrittio-backend-core`

Recommended internal layout:

```text
src/
  application/
    identity.rs
    devices.rs
    rules.rs
    telemetry.rs
    firmware.rs
    ...
  domain/
    identity/
    devices/
    rules/
    telemetry/
    ...
  ports/
    repositories.rs
    outbound.rs
  error.rs
  context.rs
  pagination.rs
  time.rs
  lib.rs
```

The exact module count can remain pragmatic. The important distinction is ownership, not a mandatory directory per entity.

Public exports should be curated from `lib.rs`. Database models, HTTP DTOs, implementation helpers, and generated wire messages must not leak through the core API.

The expected direct dependency set is deliberately small: Serde/JSON, `thiserror`, `chrono`, identifier support, `async-trait` (while the repository uses it), `extrittio-device-contract`, `extrittio-rule-engine`, and the non-wire `extrittio-common` feature described above. Core must not depend on Tokio, Axum/Tower, Diesel, Turso/libSQL, Prost, Zenoh, Reqwest, `object_store`, JWT, OpenThread, mDNS, telemetry exporters, system inspection, `dotenvy`, Argon2, `rcgen`, or Ring. Concrete crypto and network libraries implement core ports in the host.

### 7.2 `extrittio-backend-postgres`

Recommended layout:

```text
src/
  adapter.rs
  connection.rs
  executor.rs
  models/
  repositories/
  lifecycle.rs
  maintenance.rs
  schema.rs
  error.rs
  lib.rs
migrations/
diesel.toml
```

Its intentionally small public surface is:

- a PostgreSQL adapter/configuration constructor;
- a way to construct the core `RepositorySet`;
- concrete inherent methods for migration, health, and supported maintenance operations;
- a typed adapter error whose database source remains available to logging but is not exposed to clients.

### 7.3 `extrittio-backend-turso`

Recommended layout:

```text
src/
  adapter.rs
  connection.rs
  row.rs
  repositories/
  lifecycle.rs
  maintenance.rs
  migrations.rs
  error.rs
  lib.rs
```

Its public surface mirrors the PostgreSQL adapter's capabilities, but the concrete operational types may differ. Business compatibility comes from the core ports and contract suite, not from pretending both database engines have identical maintenance APIs.

### 7.4 `extrittio-backend`

Recommended layout:

```text
src/
  boot/
  cli/
  config/
  http/
  messaging/
  runtime/
  workers/
  outbound/
  database/
  error.rs
  lib.rs
```

The host should expose a small façade to `apps/extrittio`, for example:

```rust,ignore
pub async fn run(command: Command, process: ProcessContext) -> Result<ExitStatus, RuntimeError>;
```

`Command`/`ProcessContext` in this example are host-owned input types. The app owns Clap structs and converts them into that host API; the host must never import an app-owned command type. Exact naming can follow current conventions. The design requirement is that the app does not reproduce composition logic.

### 7.5 Test-only adapter contract package

Add a workspace package such as `extrittio-backend-adapter-tests` with `publish = false`.

It is not a fifth production layer. It depends on core and, behind test features, on the concrete adapters. Adapter crates must not depend back on it, which avoids a dependency cycle.

This package owns a test-local `ContractHarness` and reusable semantic suites. CI invokes it once for PostgreSQL and once for Turso.

## 8. Application boundary

### 8.1 `Application` is the enforcement boundary

The grouped `AppState` proposed in the earlier plan is useful but insufficient if handlers can still reach raw repositories. The final state is:

```text
HTTP / Zenoh / worker
        │ translate transport input + authenticate
        ▼
Application use case
        │ authorize + validate + coordinate
        ▼
Business ports / explicit transactional operation
        │
        ▼
Concrete adapter
```

`RepositorySet` is consumed by `Application::new` and is not stored separately in HTTP state. Its fields are private. Application accessors may expose narrow domain façades, such as `application.devices()` or `application.rules()`, but not repositories.

`Application::new` also receives a named dependency bundle containing only the core ports actually used by application behavior: rule snapshots, device bus, blob store, password/key protection, and deterministic clock/ID providers where required. Database lifecycle handles, Zenoh sessions, HTTP clients, metrics registries, and configuration objects are not part of this bundle.

Host runtime state should use private or `pub(crate)` fields and narrow substates:

- `HttpState`: application façade, authentication verifier, request metadata, and safe runtime information;
- `MessagingState`: application façade, decoder dependencies, and messaging publisher/subscriber handles;
- `WorkerState`: application façade plus the specific delivery or scheduling dependency for that worker;
- `OperationalState`: database lifecycle/maintenance handle, health registry, and shutdown controls.

Axum `FromRef` can derive narrow handler state where useful. It must not be used to make the full container globally reachable again.

### 8.2 What belongs in an application use case

A use case owns:

- authorization and tenant scope;
- semantic validation;
- coordinating one or more ports;
- choosing an explicit transactional repository method when partial success is invalid;
- converting persistence failures into stable application errors;
- producing durable side-effect intent where required.

A transport owns parsing, protocol validation, authentication extraction, response mapping, and transport-specific telemetry.

### 8.3 Avoid ceremonial service objects

A pure function may remain a function. Introduce a service/façade when it enforces a dependency or policy boundary, not solely to relocate code.

## 9. Identity, authorization, and tenancy

### 9.1 JWT claims stop at the host boundary

Core must not depend on `Claims` or JWT libraries. The host validates a credential and maps it to core-owned identity:

```rust,ignore
pub struct TenantContext {
    tenant_id: TenantId,
    actor: Actor,
    permissions: PermissionSet,
    request_id: RequestId,
}

pub enum Actor {
    User(UserId),
    ApiKey(ApiKeyId),
    Device(DeviceId),
    System(SystemActor),
}
```

The precise fields may evolve, but these invariants are mandatory:

- tenant scope is non-optional for tenant-scoped use cases;
- `PermissionSet` cannot be freely constructed by a handler;
- legacy tokens without an explicit tenant are mapped at the host compatibility boundary, not inside core or a repository;
- device and system execution paths use explicit identities rather than pretending to be a user;
- authorization returns an application/domain error, never an Axum response error.

### 9.2 Tenant-scoped and system-scoped operations are distinct

All tenant business repository methods accept a `TenantId` or a tenant-qualified identifier. Cross-tenant administration, migration, health, and global maintenance use separate APIs.

Do not provide a generic optional-tenant query. Do not substitute `DEFAULT_TENANT_ID` in workers, audit, or ingress after authentication. A compatibility fallback is allowed only where existing external credentials genuinely omit the tenant, and it must be tested and documented.

### 9.3 Authorization tests

Every migrated domain slice must include:

- allowed permission;
- missing permission;
- wrong tenant;
- absent/deleted actor where applicable;
- legacy-token compatibility if the route currently supports it;
- device/system context behavior for non-HTTP entry points.

## 10. Business persistence contract

### 10.1 `RepositorySet`

Core owns a cohesive `RepositorySet` made of business ports. It replaces the current `Persistence` name so that operational database concerns are not implied.

Its fields remain private. Construction is through a deliberate constructor or builder used by adapters; production access is restricted to application modules. Do not expose a public bag of `Arc<dyn Repository>` fields.

The external contract-test package still needs to exercise individual ports. Enable test-only port accessors through a non-default core feature such as `contract-testing`; the production feature matrix must prove that this feature is absent from release dependency closures. Host production code is forbidden from enabling or using it.

Repository traits should be grouped by behavior and aggregate boundary, not automatically one trait per table. Existing repository traits can be retained initially where they already express a useful boundary.

### 10.2 Keep lifecycle out of the business contract

The following are not core repository operations:

- connect or initialize a database;
- run migrations;
- health/readiness probes;
- checkpoint or compact;
- backup and restore;
- inspect storage paths;
- report engine capabilities.

Each adapter exposes those as concrete inherent operations. The host normalizes them behind a host-local enum or trait, for example `DatabaseRuntime`. This preserves the dependency direction: adapters never depend on a host-owned lifecycle trait.

The host-owned composition result conceptually contains:

```text
DatabaseRuntime
  ├── RepositorySet       -> consumed by Application
  ├── lifecycle handle    -> retained by OperationalState
  ├── maintenance support -> retained only where available
  └── capability metadata -> retained by CLI/health/diagnostics
```

Schema migration failures map to `LifecycleError`, not `PersistenceError`.

Domain bootstrap data is separate from schema migration. The host runs migrations, then invokes an idempotent application bootstrap use case through business ports.

### 10.3 Transaction boundaries are explicit

Do not add a generic transaction closure or unit-of-work trait. Encode durable invariants as named operations, implemented atomically by each adapter. Examples include:

- create a device together with required credentials and initial state;
- consume an API-key nonce or rotate a key exactly once;
- persist telemetry/event data and enqueue resulting durable actions;
- claim an outbox batch with leasing semantics;
- mark delivery success/failure with retry metadata;
- perform compare-and-set shadow/config updates;
- transition alert state with cooldown/idempotency enforcement.

The application layer selects these methods because it owns the invariant. SQL transaction primitives do not escape the adapter.

### 10.4 Expected outcomes are typed

Expected races and business results must not be inferred from vendor strings such as a constraint name. Prefer outcomes such as:

```rust,ignore
enum CreateResult<T> { Created(T), AlreadyExists }
enum CompareAndSetResult<T> { Updated(T), VersionMismatch }
enum ClaimResult<T> { Claimed(T), Empty, LeaseLost }
```

Use errors for exceptional failures. Translate unique violations to a semantic conflict only where the adapter knows which invariant was violated.

### 10.5 Cross-adapter semantic rules

Every port must document and test:

- tenant filtering;
- not-found versus empty-result behavior;
- conflict/idempotency behavior;
- ordering and tie breakers;
- pagination boundary behavior and maximum limits;
- timestamp timezone and precision;
- null and empty-string behavior;
- case sensitivity and collation assumptions;
- transaction visibility and rollback;
- concurrent update/claim behavior;
- stable identifier and enum encoding;
- retry safety.

Core uses a single UTC time representation. Prefer `DateTime<Utc>` behind a `UtcTimestamp` alias at the boundary and adapt database precision explicitly. Do not combine a broad timestamp rewrite with crate movement; preserve existing serialized forms with fixtures.

Core pagination types contain semantic data such as cursor/offset, limit, and results. HTTP query extraction and response DTOs remain in the host.

## 11. Error model

Use separate error layers:

- `DomainError`: invalid state or invariant violation;
- `ApplicationError`: authorization, expected absence/conflict, validation, and mapped dependency failure;
- `PersistenceError`: stable business-port storage failures;
- adapter errors: detailed Diesel/libSQL failures and sources;
- `LifecycleError` and `MaintenanceError`: host operational failures;
- `ApiError`: HTTP status and safe response body;
- `RuntimeError`: boot, configuration, worker, or shutdown failure.

`PersistenceError` should use stable variants such as unavailable, serialization conflict, timeout, data integrity, and internal. It must not contain migration variants or expose raw SQL, table names, constraint strings, tokens, queries, or customer data through `Display`.

Keep the original source chain for tracing. The API mapping emits a safe public message and a stable error code. Expected application outcomes should not be logged as infrastructure faults.

Add mapping tests for every public API error currently relied on by clients.

## 12. Outbound ports and durable side effects

### 12.1 Port only real boundaries

Core-owned outbound ports are justified where a use case must invoke an effect and production/test implementations differ. The initial core ports are:

- `DeviceBus` for device command publication;
- `BlobStore` for firmware objects;
- `PasswordHasher` or credential verifier where policy is core-owned;
- `KeyProtector`/certificate issuer if certificate creation remains an application use case;
- clock and ID generation only where deterministic tests or correctness require them.

`WebhookSender` is a host-local delivery interface because the application persists webhook intent and the host outbox worker delivers it. It should become a core port only if the product deliberately introduces synchronous webhook delivery semantics.

Do not wrap pure libraries behind traits merely for symmetry.

Configuration, secrets, and environment lookup stay in the host. In particular, certificate/application code receives validated key material or a `KeyProtector`; it never reads an environment variable itself.

### 12.2 Durable action/outbox contract

Separate three concerns:

1. core maps a rule result to durable intent;
2. a transactional repository operation persists business data and the intent;
3. a host worker claims, delivers, retries, and marks the intent.

Persist an explicit envelope version, not an unversioned Rust enum layout:

```rust,ignore
struct OutboxEnvelope {
    schema_version: u16,
    kind: OutboxKind,
    payload: serde_json::Value,
}
```

Use a typed `V1` payload at encode/decode boundaries and retain golden JSON fixtures. The decoder first recognizes the new envelope and falls back to the existing unversioned payload as legacy `V0`; new writes use `V1`. Adding another payload version requires backward-reading tests. This requires no outbox table rewrite and does not strand pending rows created by the old binary.

If backend versions overlap during rolling deployment, introduce `V1` in two releases: first deploy the tolerant reader while continuing to write `V0`; only after old workers are drained may a later release write `V1`. Never let a new producer write a payload that an old concurrently running worker cannot decode.

### 12.3 Device commands

The refactor preserves the current observable ordering of “record command, then publish through Zenoh” unless a separate migration is approved. Move that orchestration into an application use case and codify behavior when publish fails, including the retained command status and API result.

A durable command-dispatch outbox is recommended after the structural split. It is a reliability improvement, not something to hide inside a crate move.

### 12.4 Firmware object metadata

Move object-store and metadata coordination out of the HTTP handler into a firmware application use case. Preserve existing object keys and best-effort compensation, and add failure-injection tests for:

- object upload succeeds but metadata insert fails;
- metadata deletion succeeds but object deletion fails;
- retry of the same request;
- replacement/version conflict.

Crash-safe atomicity across object storage and the database requires a persisted workflow/state machine and is a follow-up design if the product requires that guarantee.

### 12.5 Webhook URL safety

Core owns pure semantic policy such as supported schemes and required URL shape. The host's HTTP client owns network enforcement, including DNS resolution, private-address restrictions, redirect revalidation, TLS, timeouts, and response-size limits. Tests must cover redirect and DNS-rebinding-sensitive paths at the host boundary.

### 12.6 Audit semantics

This refactor preserves current audit durability rather than silently redefining it:

- request/access audit remains a best-effort host concern;
- it uses the authenticated tenant and never invents the default tenant;
- domain-significant audit that must be atomic with a mutation is not claimed as guaranteed until it is implemented through an explicit transactional operation or outbox.

Classify required domain audit events in a follow-up ADR before strengthening the guarantee.

## 13. Rules, cache, and multi-process correctness

The current process-local rule cache cannot use “the process that handled the mutation refreshed its cache” as a production consistency guarantee.

During extraction:

- define a core-owned `RuleSnapshots` port over immutable, revisioned snapshot values, with `current` and post-commit `invalidate` behavior;
- introduce a host-owned `RuleSnapshotStore` implementation of that port;
- application rule evaluation reads the current snapshot only through that port;
- every process performs a full load at startup;
- successful rule mutations invalidate after commit; the host schedules an immediate local reload without making the already-committed API result depend on reload success;
- every process also polls/reloads on a configurable bounded interval so changes made through another replica become visible;
- mutable cooldown, active-alert, zone-entry, and idempotency state remains database-authoritative;
- durable uniqueness/claim operations prevent duplicate externally visible actions under concurrent evaluation.

The system-scoped rule-snapshot repository loads and compiles the durable view; a supervised host task invokes that application operation rather than reading a repository directly. The initial implementation may compute the revision from existing durable update metadata/content hashing or reload the snapshot wholesale to avoid a schema change. Record the maximum staleness in configuration and operational documentation. If reload cost is unacceptable, add a durable revision/invalidation mechanism as a separately approved schema change.

Contract and integration tests must cover two independent application/runtime instances sharing one database: one mutates a rule and the other observes it within the configured bound.

## 14. Feature and build model

The host library has no default database adapter and retains its orthogonal runtime capability features:

```toml
[features]
default = []
postgres = ["dep:extrittio-backend-postgres"]
turso = ["dep:extrittio-backend-turso"]
production = ["postgres", "s3", "otlp", "swagger-ui", "mdns"]
edge = ["turso", "embedded-ui", "swagger-ui", "mdns"]
all-databases = ["postgres", "turso"]
# Existing openapi, swagger-ui, embedded-ui, s3, otlp, and mdns
# features remain independently selectable.
```

The process package keeps PostgreSQL as its developer default for compatibility and forwards its selected profile to the host. The allocator feature remains in `apps/extrittio`, because a library must not own the process global allocator after the duplicate backend binary is removed. Preserve the existing production/edge capability bundles and add compile-time errors for invalid production-plus-edge combinations.

Remove the current crate-wide “at least one database backend” `compile_error!` from the host library. Validate adapter availability when constructing `DatabaseRuntime`, and enforce deployment-profile requirements in `apps/extrittio`; otherwise an OpenAPI-only host build can never compile without an adapter.

Required properties:

- core compiles by itself;
- each adapter compiles by itself;
- host/OpenAPI code compiles without a concrete database;
- host compiles with PostgreSQL only;
- host compiles with Turso only;
- a test/dev build can include both adapters;
- release artifacts contain only the selected database engine.

Remove the duplicate backend binary after the app shell owns the sole executable entry point.

## 15. Architecture enforcement

Extend `xtask verify` with an `architecture` check. Use `cargo metadata` for package dependency rules and a small source/import check for module-level restrictions that Cargo cannot express.

At minimum CI must reject:

- forbidden dependency edges from Section 6;
- Tokio, Axum/Tower, Diesel, libSQL/Turso, Prost, Zenoh, Reqwest, object-store/JWT/OpenThread/concrete-crypto SDKs, or environment access in core;
- host/transport imports in adapters;
- adapter imports from `apps/extrittio`;
- concrete adapter paths outside approved host `database`, `boot`, `maintenance`, and test modules;
- public raw repository fields in runtime state;
- `DEFAULT_TENANT_ID` use outside explicitly allowlisted bootstrap/legacy-credential code;
- unguarded Diesel in the edge dependency closure;
- unguarded libSQL/Turso in the production dependency closure.

Prefer structural checks over fragile textual rules, but retain a short allowlist for constraints only visible at source level. Every allowlist entry needs an owner comment and removal condition.

## 16. Migration strategy

### 16.1 General rules

- Move one complete behavior slice through core, both adapters, host, and tests before starting the next slice.
- Keep mechanical moves separate from semantic changes where practical.
- Preserve public routes, DTOs, wire formats, migration ordering, feature names, and CLI output with characterization fixtures.
- Temporary re-exports or compatibility shims must have a removal issue/work-package ID and may not survive the final phase.
- Do not maintain two writable implementations of the same use case longer than one slice.
- A slice is complete only after both adapters pass the shared contract suite.
- Prefer `git mv` for history-preserving mechanical relocations during implementation.

### 16.2 Dependency order

```text
P0 Baseline and semantic lock
  └──> P1 In-crate boundary cleanup
        └──> P2 Crate scaffolding and pilot slice
              └──> P3 Domain vertical slices
                    └──> P4 Transport/outbound thinning
                          └──> P5 Composition, CLI, and feature cleanup
                                └──> P6 Removal and release verification
```

P3 slices have their own order because later behavior depends on earlier domain contracts.

## 17. Detailed implementation work packages

### P0 — Baseline, decisions, and compatibility lock

**Objective:** Capture behavior before changing ownership.

#### P0.1 Build and dependency baseline

- Record the current successful commands for workspace, PostgreSQL, and edge/Turso builds.
- Capture `cargo metadata` and `cargo tree` dependency closures for the backend and app.
- Record current test inventory and any tests requiring external PostgreSQL.
- Record release binary linkage/size for production and edge profiles.
- Add a short list of known pre-existing failures; do not normalize new failures into that list.

#### P0.2 External compatibility fixtures

Create or update fixtures/tests for:

- generated OpenAPI JSON;
- route paths, methods, status codes, and representative error bodies;
- JWT claims, legacy missing-tenant behavior, API keys, and permissions;
- device event/telemetry/config/shadow/command wire messages;
- CLI help, JSON/text output, exit status, and relevant environment names;
- persisted outbox/action payloads;
- firmware object keys and metadata representation;
- encrypted certificate/key material compatibility;
- Turso backup/restore archive structure and path rules;
- PostgreSQL and Turso migration order/checksums where the tooling supports it.

Normalize only nondeterministic fields such as timestamps and generated IDs in golden tests.

#### P0.3 Persistence semantic inventory

For every existing repository method, record:

- caller/use case;
- tenant scope;
- empty/not-found/conflict semantics;
- ordering and pagination;
- transaction participation;
- time precision;
- retry/idempotency expectations;
- currently covered tests;
- target core port or operational API.

This inventory is the migration checklist; a method is removed from the old aggregate only when its target behavior is tested.

#### P0.4 Architecture verifier skeleton

- Add an `xtask architecture` command.
- Encode the target package-edge allowlist before new crates are populated.
- Add CI invocation in warning mode only for pre-existing source-level violations.
- Make new dependency-edge violations fatal immediately.
- Track each initial allowlist entry against P1–P5.

#### P0 exit criteria

- all compatibility fixtures pass on the unrefactored backend;
- all repository methods appear in the semantic inventory;
- current dependency closures are captured;
- architecture checks run in CI;
- unresolved behavior questions are recorded as ADRs, not left implicit.

P0 may be delivered as three reviewable, non-behavioral pull requests: **P0-A** baseline/inventory/ADRs, **P0-B** compatibility fixtures, and **P0-C** architecture checks/current-profile CI. P1 starts only after all three are green.

### P1 — Establish boundaries inside the current crate

**Objective:** Separate business, transport, and operational code before changing package paths.

#### P1.1 Remove transport types from domain code

- Move request/response DTOs and Axum extractors into `http` modules.
- Replace domain references to `AppError` with `DomainError`/`ApplicationError`.
- Split HTTP pagination extraction from core pagination semantics.
- Keep serde representations stable through explicit DTO mapping.

#### P1.2 Introduce core identity context

- Add `TenantId`, `Actor`, `PermissionSet`, and `TenantContext` in a backend-core-shaped module.
- Map validated JWT/API-key/device identity to that context in host-shaped modules.
- Remove optional tenant from application calls.
- Isolate the legacy default-tenant token fallback in one compatibility mapper.
- Remove default-tenant fallback from audit, workers, and repository calls.

#### P1.3 Separate business repositories from lifecycle

- Rename or introduce `RepositorySet` for business ports.
- Move health, migration, checkpoint, backup/restore, backend descriptor, and path inspection behind an internal `DatabaseRuntime` façade.
- Split schema migration from idempotent domain bootstrap.
- Remove `Migration` from the business persistence error vocabulary.

#### P1.4 Restrict state

- Make existing `AppState` fields private or `pub(crate)`.
- Add narrow accessors/substates.
- Route new/changed handlers through an application façade; do not perform a flag-day conversion yet.
- Add an architecture check preventing new handler-to-repository access.

#### P1.5 Prepare adapter-neutral models

- Move legacy Diesel-only files under PostgreSQL-specific module paths.
- Separate Diesel schema/model types from domain types.
- Separate Turso row decoding types from domain types.
- Add explicit UTC/time and pagination conversions at adapter boundaries.
- Reduce `extrittio-common` features pulled into domain-shaped modules if Prost is currently forced in.

#### P1 exit criteria

- no domain-shaped module imports Axum or JWT claims;
- new code cannot reach public `AppState` repository fields;
- operational methods are absent from `RepositorySet`;
- all existing tests and fixtures remain green;
- architecture source allowlist is smaller than at P0.

### P2 — Scaffold crates and prove a walking skeleton

**Objective:** Validate the package graph with one low-risk vertical slice before bulk migration.

#### P2.1 Create workspace packages

- Create `crates/backend-core`, `crates/backend-postgres`, `crates/backend-turso`, and the test-only `crates/backend-adapter-tests` package.
- Add explicit workspace dependencies and feature forwarding.
- Set `extrittio-backend` default features to empty.
- Add temporary compatibility re-exports from the old host paths only where needed for incremental compilation.

#### P2.2 Move adapter foundations

Move without changing behavior:

- PostgreSQL connection/executor, schema generation, migration assets, and database-specific errors;
- Turso connection/row helpers, migration assets, backup path helpers, and database-specific errors;
- Diesel configuration and all path references in tests, CI, contributor documentation, and embedded-migration macros.

Do **not** move all per-domain repository implementations here. A repository implementation moves only after its core trait and domain types move in P2.4/P3; otherwise the new adapter would have to depend back on the host.

Use one temporary bridge during vertical migration:

- each new adapter owns a cloneable engine handle (`PostgresDatabase`/`TursoDatabase` or equivalent);
- a host-local legacy wrapper uses the same handle for unmigrated repository traits;
- migrated trait implementations live in the adapter crate on the adapter-owned handle;
- each slice deletes the matching legacy implementation;
- the bridge must not create a second pool/database, duplicate writes, or introduce an adapter-to-host dependency;
- any schema/model exports needed solely by the host wrapper are gated by a temporary `migration-bridge` feature and removed in P6.

The foundation path changes include:

- `crates/backend/src/db` to `crates/backend-postgres/src` model/schema modules;
- `crates/backend/src/persistence/postgres/executor.rs` and connection/adapter foundation code to `crates/backend-postgres/src`;
- `crates/backend/migrations/postgres` and `crates/backend/diesel.toml` to the PostgreSQL crate;
- `crates/backend/src/persistence/turso/database.rs`, `row.rs`, and adapter foundation code to `crates/backend-turso/src`;
- `crates/backend/migrations/turso` to the Turso crate;
- embedded paths in `crates/backend/src/lib.rs`, `crates/backend/tests/api_tests.rs`, `crates/backend/tests/cert_tests.rs`, and Turso `include_str!` declarations;
- documented paths in `CONTRIBUTING.md` and `docs/architecture/overview.md`.

The remaining files below `persistence/postgres` and `persistence/turso` move with their P2.4/P3 domain slices.

At this point old domain repository implementations may still call these foundations through temporary internal paths.

#### P2.3 Establish core foundations

Move the stable versions of:

- identifiers and value objects;
- tenant/actor context;
- domain/application/persistence errors;
- pagination and UTC time types;
- repository and outbound port modules;
- `RepositorySet` construction API;
- minimal `Application` façade.

#### P2.4 Pilot slice: zones

Use a simple tenant-scoped domain such as zones as the walking skeleton:

1. move its domain types, geometry validation, and use cases to core;
2. define/document its tenant CRUD port semantics;
3. move cross-tenant `list_all` out of the zone CRUD port and into the system-scoped rule-snapshot loading contract;
4. implement the port in PostgreSQL and Turso crates;
5. add the shared adapter contract tests;
6. adapt HTTP handlers to the application use case;
7. delete the old repository paths for the slice;
8. prove tenant isolation, stable ordering, in-use conflict, not-found behavior, and geometry error mapping.

Repository inspection found that PostgreSQL sorted zones by `created_at DESC`, while Turso sorted by `name, id`. ADR-006 makes `name ASC, id ASC` the canonical list contract as an explicit compatibility-noted correction: it is deterministic and matches other catalog-style lists. The same ADR resolves the schema mismatch in favor of per-tenant binary name uniqueness, preserving Turso's shipped invariant; PostgreSQL adds a duplicate-data preflight and unique index. Both implementations and the shared contract suite must change together.

If zones reveal an unsuitable generic abstraction, fix the core contract before moving the next slice.

#### P2.5 Turn architecture checks fatal for package rules

- verify the target graph from `cargo metadata`;
- verify core's forbidden dependency list;
- verify the host can compile with no database for OpenAPI/test-only targets;
- verify each single-adapter feature build.

#### P2 exit criteria

- all four production packages compile in their intended feature combinations;
- zones execute through core and both concrete adapters;
- zones pass one shared semantic suite;
- no old zones persistence implementation remains;
- migrations still run from their new locations;
- production and edge dependency-closure checks pass.

### P3 — Migrate domain behavior as vertical slices

**Objective:** Remove the old broad persistence/application surface slice by slice.

Each slice follows the same checklist:

1. characterize behavior and fill missing tests;
2. move domain types and policy to core;
3. define stable port methods and explicit transaction operations;
4. implement PostgreSQL and Turso behavior;
5. run the shared contract suite against both;
6. move orchestration into application use cases;
7. reduce HTTP/Zenoh/worker callers to translation;
8. delete old code and compatibility exports;
9. run the full profile matrix and dependency checks.

#### P3.1 Identity and bootstrap

Move in this order:

- roles and permissions;
- users and password operations;
- API keys and nonce/rotation semantics;
- certificates/key protection;
- idempotent application bootstrap and default data.

Special acceptance checks:

- JWT and API-key compatibility;
- permission-denied mappings;
- tenant isolation;
- encrypted key material can still be read;
- no core environment access;
- concurrent API-key/nonce operations have identical outcomes on both adapters.

#### P3.2 Device catalog and provisioning

Move:

- device types;
- fleets;
- blueprints;
- devices and provisioning;
- initial configuration/shadow/credential creation needed by device creation.

Define one explicit atomic provisioning operation. Test rollback after each injected failure point and duplicate/retry behavior.

#### P3.3 Rules, alerts, and durable actions

Move:

- rule definitions and validation;
- rule evaluation coordination;
- alerts and cooldown transitions;
- outbox persistence/claim/retry operations;
- action-to-versioned-envelope mapping.

Add `RuleSnapshotStore` at the host boundary and two-runtime refresh tests. Make database constraints/claims authoritative for duplicate prevention.

#### P3.4 Device state, commands, and configuration

Move:

- reported/desired shadows;
- configuration versions and acknowledgements;
- command creation/status/history;
- associated compare-and-set or idempotency operations.

Keep Zenoh publication in a host `DeviceBus` implementation. Lock down publish-failure behavior with tests before moving the route/consumer.

#### P3.5 Ingress and time-series behavior

Move application behavior for:

- telemetry;
- events;
- device logs;
- presence/activity updates;
- ingestion-triggered rule evaluation and action enqueueing.

Zenoh code should decode, authenticate device identity, map to a core command, invoke the use case, and record transport metrics. It must not coordinate repositories.

Test malformed protocol input at the host and semantic invalidity in core. Test atomic rollback between ingested data and durable action intent.

#### P3.6 Firmware

Move firmware metadata and object-store orchestration behind the firmware application façade. Preserve keys and response behavior. Add failure injection and retry tests described in Section 12.4.

#### P3.7 Projections and operational read models

Move:

- analytics queries;
- dashboard projections;
- activity feeds;
- audit persistence where it represents stored request/access audit;
- metrics-derived database reads.

Document ordering, aggregation windows, empty results, timezone boundaries, and database-specific numeric behavior in the contract suite.

#### P3 exit criteria

- every business repository method is implemented in a new adapter crate or intentionally removed;
- the old `Persistence` aggregate and old adapter repository modules are gone;
- all handlers/workers/consumers invoke application façades;
- both adapters pass the complete shared contract suite;
- no concrete database type is visible in core or transport handler signatures.

### P4 — Thin transports and isolate outbound runtimes

**Objective:** Finish moving orchestration out of entry points and separate pure intent from delivery.

#### P4.1 HTTP

- organize routes by transport domain without duplicating core domain modules;
- retain DTO conversion and `ApiError` mapping in the host;
- ensure handlers contain no multi-repository coordination;
- generate and compare OpenAPI without constructing a database;
- retain middleware ordering and extensions with integration tests.

#### P4.2 Zenoh/device ingress

- keep topic parsing, wire decoding, subscription/publish setup, and transport retries in the host;
- use explicit `Device` actor context;
- invoke application commands for all business decisions;
- isolate OpenThread integration in the host and remove it from the app shell.

#### P4.3 Workers

Split each worker into:

- scheduling/supervision in the host;
- claiming and state transitions through application/core ports;
- concrete delivery through a host outbound implementation;
- structured success/failure reporting.

Workers must shut down cooperatively and expose readiness/health without holding database-specific types in general runtime state.

#### P4.4 Security-sensitive outbound work

- inject certificate key protection rather than reading the environment;
- enforce webhook destination policy at every redirect/resolution;
- ensure logs redact credentials, payload secrets, and database details;
- test timeout, cancellation, and retry classification.

#### P4 exit criteria

- HTTP and Zenoh modules perform translation, authentication extraction, and response/transport work only;
- pure outbox mapping lives in core and delivery lives in host workers;
- outbound concrete SDKs are absent from core;
- security and failure-injection tests pass.

### P5 — Composition, CLI, state, and feature cleanup

**Objective:** Make the host the sole composition root and prove artifact isolation.

#### P5.1 Database runtime factory

- implement the host-local `DatabaseRuntime` enum/trait façade;
- construct the selected adapter behind feature gates;
- pass only `RepositorySet` into `Application`;
- retain lifecycle/maintenance handles only in `OperationalState`;
- expose capability-aware CLI operations with explicit unsupported-operation errors.

#### P5.2 Runtime state

- replace broad `AppState` with the narrow states from Section 8;
- make fields private;
- remove repository accessors from transport state;
- ensure workers receive only their required façade and outbound dependency.

#### P5.3 CLI and executable ownership

- move service/database orchestration from `apps/extrittio` into the backend host façade;
- remove all direct Turso/PostgreSQL/OpenThread imports from the app;
- preserve command names, flags, environment variables, output, and exit codes;
- remove the duplicate backend executable target.

#### P5.4 Feature normalization

- make host database defaults empty;
- retain a compatible app developer default if desired;
- add PostgreSQL-only, Turso-only, both-adapter test, and no-adapter checks;
- inspect `cargo tree` and final binaries to prove unwanted database engines are absent.

#### P5 exit criteria

- `apps/extrittio` imports only the backend host façade and process-level libraries;
- concrete adapters are referenced only in approved host composition/maintenance modules;
- general runtime state contains no adapter object;
- CLI compatibility fixtures pass;
- release artifacts contain only their intended database stack.

### P6 — Remove scaffolding and release

**Objective:** Eliminate temporary paths and demonstrate production readiness.

- remove compatibility re-exports, deprecated modules, old features, and allowlist entries;
- update architecture, contributor, migration, backup, deployment, and troubleshooting documentation;
- run formatting, linting, unit, contract, integration, security, and release-profile checks;
- run PostgreSQL and Turso migration tests from a supported previous database snapshot;
- restore a representative Turso backup and verify application reads;
- perform smoke tests for HTTP, device ingress, rules/actions, firmware, CLI maintenance, and graceful shutdown;
- compare performance and binary size with P0, investigating material regressions;
- verify a clean checkout can build both supported deployment profiles using documented commands.

#### P6 exit criteria

- no temporary architecture exceptions remain;
- all compatibility fixtures are unchanged or have an approved migration note;
- operational runbooks use only the new paths and commands;
- both deployment profiles pass the complete release matrix.

## 18. Shared adapter contract suite

### 18.1 Harness design

The test-only package defines a local abstraction similar to:

```rust,ignore
trait ContractHarness {
    async fn reset(&self);
    fn repositories(&self) -> RepositorySet;
    fn capabilities(&self) -> ContractCapabilities;
}
```

Implementations live in the test package and wrap each concrete adapter. This trait is test infrastructure, not a production adapter abstraction.

Use isolated PostgreSQL schemas/databases or serialized test namespaces, and a fresh temporary Turso database per test group. Tests must be deterministic and clean up through their harness rather than assuming global state.

### 18.2 Required suites

For every port or aggregate, share tests for:

- create/read/update/delete semantics;
- tenant isolation;
- conflicts and idempotent retry;
- missing records;
- stable ordering and pagination edges;
- timestamp precision/timezone round trips;
- nullable and text/case behavior;
- rollback on injected failure;
- concurrent compare-and-set/claim behavior;
- enum/identifier serialization;
- capability-specific behavior.

Add higher-level application contract tests for cross-repository invariants. Adapter unit tests may still cover SQL/query details that are not part of the shared semantic contract.

### 18.3 Capability handling

Do not weaken the common business contract to accommodate an engine-specific operational limitation. Business capabilities required by the product must work on both adapters. Operational capabilities such as checkpoint or archive format may differ and are tested conditionally through host/adapter-specific suites.

### 18.4 Test ownership

- core unit/application tests use fake ports and contain no database setup;
- the adapter-contract package owns shared semantic suites;
- each adapter owns query translation, migration, and engine-specific maintenance tests;
- the host owns HTTP/OpenAPI, middleware, Zenoh codec, worker, outbound-security, and composition tests;
- `apps/extrittio` owns top-level CLI parsing and exit-code compatibility tests;
- end-to-end profile smoke tests remain CI/deployment tests and exercise the assembled app.

Move the existing large inline Turso tests and PostgreSQL-heavy host tests incrementally with their behavior slices; do not perform a separate flag-day test relocation.

## 19. Verification matrix

The exact commands should be recorded in repository scripts/CI rather than copied indefinitely into this document. The matrix must cover:

| Check | Core | PostgreSQL | Turso/edge | Host no DB | Both adapters |
|---|---:|---:|---:|---:|---:|
| `cargo check` | required | required | required | required | required |
| unit tests | required | required | required | required | as applicable |
| shared adapter contracts | n/a | required | required | n/a | optional comparison |
| application integration tests | required with fakes | required | required | selected | required |
| OpenAPI golden | n/a | n/a | n/a | required | required |
| migrations from previous snapshot | n/a | required | required | n/a | n/a |
| backup/restore | n/a | engine-specific | required | n/a | n/a |
| architecture rules | required | required | required | required | required |
| dependency closure | required | no Turso | no Diesel | neither adapter | both expected |
| lint/format/docs | required | required | required | required | required |

Also run targeted concurrent tests for provisioning, outbox claims, API-key operations, shadows/config compare-and-set, alert transitions, and multi-runtime rule refresh.

## 20. Compatibility inventory

The following are frozen unless a separate migration is approved:

- HTTP paths, methods, JSON field names, status codes, pagination defaults, and documented errors;
- OpenAPI operation/schema identity where clients depend on it;
- JWT/API-key claim interpretation, including explicitly documented legacy fallback;
- device topics and binary/JSON wire encodings;
- persisted identifiers, enums, timestamps, and outbox payloads;
- PostgreSQL and Turso migration order and upgrade behavior;
- Turso backup archive layout, filenames, and restore behavior;
- firmware object key construction;
- encrypted certificate/key material format;
- CLI commands, flags, output modes, environment names, and exit statuses;
- worker retry, lease, and shutdown behavior;
- default-tenant bootstrap behavior, while eliminating unrelated runtime fallbacks.

If a compatibility fixture exposes a current bug, document it. Fix it in a separate behavioral change unless leaving it would violate tenant isolation or data integrity.

## 21. File migration map

Use this as a default routing guide, not as permission for blind directory moves:

| Current concern | Target |
|---|---|
| domain entities/value objects | `backend-core/domain` |
| authorization policy and use-case orchestration | `backend-core/application` |
| business repository traits | `backend-core/ports` |
| HTTP requests/responses/routes/middleware/OpenAPI | `backend/http` |
| Zenoh topics/codecs/subscriptions/publishers | `backend/messaging` |
| supervised worker loops | `backend/workers` |
| action-to-outbox pure mapping | `backend-core` |
| action delivery | `backend/workers` + `backend/outbound` |
| Diesel schema/models/executor/repos/migrations | `backend-postgres` |
| Turso rows/queries/repos/migrations/backup helpers | `backend-turso` |
| database selection/lifecycle normalization | `backend/database` |
| process/service CLI orchestration | `backend/cli` or `backend/boot` |
| top-level argument parsing/exit code | `apps/extrittio` |
| adapter semantic tests | `backend-adapter-tests` |

### 21.1 Current persistence-port migration ledger

The current `Persistence` aggregate is exhausted by this ledger:

| Current member | Target work package | Target boundary note |
|---|---|---|
| `activity` | P3.7 | application read model + business port |
| `analytics` | P3.7 | application read model + engine-specific queries |
| `api_keys` | P3.1 | identity use cases; explicit nonce/rotation operation |
| `alerts` | P3.3 | rules/alerts aggregate and transition operations |
| `audit` | P3.7 | stored request/access audit; durability remains explicit |
| `bootstrap` | P1.3 + P3.1 | split lifecycle from system-scoped domain bootstrap |
| `certificates` | P3.1 | identity use case with injected key protection |
| `commands` | P3.4 | command state port; publication through `DeviceBus` |
| `configuration` | P3.4 | versioned config/acknowledgement operations |
| `dashboard` | P3.7 | application read model |
| `device_blueprints` | P3.2 | catalog/contract application boundary |
| `device_types` | P3.2 | catalog port with explicit in-use deletion outcome |
| `devices` | P3.2 + P3.5 | split catalog/provisioning from ingress atomic writes |
| `events` | P3.5 | ingress and query semantics |
| `fleets` | P3.2 | tenant catalog port |
| `firmware` | P3.6 | metadata/status port; object storage is outbound |
| `logs` | P3.5 | device ingress/query port |
| `metrics` | P3.7 | operational read/write port, not database health |
| `outbox` | P3.3 | durable intent claim/lease/retry operations |
| `roles` | P3.1 (completed) | core authorization model and application façade; adapter-owned PostgreSQL/Turso implementations; separate shared role contract |
| `rules` | P3.3 | split tenant CRUD from system rule-snapshot loading |
| `shadows` | P3.4 | compare-and-set/update operations; publication outbound |
| `telemetry` | P3.5 | ingestion/query/retention semantics |
| `users` | P3.1 (completed) | core identity/password application façade and business port; adapter-owned PostgreSQL/Turso implementations; separate shared user contract; host-owned injected Argon2 and clock |
| `zones` | P2.4 | pilot tenant CRUD; cross-tenant loading moves to rules |

`BackendDescriptor` and its capabilities move to the host's `DatabaseRuntime`; they are not members of the new business `RepositorySet`.

## 22. Pull-request sizing and review order

Recommended pull-request boundaries:

1. P0-A baseline, persistence inventory, and ADRs.
2. P0-B compatibility fixtures.
3. P0-C architecture-check skeleton and current-profile/dependency-closure CI.
4. P1 transport/domain error and identity separation.
5. P1 repository/lifecycle separation and private state.
6. P2 package scaffolding and adapter foundations.
7. P2 zones walking skeleton and first shared contracts.
8. One pull request per P3 sub-slice, splitting further when a transactional aggregate is large.
9. P4 transport/outbound cleanup in independently testable groups.
10. P5 composition/state/CLI/features.
11. P6 cleanup, docs, and release verification.

Avoid a pull request that only copies all traits, another that only copies all PostgreSQL code, and a final one that attempts to reconnect behavior. Each behavioral slice should demonstrate the final path.

Every pull request description should state:

- behavior moved;
- compatibility fixtures exercised;
- old path removed or temporary shim and removal work package;
- both-adapter status;
- architecture exceptions added/removed;
- rollback approach;
- any intentional semantic change.

## 23. Risks and mitigations

### Semantic drift between adapters

Mitigation: shared contract tests, documented port semantics, and vertical slices on both engines.

### A core crate that is infrastructure-free but still an anemic trait bag

Mitigation: move authorization and orchestration into application use cases; prohibit handler repository access.

### Overly broad public state reappears under new names

Mitigation: private fields, narrow substates, curated exports, and architecture checks.

### Transactional regressions during repository splitting

Mitigation: name aggregate operations, failure-injection tests, concurrency tests, and no generic transaction abstraction.

### Production binary still links both database stacks

Mitigation: empty host defaults, single-adapter CI builds, `cargo tree` checks, and artifact/link inspection.

### Multi-process rule inconsistency

Mitigation: bounded cross-process snapshot refresh plus database-authoritative mutable state and idempotency.

### Stored payload or migration incompatibility

Mitigation: golden stored-data fixtures, versioned outbox envelopes, previous-snapshot migration tests, and restore smoke tests.

### Refactor never converges because compatibility shims accumulate

Mitigation: every shim has a named removal phase; P6 cannot complete with exceptions remaining.

### Excessive abstraction

Mitigation: introduce ports only at I/O/policy boundaries, retain pure functions, and do not add per-domain crates or a generic unit of work.

## 24. Recommended follow-up refactors

These are valuable but should be separate from the structural extraction unless a prerequisite above explicitly includes them:

1. Durable command-dispatch outbox so database state and device publication recover after crashes.
2. Schema-backed firmware upload/delete workflow if object/database crash consistency is required.
3. Transactional domain audit for the subset of events with compliance requirements.
4. Durable rule-set revision/invalidation if bounded polling is too costly or too stale.
5. Stronger typed identifiers for high-risk cross-aggregate IDs, preserving wire/database representation.
6. Finer `extrittio-common` features or smaller focused shared crates only after concrete dependency pressure is measured.

Do not add these to the critical path without an explicit decision, because each changes behavior or data rather than merely clarifying ownership.

## 25. Definition of done

The refactor is complete when:

- the four production crates have the dependency graph in Section 6;
- core has no transport, database, runtime SDK, or environment dependency;
- the process shell does not construct adapters;
- application use cases are the only path from transports/workers to business repositories;
- tenant scope is mandatory for tenant operations and no accidental default fallback remains;
- lifecycle and maintenance are absent from the business repository contract;
- both adapters pass the same documented semantic contract suite;
- all required transaction/concurrency tests pass on both engines;
- stored payloads, migrations, backup/restore, CLI, HTTP, and device compatibility fixtures pass;
- production excludes Turso/libSQL and edge excludes Diesel/PostgreSQL;
- OpenAPI can be produced without a database adapter;
- architecture enforcement is fatal in CI with no temporary allowlist entries;
- old persistence paths, duplicate binary ownership, and compatibility shims are removed;
- operational and contributor documentation describes the new architecture and commands.

## 26. Immediate next implementation stage

Continue **P3.1 identity/bootstrap one vertical slice at a time**, closing that slice's P1/P2 prerequisites as part of the same change. P1 and P2 remain partially open at the repository-wide level because their remaining work is distributed across unmigrated domains; they are not a flag-day gate in front of P3. The zones, roles/permissions, and users/passwords slices have established the repeatable path: prepare only the boundary and adapter foundations the slice needs, migrate both engines and callers, then remove that slice's escape hatch.

### 26.1 Apply the remaining P1 boundaries per slice

1. For each touched domain, complete its P1.1 work by moving HTTP error/status mapping into the host transport boundary and keeping `AppError`, Axum extractors, JWT claims, and HTTP DTOs out of core. Add safe public mapping tests for any newly introduced `ApplicationError` behavior.
2. Finish P1.2 at each identity entry point as it migrates: API-key, device, worker, and explicit system actors map to `TenantContext`; accidental default-tenant substitutions are removed; the missing-tenant legacy JWT fallback remains only in the documented compatibility mapper.
3. Preserve the completed P1.3 split: new business ports enter `RepositorySet`, operational capabilities stay on `DatabaseRuntime`, schema migration remains separate from idempotent application bootstrap, and neither lifecycle errors nor engine handles may leak back into business ports.
4. Complete P1.4 incrementally: derive narrow HTTP/messaging/worker/operational substates from the now-private `AppState`, and reduce the architecture verifier's tracked handler-to-repository access counts with every slice. Existing handlers may use temporary narrow accessors until their P3 slice, but no accessor may expose the repository bag and no allowlist cap may increase.
5. Complete only the P1.5 prerequisites needed by the active slice: adapter-owned row/schema types, explicit domain conversions, UTC precision rules, and pagination semantics. Keep Prost, Diesel, Turso, Axum, JWT, and environment access outside core.

### 26.2 Apply P2 foundations without a second migration track

1. Move connection, executor, row-decoding, lifecycle, health, and maintenance foundations to their adapter owners when the active slice reaches them. Keep exactly one shared pool/database handle and retain bridge exports only while an unmigrated repository still uses them.
2. Extend core's private `RepositorySet`, lifecycle-free business ports, pagination/time types, and outbound ports only for the active slice. Do not add placeholder abstractions for later domains.
3. Preserve the completed zones, roles, and users contracts as regression gates while adding a separate shared contract for the active slice. Keep system rule-snapshot behavior separate until P3.3.
4. Keep the CI matrix fatal for `core`, no-adapter/OpenAPI host, PostgreSQL-only host, Turso-only host, and `all-databases`; lint and test the three extracted packages directly, and run each shared adapter contract exactly once in its engine-specific job.
5. Extend the completed P0 compatibility lock only where the active inventory row exposes an uncovered writer or decoder; retain the existing public-error, legacy-credential, encrypted-key, backup-manifest, and migration-snapshot fixtures unchanged while ownership moves.

For each migrated slice, the closure checkpoint passes when callers use `Application`, the legacy port and both legacy implementations for that slice are gone, both adapters pass its shared contract, lifecycle concerns remain outside its business port, architecture debt decreases, and the applicable feature matrix is green. Repository-wide P1/P2 status becomes complete only after the last dependent slice removes the remaining shared bridges and broad runtime escape hatches.

### 26.3 Continue P3.1: identity and bootstrap

The roles/permissions and users/passwords sub-slices are complete. They established core-owned authorization and identity policy, application façades and ports, adapter-owned repositories, independent shared contracts, host `Application` routing, injected password/clock implementations, and deletion of their legacy host implementations. ADR-007 is the authority for role behavior; ADR-008 is the authority for user/password behavior, with the embedded-NUL cross-engine question remaining explicit as PI-16.

Continue one reviewable sub-slice at a time in this order: API keys/nonces, certificates/key protection, then idempotent bootstrap. For each remaining sub-slice:

1. characterize tenant, authorization, conflict, ordering, and transaction behavior;
2. move domain policy and use cases to core without transport or environment dependencies;
3. add the smallest stable business port or named atomic operation;
4. implement and run the same contract against PostgreSQL and Turso;
5. route HTTP/worker callers through `Application`, then delete the matching legacy implementation and bridge export;
6. verify JWT/API-key compatibility, permission-denied and public-error mappings, encrypted key readability, retry/concurrency behavior, and migration/bootstrap idempotency.

Do not begin P3.2 until all identity/bootstrap operations have left the legacy aggregate, both adapters pass the shared P3.1 contracts, and core can still be built and tested without any runtime or database SDK.
