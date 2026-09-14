# Backend Crate Refactor Decision Records

- **Status:** Accepted for the backend crate refactor
- **Scope:** P0-A decisions required by `backend-crate-architecture-plan.md`
- **Decision date:** 2026-08-31
- **Compatibility rule:** Preserve externally observable behavior unless this document explicitly approves a correction

This document resolves the behavior choices that must be stable before code moves between crates. It is subordinate to the architecture plan, but it is the implementation authority for the decisions below. Evidence paths describe the current tree; they are not target ownership.

| ADR | Decision | Primary owner |
|---|---|---|
| ADR-001 | Missing-tenant compatibility is restricted to epoch-bound signed user JWTs | P1.2 |
| ADR-002 | Request/access audit remains best effort and never invents a tenant | P1.2, P3.7 |
| ADR-003 | A failed command publish returns 502 and retains the recorded dispatch attempt | P3.4 |
| ADR-004 | Firmware upload/delete retains the current best-effort compensation order | P3.6 |
| ADR-005 | Rule snapshots reload every 5 seconds by default, with a bounded healthy-state contract | P3.3 |
| ADR-006 | Tenant zone lists use binary `name ASC, id ASC` order | P2.4 |
| ADR-007 | Roles use system-first binary order, atomic permission invalidation, and one timestamp update per successful mutation | P3.1 |
| ADR-008 | Users use exact-tenant atomic role/password semantics, durable authentication epochs, and snapshot-consistent security reads | P3.1 |
| ADR-015 | Operational metrics aggregation and time windows | R10 |

## ADR-001: Legacy missing-tenant mapping

- **Status:** Accepted
- **Owner work package:** P1.2; compatibility exception removed in P6

### Context and evidence

- `crates/backend/src/auth.rs` defines `Claims.tenant_id` as optional, validates signed JWTs, and issues new JWTs with a tenant claim and a 24-hour lifetime.
- `crates/backend/src/auth/context.rs` currently maps both a missing tenant and an invalid tenant string to `DEFAULT_TENANT_ID`.
- `crates/backend/src/rate_limit.rs` independently maps a token with no tenant to the string `"default"`.
- `crates/backend/src/domains/identity/auth_routes.rs` defaults a login request with no tenant selection to the default tenant. That is login routing behavior, not legacy claim repair.
- `crates/backend/src/middleware.rs` and `crates/backend/src/background.rs` also synthesize the default tenant outside credential compatibility. Those uses are not part of this exception.

Silently changing a malformed, explicitly supplied tenant into the default tenant crosses a security boundary. At the same time, rejecting a correctly signed token merely because it predates the tenant claim would break a bounded rolling-upgrade case.

### Decision

The host owns one compatibility mapper from validated user JWT claims to the non-optional core `TenantContext`. Authentication-epoch validation from ADR-008 happens before this tenant compatibility mapping:

1. A present, valid tenant claim maps to that tenant.
2. An absent tenant claim on an otherwise valid signed **user JWT** maps to `DEFAULT_TENANT_ID` only when that token also carries a non-empty `auth_epoch` that resolves to the persisted principal. A signed token with no `auth_epoch` is rejected; it does not enter this compatibility branch.
3. A present but empty or invalid tenant claim is rejected as unauthorized; it never falls back.
4. API keys, device identities, worker/system actors, repository calls, and arbitrary optional tenant inputs never use this fallback.
5. Login with an omitted tenant continues to authenticate against the default tenant for HTTP compatibility. The token issued by a successful login contains an explicit tenant claim.
6. Rate limiting must consume the same mapped identity where available. It must not implement a second tenant fallback. Before authentication mapping is available, it may fall back to an IP key, not a fabricated tenant identity.

The compatibility mapper records a counter and a structured warning without logging the token. Its result is a non-optional `TenantId` before any application use case is invoked.

### Consequences

- Epoch-bound signed user tokens that omit only the tenant remain usable during the bounded missing-tenant compatibility window.
- Tokens issued before `auth_epoch` was introduced are intentionally rejected once the security correction is deployed. This one-time logout is required to prevent a deleted principal's token from becoming valid for a replacement principal.
- Malformed tenant claims fail closed instead of gaining default-tenant access.
- Core identity types do not know about JWTs or `DEFAULT_TENANT_ID`.
- Default-tenant bootstrap and default login selection remain valid product behavior but are separate from runtime tenant fallback.

### Compatibility and rollout

- P1.2 introduces the mapper and routes authentication plus rate-limit identity through it.
- Keep the missing-tenant branch through mixed-version deployment and for at least one maximum token lifetime, currently 24 hours, after every token issuer writes tenant claims. This branch never relaxes the `auth_epoch` requirement.
- Deploy the schema backfill before or atomically with epoch-aware token issuance and validation. After validation is enabled, all tokens without `auth_epoch` fail closed and users authenticate again to receive a current token.
- Remove the branch in P6 only after no compatibility-hit metric has been observed for one full supported token lifetime. If external issuers still require it, retain it only through a separately approved, time-bounded compatibility policy.
- Removing default-tenant substitution from audit and workers is an intentional tenant-isolation correction, not an API compatibility break.

### Required tests

- A signed JWT with a valid tenant maps to that exact tenant.
- A signed epoch-bound JWT with no tenant maps to the default tenant and increments the compatibility metric.
- A correctly signed JWT with no `auth_epoch`, or with an empty or non-matching epoch, is unauthorized even when its tenant is absent.
- Empty and whitespace-only present claims, plus every form rejected by the final `TenantId` parser, are rejected.
- A missing/invalid tenant cannot query or mutate another tenant's records.
- API-key, device, and system actor construction requires an explicit tenant or an explicitly system-scoped API.
- Login without tenant still selects the default tenant and returns a token containing `tenant_id = "default"`.
- Rate-limit keys use the mapped tenant and user; invalid mapping uses the IP path rather than `default`.

## ADR-002: Audit durability and tenant ownership

- **Status:** Accepted with an explicit non-guarantee
- **Owner work packages:** P1.2 for tenant correction; P3.7 for persistence migration; P4.1 for middleware ownership

### Context and evidence

- `crates/backend/src/middleware.rs` runs the mutating request first, builds an audit event from the response, then attempts persistence.
- The same middleware logs an audit persistence failure and still returns the already produced response.
- For requests without `RequestContext`, it currently stores the event under `DEFAULT_TENANT_ID` with actor type `anonymous`.
- `crates/backend/src/domains/audit/audit_service.rs` and `crates/backend/src/domains/audit/repository.rs` expose separate `record` and `list` operations. Audit persistence does not participate in the business mutation transaction.

No evidence in the current implementation establishes compliance-grade or mutation-atomic audit. Claiming that guarantee during a crate move would be unsafe.

### Decision

Request/access audit remains a best-effort host concern during this refactor:

1. Audit persistence happens after the application result is known.
2. Audit persistence failure is logged and measured but never changes the HTTP status or rolls back a successful mutation.
3. A tenant-scoped audit row is written only when an authenticated, validated `TenantContext` exists.
4. An unauthenticated mutating request may produce a structured host access log, but it is not written into a tenant audit table and never uses `DEFAULT_TENANT_ID`.
5. Stored audit metadata remains limited to safe request facts; credentials, authorization headers, cookies, and sensitive bodies are excluded.
6. The core does not emit or promise mutation-atomic domain audit as part of this refactor.

Any event that must be durable or atomic with a business mutation requires a separate classification ADR and an explicit transactional operation or durable outbox. Until then, documentation and API names must call the current records request/access audit, not a compliance ledger.

### Consequences

- Audit storage outages do not turn completed mutations into HTTP failures.
- Anonymous requests no longer create misleading default-tenant audit rows.
- A process crash after commit and before audit persistence can lose an audit event; this is accepted current durability.
- The adapter contract covers storage semantics, while middleware timing remains a host integration concern.

### Compatibility and rollout

- P1.2 removes the anonymous/default-tenant database fallback and adds structured logging for such requests.
- P3.7 moves the same best-effort storage contract to both adapters without strengthening its durability.
- P4.1 keeps request metadata and response-outcome mapping in HTTP middleware.
- Existing authenticated audit list responses and ordering remain compatible.
- A future compliance requirement is materially unresolved; it must not be inferred from this ADR. The recommended follow-up is the architecture plan's transactional domain-audit work, outside the structural critical path.

### Required tests

- An authenticated successful mutation records a success event under the authenticated tenant.
- An authenticated rejected mutation records a failure event under the authenticated tenant where middleware placement permits it.
- An unauthenticated mutation attempt creates no tenant audit row and emits the structured access event.
- Audit repository failure leaves the original successful and failed HTTP responses unchanged.
- Audit listing is tenant isolated and has deterministic pagination/order in both adapters.
- Metadata redaction tests prove that bearer tokens, cookies, passwords, and request bodies are absent.

## ADR-003: Device-command publish failure

- **Status:** Accepted; cross-adapter correction approved
- **Owner work packages:** P3.4 for application/status contract; P4.2 for `DeviceBus` transport mapping

### Context and evidence

- `crates/backend/src/domains/commands/command_service.rs` validates and records a command before calling `zenoh::Session::put`.
- If the publish fails, that function returns `AppError::Zenoh` and does not delete or update the command row.
- `crates/backend/src/error.rs` maps that error to HTTP 502 with code `device_communication_error` and safe message `Device communication failed`.
- PostgreSQL creates the row through the table default `sent`; see `crates/backend-postgres/migrations/00000000000000_initial_schema/up.sql` and `crates/backend/src/persistence/postgres/commands.rs`.
- Turso explicitly creates `pending` and uses `completed`/`timeout`, while PostgreSQL uses `sent`/`delivered`/`succeeded`/`failed`/`timed_out`; see `crates/backend/src/persistence/turso/commands.rs` and `crates/backend/src/persistence/postgres/commands.rs`.
- `apps/frontend/src/components/devices/commands-tab.tsx` recognizes the PostgreSQL vocabulary, and PostgreSQL is the current default production profile.

The adapters therefore disagree on externally visible status semantics. Preserving both is incompatible with one shared port contract. Changing the default production behavior during an ownership refactor carries greater compatibility risk than normalizing the edge adapter to it.

### Decision

The canonical refactor behavior is:

1. Validate authorization, device existence, contract, route, and input before creating a command.
2. Persist one command dispatch attempt with status `sent`.
3. Publish through the host `DeviceBus`.
4. On accepted publication, return the existing success response: HTTP 201 for the command route and the recorded command body.
5. On synchronous publication failure, retain the row as `sent`, return HTTP 502 with code `device_communication_error`, and record the transport failure in logs/metrics without exposing its raw text to the client.
6. Do not automatically republish and do not delete the row. The existing timeout process may later transition it to `timed_out`.
7. Database failure before row creation prevents publication.

For P3.4, PostgreSQL's status vocabulary is canonical for both adapters: `sent`, `delivered`, `succeeded`, `failed`, and `timed_out`. Turso's `pending`, `completed`, and `timeout` command states are an explicitly approved compatibility correction. Existing persisted Turso values must remain readable and be mapped at the adapter boundary or migrated with a separately reviewed data migration; they must not become corrupt/unreadable rows.

`sent` means that the application recorded a dispatch attempt, not that the device received it. Only `delivered` or a terminal device response conveys stronger evidence.

### Consequences

- The HTTP and retained-row behavior matches the current default PostgreSQL deployment.
- A 502 can coexist with a visible `sent` history record. This is intentionally documented rather than hidden.
- There is no crash-safe retry between record and publication.
- A durable command-dispatch outbox remains a recommended post-refactor reliability project and would require a new ADR/API compatibility review.

### Compatibility and rollout

- P0-B captures the 502 body and current PostgreSQL row behavior.
- P3.4 introduces an application result that distinguishes persistence failure from bus failure while retaining the HTTP mapping.
- In a mixed-version edge rollout, tolerant reads for old Turso statuses ship before any optional data normalization.
- P4.2 replaces direct Zenoh use with `DeviceBus` without changing ordering or error mapping.

### Required tests

- Invalid authorization/input/device/contract creates no row and does not invoke the bus.
- Persistence failure does not invoke the bus.
- Successful publication creates exactly one `sent` row and returns 201.
- Injected bus failure creates exactly one retained `sent` row and returns the stable 502 body.
- Retrying the HTTP request creates a distinct dispatch attempt, matching current generated-correlation-ID behavior.
- The timeout transition uses `timed_out` in both adapters.
- Device acknowledgements and terminal responses have the same transitions in both adapters.
- Both adapters can read pre-normalization Turso command states during rollout.

### R07 implementation note: command states and persistence

Command persistence now resides in both adapters behind the core-owned port.
Core maps device responses; atomic updates accept only active statuses and retain
terminal rows. Turso maps historical pending/completed/timeout values to
sent/succeeded/timed_out without rewriting rows, and new writes use canonical
states. Historical pending rows remain eligible for response and timeout updates.
Timeout cutoffs use checked arithmetic and one clock sample. This closes the
status-vocabulary correction, but the host DeviceBus/application dispatch and
rule-action retry integration remain unfinished. Behavioral verification is deferred.

## ADR-004: Firmware object/metadata compensation

- **Status:** Accepted with an explicit crash-consistency limitation
- **Owner work package:** P3.6; host `BlobStore` implementation finalized in P4

### Context and evidence

- `crates/backend/src/domains/firmware/firmware_updates.rs` uploads the object before creating database metadata.
- If metadata creation fails, it attempts object deletion, logs cleanup failure, and returns the original metadata error.
- Delete removes database metadata first, then attempts object deletion. Object failure or a storage-backend mismatch is logged, while the API still returns 204.
- `crates/backend/src/domains/firmware/firmware_store.rs` generates keys as `tenants/{safe_tenant}/firmware/{uuid}/{safe_filename}`.

The database and object store cannot participate in one transaction. Reversing operation order merely changes which orphan is possible. A crash-safe guarantee requires persisted workflow state, which is outside a structural crate split.

### Decision

Move the coordination unchanged into a firmware application use case and preserve this order:

**Upload**

1. Validate, sanitize the filename, compute size and SHA-256, and allocate the existing key format.
2. Put the object.
3. Transactionally create firmware and blob metadata.
4. If metadata fails, make one best-effort delete of the just-written object.
5. Return the metadata error even if compensation also fails; log and count compensation failure with the storage key redacted or safely structured.

**Delete**

1. Transactionally delete metadata and return the former blob location.
2. If a configured compatible backend owns the object, make one best-effort object delete.
3. If object deletion fails or the backend is unavailable/mismatched, log and count the orphan condition but retain HTTP 204.

Compensation is idempotent: deleting an already absent object is treated as success by the `BlobStore` contract. The refactor does not add background orphan retry, tombstones, or a persisted upload workflow.

### Consequences

- Object keys, response codes, and current error precedence remain compatible.
- Upload can leave an unreferenced object if compensation or the process fails.
- Delete can leave an unreferenced object after metadata is gone.
- Object-store SDK types remain in the host; core sees only `BlobStore` and firmware application outcomes.

### Compatibility and rollout

- P0-B freezes key construction and representative HTTP results.
- P3.6 moves orchestration from the Axum handler without changing the order above.
- A fake/in-memory `BlobStore` provides deterministic fault injection.
- A requirement for zero orphaned objects is materially unresolved and cannot be promised by this design. It requires the architecture plan's separate schema-backed firmware workflow.

### Required tests

- Object put failure creates no metadata and returns the existing storage-unavailable response.
- Metadata failure after put invokes delete once and returns the metadata error.
- Metadata failure plus compensation failure returns the metadata error and records compensation failure.
- Metadata delete failure does not delete the object.
- Object delete failure after metadata deletion still returns 204 and records the orphan condition.
- Backend mismatch still returns 204 and records that cleanup could not run.
- Repeated compensation of an absent object succeeds.
- Keys retain the exact tenant/UUID/sanitized-filename layout and prevent traversal.
- Duplicate version/retry behavior is identical in both database adapters.

## ADR-005: Rule-snapshot refresh bound

R05 implementation update (2026-09-14): core now owns rule records, application
validation/authorization, and the rule port; adapters own queries. PI-11 requires
one snapshot for definitions, children, zones, and initial runtime hints:
PostgreSQL uses a read-only repeatable-read transaction and Turso a read-connection
transaction. HTTP list ordering keeps its historical engine difference; snapshot
rule/child ordering is explicit and stable. The host immutable store, polling,
fingerprint-before-compilation, core evaluation, post-commit invalidation, readiness,
and reload metrics are now implemented. Readers retain immutable handles; definition
reloads preserve current runtime hints pending R06's durable state ownership.
Behavioral evidence remains deferred under the current test policy.

- **Status:** Accepted for healthy dependencies; no partition-time guarantee
- **Owner work packages:** P3.3 for snapshot semantics; P4.3 for supervision/readiness

### Original context and evidence (before R05)

- `crates/backend/src/app/boot.rs` performs one full rule-cache load at process startup.
- `crates/backend/src/domains/rules/rules.rs` refreshes only the process that handled a successful HTTP mutation.
- `crates/backend/src/state.rs` exposes one process-local `Arc<RwLock<RuleCache>>`.
- `crates/backend/src/domains/rules/rule_engine/actions.rs` also mutates cooldown, active-alert, and zone-entry maps in that process-local cache.
- No current worker reloads rule definitions changed by another backend replica.

The current cross-replica staleness is unbounded. A schema-backed revision would provide stronger coordination but is not required to establish a bounded improvement during extraction.

### Decision

P3.3 introduces a host `RuleSnapshotStore` with immutable snapshots and single-flight full reloads:

1. Every process must load a complete snapshot before becoming ready.
2. A successful local rule mutation invalidates after database commit and schedules an immediate local reload. Reload failure does not change the already committed API result.
3. Every process polls independently using `RULE_SNAPSHOT_REFRESH_INTERVAL_SECS`.
4. The default interval is **5 seconds** for all deployment profiles. Configuration accepts **1 through 60 seconds**; zero or larger values fail startup validation.
5. A reload must not overlap another reload in the same process. A missed tick coalesces into one reload.
6. Under a healthy database, a reload that completes within one configured interval gives a maximum observation target of **two configured intervals** from commit. This is the tested healthy-state bound.
7. The last known-good immutable snapshot remains in use if reload fails. Readiness becomes degraded once its age exceeds two configured intervals, and metrics expose snapshot age, reload duration, success, and failure.
8. Mutable cooldown, active-alert, zone-entry, claim, and idempotency state is database-authoritative. Snapshot replacement cannot be the sole duplicate-prevention mechanism.
9. The initial revision may be a deterministic content hash or adapter-provided durable update fingerprint; no schema change is required. Equal revisions avoid recompilation where practical.

The 5-second default is an operational starting point, not a hard real-time or safety guarantee. During database unavailability or network partition, staleness is unbounded and explicitly surfaced by degraded readiness.

### Consequences

- Cross-process changes become visible within a measurable healthy-state target instead of never.
- Full reload every five seconds adds database and compilation load; P3.3 must measure it with a representative rule set.
- Evaluations continue with the last known-good rules during transient failures.
- If the load is unacceptable or a hard partition-time bound is required, the durable revision/invalidation follow-up must be approved before relaxing the interval.

### Compatibility and rollout

- Add the configuration with the default above; deployments need no new environment setting.
- Start the polling store before moving rule evaluation to it, then switch all evaluators in P3.3.
- Do not delete direct cache access until all mutation/evaluation paths use the snapshot port and database-authoritative operations.
- There is no documented product safety SLO for rule propagation. Deployments must not treat this ADR as a guarantee for safety-critical actuation; such a requirement needs explicit product approval and a stronger durable design.

### Required tests

- Startup fails readiness if the initial load fails.
- Two runtime instances sharing each adapter observe a mutation within two configured intervals.
- Local invalidation schedules an immediate reload without changing a successful mutation response when reload fails.
- Paused-time tests prove tick coalescing and no overlapping reloads.
- A failed reload retains the prior snapshot, increments failure metrics, and degrades readiness after the age threshold.
- Recovery replaces the snapshot and restores readiness.
- Equal revisions do not alter behavior.
- Concurrent evaluation cannot create duplicate externally visible actions; durable claim/uniqueness tests provide the correctness proof.

## ADR-006: Canonical zone ordering and name uniqueness

- **Status:** Accepted as an intentional compatibility correction
- **Owner work package:** P2.4

### Context and evidence

- Before P2.4, `crates/backend/src/domains/zones/zone_repo.rs` ordered PostgreSQL zone lists by `created_at DESC` and had no tie breaker.
- Before P2.4, the host Turso implementation ordered tenant lists by `name, id` and cross-tenant lists by `tenant_id, name, id`.
- The pre-extraction host port exposed both tenant CRUD and cross-tenant `list_all`; the core now separates `ZoneRepository` from `RuleZoneSnapshotRepository`.
- The pre-extraction Turso baseline, now at `crates/backend-turso/migrations/0001_baseline.sql`, enforces `UNIQUE (tenant_id, name)`, while the legacy PostgreSQL schema did not enforce equivalent zone-name uniqueness.
- Zone lists are not paginated, so adapter-independent deterministic collation can be applied without cursor implications.

The current behavior cannot satisfy one shared contract. The architecture plan already approves `name ASC, id ASC` for the P2.4 walking skeleton.

### Decision

The tenant-scoped `ZoneRepository::list` contract is:

1. Filter by the exact `TenantId` before ordering.
2. Sort by zone `name` ascending, then `id` ascending.
3. Comparisons are case-sensitive binary/Unicode-scalar order with no locale folding and no Unicode normalization.
4. PostgreSQL uses an explicit deterministic binary collation and Turso/SQLite uses its binary collation, or the adapter applies the equivalent Rust ordering before return.
5. `id` is always the stable tie breaker.
6. Zone names are unique per exact `(tenant_id, name)` pair using the same binary collation. A duplicate create/update returns the stable `UniqueViolation` constraint `zones.tenant_name`, which the host maps to the existing conflict response.

Cross-tenant `list_all` is removed from tenant CRUD. The system rule-snapshot loader returns zones in `tenant_id ASC, name ASC, id ASC` using the same comparison rules.

### Consequences

- Both adapters return byte-for-byte comparable order for valid UTF-8 values.
- PostgreSQL clients see an intentional change from newest-first to name-first.
- The result is stable when timestamps collide and is suitable for the first shared contract suite.
- No public DTO changes are required.
- PostgreSQL gains a binary-collated unique index to match the already shipped Turso invariant. This is an intentional compatibility correction: PostgreSQL clients can no longer create two zones with the same exact tenant/name pair.

### Compatibility and rollout

- P0-B characterizes both current adapter results.
- P2.4 changes both adapters together, updates the API fixture with an approved migration note, and deletes the old PostgreSQL zone repository path.
- PostgreSQL migration `20260831000000_zone_name_uniqueness` performs a duplicate-data preflight before creating the index. It fails with an actionable rename/merge instruction and never deletes or rewrites existing rows.
- Operators can run the migration's grouped preflight query ahead of rollout. A failed preflight is a deployment blocker requiring an explicit data cleanup; the application must not choose a duplicate automatically.
- The ordering change must be called out in release notes because array order is externally observable.

### Required tests

- Tenant isolation with interleaved zones from two tenants.
- Adapter contracts cover ASCII case ordering, non-ASCII values, and the shared duplicate-name conflict.
- Repeated reads return the same order independent of insertion timestamps.
- PostgreSQL and Turso return the same ordered IDs for the same fixture.
- The system snapshot order includes `tenant_id` as its first key.
- A PostgreSQL preflight detects existing duplicate `(tenant_id, name)` values without changing data.
- CRUD not-found, in-use delete, and geometry error behavior remains unchanged.

## ADR-007: Canonical role and permission mutation semantics

- **Status:** Accepted as a cross-adapter security and determinism correction
- **Owner work package:** P3.1 roles/permissions

### Context and evidence

- The legacy PostgreSQL role list returned system roles first, while Turso ordered every role only by name and ID. Neither contract fixed collation.
- PostgreSQL advanced `roles.updated_at` for every successful custom-role update, including permission-only and empty patches. Turso advanced it only for name or description changes.
- PostgreSQL invalidated assigned users' signed authorization state by incrementing `users.permission_version` when role permissions changed. Turso replaced permissions without that invalidation.
- Database-generated duplicate identifiers differed by engine and were not a stable business contract.
- The HTTP permission-catalog array order is externally observable and was already consumed by clients.

These differences cannot remain behind one core port. The missing Turso invalidation is security-sensitive because an already issued token could otherwise retain changed authorization until another user mutation.

### Decision

The core `RoleRepository` and `RoleApplication` contract is:

1. Every operation requires the exact tenant. A role ID owned by another tenant behaves as not found and cannot affect that tenant's users or permissions.
2. `list` returns all system roles first, followed by custom roles. Within each group it sorts by binary `name ASC`, then `id ASC`. Each role's permission keys use binary ascending order.
3. Exact role names are unique per tenant. A duplicate create or rename maps to `UniqueViolation` with stable constraint name `roles.tenant_name`; generated database constraint names do not cross the adapter boundary. Role primary-key conflicts use `roles.id` where exposed.
4. Create, hydrate, permission insertion, and every multi-field update are atomic. A failed name or permission write rolls the entire mutation back.
5. Replacing permissions increments `permission_version` once for every assigned user in the same transaction. Metadata-only and empty patches do not invalidate permission versions.
6. Every successful update of a custom role sets `updated_at` once from the adapter/database clock, including metadata-only, permission-only, and empty patches. Timestamps round-trip at UTC microsecond precision. Missing and system-role outcomes do not change the timestamp.
7. Delete atomically distinguishes missing, system, in-use, and deleted. In-use includes the role name and current assignment count; a successful retry returns not found.
8. The single core `Permission` catalog is authoritative. The host policy module is only a compatibility façade, and `Permission::all()` preserves the existing `/api/v1/roles/permissions` array order exactly.

No migration file changes are required: both shipped schemas already enforce tenant-local role-name uniqueness and have the required permission-version column. Existing migration bytes and checksums remain unchanged.

### Consequences

- Turso now invalidates authorization state with the same transactional guarantee as PostgreSQL.
- Turso role list order and timestamp behavior intentionally change to the canonical contract.
- Callers receive deterministic output independent of database locale or insertion order.
- Empty update requests remain successful and observable through `updated_at`, matching PostgreSQL compatibility behavior.
- Role contracts live in their own harness and do not couple role fixtures or lifecycle hooks to the zones walking-skeleton harness.

### Compatibility and rollout

- HTTP paths, methods, DTO fields, status codes, public messages, and OpenAPI schemas remain unchanged.
- The permission endpoint's exact key order is locked by a core golden test and host compatibility coverage.
- Both adapters are composed from the same physical engine handle already owned by the host: a clone of the PostgreSQL pool or an owner-retaining `TursoConnectionHandles` clone.
- The old host role service, role port/types, PostgreSQL/Turso implementations, and direct handler repository access are deleted in the same slice.

### Required tests

- Tenant isolation and wrong-tenant update/delete outcomes.
- System-first binary role ordering and binary permission ordering. Verify that both queries retain `id ASC` as a defensive final key; tenant/name uniqueness means a same-group name-tie fixture is not constructible.
- Stable duplicate constraint mapping for create and rename, including concurrent duplicate creates.
- Atomic rollback of multi-field updates.
- Assigned-user permission-version increments for permission-only and concurrent permission updates; no increment for metadata-only or empty updates.
- `updated_at` changes once for every successful metadata, permission-only, and empty update and retains microsecond precision.
- System-role and in-use deletion outcomes and retry behavior.
- Exact permission-catalog order and unchanged HTTP/OpenAPI compatibility.

## ADR-008: Canonical user, credential, and password semantics

- **Status:** Accepted as the P3.1 users/passwords implementation authority
- **Owner work package:** P3.1 users/passwords

### Context and evidence

- The legacy user repository combined general user projections, credential material, role assignment, password changes, login bookkeeping, and the last-owner invariant behind one host-owned port.
- Both adapters hydrated users through multiple statements, but their ordering and text behavior were not expressed by a shared contract. In particular, PostgreSQL rejects embedded NUL in `text` while Turso can accept it; this remaining input-parity question is tracked as PI-16 rather than hidden by the new port.
- A numeric user ID plus `permission_version` is not a durable principal identity. SQLite may reuse the deleted maximum row ID, and a replacement user can naturally reach the same initial permission version. An unexpired token bound only to tenant, numeric ID, and version could therefore authenticate as the replacement principal.
- Credential lookup and session hydration are security-sensitive compound reads. Reading the verifier, user, roles, and permissions across unrelated snapshots can combine revisions during a concurrent delete/recreate, password change, or role change, even when ordinary paginated list projections are allowed to be relaxed.
- Password validation and Argon2 execution were host service functions. Moving orchestration to core required a stable password policy without moving concrete crypto, randomness, Tokio blocking work, or environment access into core.
- Password changes and role replacement invalidate signed authorization state through `permission_version`. Last-owner deletion or demotion is a tenant security invariant that must remain correct under concurrent writers on both engines.

### Decision

The core `UserApplication` and `UserRepository` contract is:

1. Every repository operation requires the exact tenant. A user or role owned by another tenant behaves as missing and no rejected operation mutates either tenant.
2. User creation trims leading and trailing username whitespace once and rejects an empty result. Otherwise usernames remain exact and case-sensitive; authentication passes the supplied username through without trimming or case normalization. Exact usernames are unique per tenant and duplicate writes map through the stable `users.tenant_username` constraint. The same username may exist in another tenant.
3. User pages sort by binary `username ASC, id ASC`. Hydrated assigned roles sort by binary `name ASC, id ASC` without the system-first grouping used by role-management lists. Effective permission keys are distinct and binary ascending. The compatibility primary-role projection chooses `owner`, then `admin`, then the first binary-sorted assigned role.
4. `role_ids: None` selects the exact tenant's built-in `viewer` role. An explicit empty role list is invalid at the application boundary. Explicit IDs are sorted and deduplicated before persistence; a missing or cross-tenant ID returns the typed roles-not-found outcome without a partial user or assignment write.
5. User creation, selected-role validation, assignments, the compatibility primary role, a fresh durable random `auth_epoch`, and the initial permission-version update are one transaction. The epoch is non-empty, unique, opaque outside identity/session internals, and never derived from a numeric ID, username, or permission version. Existing rows receive distinct random epochs through adapter-owned migrations. A successful create is returned fully hydrated and has `permission_version == 2`, preserving the existing insert-then-assignment behavior.
6. `set_roles` is a complete atomic replacement. Missing users, missing roles, and last-owner rejection have separate typed outcomes and roll back every assignment, primary-role, and version change. Every successful call increments `permission_version` exactly once, including an identical-value retry.
7. Only the `owner` role has a last-member guard; a sole `admin` may be demoted. All owner assignments count, including inactive users. Delete and role replacement serialize around the tenant's owner state so concurrent delete/delete, demote/demote, or delete/demote operations cannot remove every owner. Rejected operations do not increment versions. A successful delete retry returns not found.
8. Every successful password write replaces the encoded verifier and increments `permission_version` exactly once, including a retry with the same verifier. Missing and cross-tenant users return the typed not-found outcome and do not mutate state.
9. Successful-login bookkeeping stores the caller-supplied UTC instant at microsecond precision, is last-write-wins, and never changes `permission_version`. After credentials have verified, a concurrent delete that makes this best-effort bookkeeping update return not found does not retroactively fail authentication.
10. Ordinary paginated user lists may retain relaxed multi-statement semantics: total, page membership, and hydrated projections can span revisions under a concurrent writer. Security-sensitive `find_credentials_by_username` and `get_details` must instead hydrate the user, epoch, verifier where applicable, roles, and permissions from one database snapshot. PostgreSQL uses a read-only repeatable-read transaction and Turso uses one read transaction; neither method may return a mixed principal/revision.
11. Every newly issued signed user JWT carries the persisted `auth_epoch`. Session resolution requires an exact match of tenant, user ID, `permission_version`, and `auth_epoch`, plus an active persisted user. A missing, empty, or mismatched epoch is unauthorized. This intentionally invalidates all legacy JWTs that predate the epoch claim once; their holders must authenticate again. The epoch remains absent from public user response DTOs and OpenAPI schemas.
12. Core password validation uses UTF-8 byte length of at least 12 and requires at least one ASCII lowercase character, ASCII uppercase character, ASCII digit, and character outside the ASCII alphanumeric set. Consequently ASCII punctuation, whitespace, and non-ASCII characters satisfy the symbol category. Existing validation order and public messages remain stable.
13. Core receives a `PasswordHasher` and UTC `Clock` as outbound ports. The host implementation uses Argon2's existing default parameters, a fresh `OsRng` salt, PHC encoding, and blocking-task isolation. Plaintext request DTOs deliberately lack `Debug`, `Clone`, and serialization. Plaintext transferred to the host `PasswordHasher` is wrapped in `Zeroizing` inside the blocking hash/verify task and is zeroized when that task ends; plaintext rejected before that boundary is not guaranteed to be zeroized. Encoded hashes have redacted `Debug`, no serialization, and appear only at credential-specific read and password-write boundaries.

The contract does not select behavior for embedded NUL in usernames. PI-16 must decide whether core rejects it with a stable invalid-input error or both storage adapters are made to accept the same representation.

### Consequences

- HTTP routes, public DTO fields, status codes, cookie behavior, validation messages, and OpenAPI schemas remain unchanged while orchestration moves through `Application`. Adding the private `auth_epoch` JWT claim and rejecting pre-epoch tokens are the explicit security correction.
- General user reads cannot expose an encoded password hash; authentication receives it only through `UserCredentials`.
- PostgreSQL row locks and Turso's serialized writer implement the same owner invariant and atomic aggregate outcomes without a generic cross-engine transaction abstraction.
- Exact login lookup intentionally differs from create-time trimming. A client that creates `" alice "` persists `"alice"` and must authenticate as `"alice"`.
- Same-role and same-password retries intentionally invalidate previously issued authorization once per successful call; they are not version-idempotent.
- Deleting and recreating a user cannot revive a prior session even if the storage engine reuses the numeric ID and the replacement reaches the same permission version.
- Relaxed multi-statement consistency is limited to ordinary list projections. Authentication and session resolution require one security snapshot; PI-16 remains an explicit input-parity limitation.

### Implemented evidence and remaining proof

- Core unit tests cover permission-before-validation ordering, create-time trim, empty-role rejection, role-ID normalization, the byte/ASCII password policy, stable error mappings, exact untrimmed login lookup, inactive/wrong/malformed credential rejection, injected clock use, and secret-safe encoded-hash debugging.
- The shared user adapter contract covers exact tenant isolation, cross-tenant same-name users, binary ordering and pagination, stable repeated lists, default viewer and primary-role priority, distinct permission ordering, atomic missing-role rollback, stable duplicate mapping, repeated password and same-role version increments, two successful login writes whose second caller-supplied timestamp wins without a version bump, an inactive final owner still blocking deletion, delete retry, concurrent role/user invalidation, concurrent create/role-delete, and all combinations of final-owner deletion/demotion.
- Host tests verify a pre-existing PHC Argon2 verifier, malformed-verifier failure, password round trips, wrong-password failure, and distinct random salts for identical plaintext.
- Security-correction closure additionally requires migration tests that backfill distinct non-empty epochs and enforce them for new rows; shared adapter tests that prove fresh epochs on create and delete/recreate (including Turso numeric-ID reuse); core session tests for epoch mismatch; and a host login/JWT/middleware test that proves issuance, legacy missing-epoch rejection, version invalidation, and same-ID/same-version replacement non-revival. These are required proof, not verification claims in this record.
- Credential and session adapter tests or equivalent concurrency evidence must demonstrate that a security read cannot combine a verifier or authorization projection from different principal revisions. Ordinary list snapshot consistency is deliberately outside this closure gate.
- Embedded-NUL parity is intentionally untested until PI-16 selects a portable contract.

## ADR-009: Preserve the combined CI-ingest persistence operation during extraction

- **Status:** Accepted for the 2026-09-14 ownership extraction; behavioral test execution deferred by user instruction
- **Owner work package:** P3.1 API-key authentication / P3.6 firmware ingest

### Decision

Core owns `CiIngestApplication`, the UTC input types, outcomes, device-type scope
policy, and `CiIngestRepository`. Each adapter implements the combined operation:
resolve a stored key hash, touch usage, find a device type within the key's tenant,
apply core scope policy, and insert firmware metadata. The caller cannot supply
or override that tenant. Hashing, request validation, rate limiting, and HTTP
translation remain host responsibilities.

Keeping this operation combined preserves Turso's existing transaction. It also
avoids introducing a gap between authorization and insertion by independently
calling key and firmware repositories from the application.

The following existing differences are intentionally retained:

- PostgreSQL performs key lookup and a best-effort `last_used_at` update outside
  the firmware insert/read transaction. Missing device types, scope rejection,
  and insert failures may leave the usage touch persisted. A touch error is ignored.
- Turso holds the shared serialized writer and uses one transaction. Unknown keys
  roll back without mutation. Missing device types and scope rejection commit the
  usage touch. Insertion errors roll back the transaction, including that touch.
- Concurrent key deletion retains each engine's existing isolation behavior.
  This extraction does not promise identical revocation timing across engines.

Both implementations call the same pure core scope policy after tenant-local
device-type lookup. Existing outcome precedence and conflict/not-found/forbidden
messages remain unchanged. Core timestamps use `DateTime<Utc>`; PostgreSQL converts
to naive UTC at its row boundary, and Turso stores microseconds as before.

### Consequences and remaining verification

The legacy host CI service, PostgreSQL API-key helpers, API-key compatibility
re-exports, and firmware-port ingest method are removed. HTTP calls the core
application and its direct repository exception is deleted. No migration or
public API schema change is required.

Shared ingest contracts and failure/concurrency execution remain outstanding
because tests were skipped. Before declaring semantic parity, choose a separate
transaction/revocation contract and prove it on both engines. Existing API-key
name uniqueness differences and planned nonce/rotation scope remain separate.

## ADR-010: Certificate ownership, one-time rotation, and system identity scope

- **Status:** Implemented in the R01 continuation; behavioral verification deferred by instruction.
- **Owner work package:** R01 / P3.1 certificates and key protection.

Core owns certificate records, the business repository port, tenant certificate
operations, provisioning material preparation, and a separate system façade for
CA setup, encrypted-key maintenance, server material, and active certificate IDs.
The host supplies `CertificateIssuer` and `KeyProtector`; crypto configuration is
resolved at composition. Ring/rcgen, PEM/fingerprinting, randomness, blocking-task
execution, and the unchanged `enc:v1:` envelope remain host outbound concerns.
Key-bearing core records redact private material from `Debug`.

PI-06 selects the existing PostgreSQL one-time rotation behavior for both engines:
regeneration returns the new plaintext key in its response, and replacement clears
the persisted key before committing. Turso now also clears it transactionally.
A later download does not return that regenerated key again. Initial provisioning
continues to store material until the first successful compare-and-set consumption.
This is an intentional Turso behavior correction without a schema change; rollback
to an older binary cannot restore a key already consumed. Reissue a certificate if
its one-time response is lost, matching the existing PostgreSQL flow.

PI-14 retains the global device-ID invariant enforced by `devices.id` primary keys
in both shipped schemas. CN-only system ACL enumeration is explicitly global.
Tenant certificate reads/mutations still require exact tenant and device identity;
this decision does not authorize default-tenant substitution or cross-tenant reads.
Changing to tenant-local duplicate device IDs would require a separate schema,
certificate identity, and device transport migration.

Host certificate services/helpers and adapter implementations are removed after
all production callers migrate. Tests bound to those removed interfaces are removed
rather than repaired; pure encryption-envelope tests remain with the outbound
implementation. Compilation and architecture checks are the current evidence;
concurrent consumption/rotation and stored-data execution remain deferred.

## ADR-011: System bootstrap ownership and owner-role convergence

- **Status:** Implemented in the R02 continuation; behavioral verification deferred by instruction.
- **Owner work package:** R02 / P3.1 bootstrap.

Core `BootstrapApplication` owns built-in seed definitions, owner validation, local
admin/admin compatibility, injected password hashing, and application configuration
selection. The host retains environment reads, generated JWT secret candidates,
logging, and the explicit default-tenant bootstrap selector. Database lifecycle
remains on the host `DatabaseRuntime`; core bootstrap contains no migration,
backup, checkpoint, or health capability.

PI-15 retains global emptiness: owner creation is skipped whenever any user exists
in any tenant. Both adapters check this inside their serialized owner transaction.
Before creating the owner, both insert the tenant's owner role if missing and
ensure its full permission catalog, then create one user/role assignment with a
fresh random authentication epoch and permission version 2. PostgreSQL now handles
the missing-role precondition as Turso already did; no migration is needed.
Built-in device-type seeds remain insert-only, with no update to an existing name.
JWT secret candidates converge through the existing unique-key insert/read path.

Only the explicit local bootstrap operation accepts the historic admin/admin pair;
normal startup keeps the existing username/password checks. Password hashes use
core's redacted encoded-hash type. Existing public CLI/setup functions remain thin
host wrappers. No runtime/CLI caller uses the bootstrap repository directly.
Concurrent startup and migration/fixture execution remain deferred. Invalid tests
that directly used removed host bootstrap implementations are removed as specified
by the current execution policy.

## ADR-012: Catalog ownership and deterministic name ordering

- **Status:** Implemented in R03; behavioral verification deferred by instruction.
- **Owner work package:** R03 / P3.2 device types and fleets.

Core owns catalog values, tenant authorization, application operations, and the
six-operation device-type and four-operation fleet business ports. Both adapter
crates own queries and row conversion. HTTP routes and compatibility device-type
resolution in device/firmware creation invoke the core application.

PI-04's catalog ordering is binary name ascending, then numeric ID ascending:
PostgreSQL explicitly uses C collation and Turso explicitly uses BINARY collation.
This intentionally removes PostgreSQL locale-dependent sorting. Lists retain
separate count/page reads and existing pagination; they do not gain snapshot
consistency. Fleet device counts always join/group inside the exact tenant.

Existing application behavior is retained: device-type names/icons are trimmed,
color is normalized to uppercase, omitted icon/color get existing defaults, and
an empty update returns the current record. The historic device-type ID 1 deletion
restriction and compatibility default-name lookup remain unchanged. Fleet create
preserves supplied nonblank whitespace, while rename trims it. Existing uniqueness,
in-use/foreign-key deletion behavior, and error precedence remain as before.
Core `InvalidOperation` represents operation-precondition rejection; the host maps
it to the existing 422 `unprocessable_entity` response, while `InvalidInput` retains
400 `bad_request`. No route/DTO or schema changes are required.

The host catalog services, ports/types, and adapter implementations are removed.
One PostgreSQL `find_device_type_by_id` compatibility helper remains exclusively
for the legacy firmware transaction, with R09 as its deletion owner. Its unused
CRUD/list/name helpers have been removed and no new callers are permitted.

Invalid service tests and the remaining legacy Turso cases that constructed the
removed catalog implementations are deleted under the current policy. Public API
and other unaffected tests remain; execution and shared catalog contracts are deferred.

## ADR-013: Blueprint publication retries and atomic device provisioning

- **Status:** Implemented in R04; behavioral verification deferred by instruction.
- **Owner work package:** R04 / P3.2; decisions PI-12 and PI-13.

Core owns blueprint values, validation, publication compatibility, contract compilation,
and device catalog/provisioning policy. Each database crate owns its queries and
transaction boundaries. `DeviceApplication::provision` is the only application
entry point that creates a device; the HTTP handler translates inputs and computes
the host endpoint. CLI creation already uses that HTTP endpoint. Core uses its clock
for application timestamps and the existing UUID facility for new identities.

PI-13 publication is conditional on both the draft timestamp and exact JSON document.
Inside the transaction, compare the current latest revision against the revision
used to calculate compatibility. If that baseline changed, return a publication
conflict. An unchanged draft matching the latest revision's document and canonical
hash returns that existing immutable revision, including its original compatibility
metadata and creation timestamp. This makes consecutive identical publishes and
lost-response retries stable, without a new idempotency key or schema migration.
Reverting to an older document after an intervening different revision creates a
new revision; retries after a draft edit publish the current draft, not an old
request body. PostgreSQL locks the blueprint parent before the draft, matching
replacement's lock order. Turso uses its shared writer transaction; database busy
errors retain the adapter's existing retry/error handling. Revision overflow is
rejected rather than narrowing an out-of-range integer.

Provisioning prepares the compiled contract and certificate before opening a
transaction. The adapters commit the device, empty initial shadow, immutable
contract, pending assignment, optional prepared certificate, and initial
configuration together. When the blueprint declares configuration, core passes the
compiled desired configuration (defaults plus request overrides) for insertion into
`device_configs`; without declared configuration no configuration row is created.
This intentionally materializes configuration that was previously present only in
the contract. Tenant-qualified foreign keys enforce blueprint/type/fleet/device
relationships. A compiled contract is mandatory in the creation port.

Preparation failure creates no device rows. Transaction failure rolls back the
aggregate; generated key/contract values are discarded. Current certificate
preparation is local and has no remote resource to clean up. Duplicate name/identity
creation is a conflict, never an upsert or implicit key rotation. After a lost
creation response, the caller must look up the existing device; retrying the same
name does not silently create or overwrite it. This does not add an API idempotency
key or distributed transaction.

PI-12 target selection removes repeated IDs, preserving the first occurrence,
before applying the selection-size limit. Bulk assignment counts unique matching
tenant-owned rows, including devices already assigned to the requested fleet;
missing or foreign-tenant IDs count zero. Bulk deletion counts rows actually deleted,
so a completed deletion retried later returns zero. Turso also deduplicates at its
port boundary, matching PostgreSQL's set-based statements.

Catalog and blueprint host services/types/SQL are removed. The explicit host
`DeviceIngressRepository` retains protocol identity resolution, heartbeat/offline
transactions, and rule-action coupling until R08. Command callers use the core
catalog port until R07. No new database handles or migration bridges are introduced.
No stored contract/wire shape changes or schema migrations are needed. Existing
binaries can read the resulting rows; rolling back code also restores the old
non-idempotent publication behavior. Concurrency, failure injection, configuration
compatibility, and database execution evidence remain deferred to R14.

## ADR-014: Durable outbox claim ownership and alert transaction boundaries

- **Status:** R06 implemented, including database-authoritative runtime state and worker/maintenance ownership. Behavioral verification deferred by instruction.
- **Owner work package:** R06 / P3.3; PI-02, alert portion of PI-03/PI-04, and PI-10.

Core owns alert records, transition preconditions, tenant-facing authorization,
action-to-outbox metadata/idempotency mapping, persisted action encoding, replay
validation, and retry classification. PostgreSQL and Turso own queries and
transactions, using the existing shared engine handles. The host retains concrete
HTTP/Zenoh delivery clients, scheduling, and cancellation.

### Claims and retries

Claim ordering ranks eligible rows within each tenant by `available_at`,
`created_at`, and `id`, then selects and returns rows by tenant rank followed by
those same keys. PostgreSQL skips rows locked by other claimers and rechecks
eligibility on the locked base row; Turso claims within its writer transaction.
This is deterministic batch selection, not a guarantee of delivery completion
order across concurrent tasks or fairness under every contention pattern.

Each batch receives a fresh opaque UUID-bearing token in the existing `locked_by`
column. The worker label alone is never the ownership credential. An event ID
plus token identifies its claim even after a same-worker reclaim or dead-letter
replay that resets attempts. Success requires processing status and token;
failure additionally requires the claimed attempt. A rejected conditional update
returns false and cannot overwrite a newer claim. Expired final attempts become
dead letters. Lease comparisons retain microsecond precision on both engines.

Core preserves the existing exponential retry delay (2 through 256 seconds).
Invalid action envelopes, unsupported versions, and tenant/event-type metadata
mismatches are permanent failures; delivery failures remain retryable up to the
persisted attempt limit. Claim ownership does not make external side effects
exactly-once; database-authoritative duplicate suppression is still unfinished.

### Persisted action compatibility and rollout

New payloads use `{ "version": 1, "action": <existing externally tagged enum> }`.
The decoder also accepts existing raw enum payloads. Existing enum field names,
action metadata, and idempotency hashes remain unchanged. No schema migration is
needed for the envelope or claim token.

Old worker binaries cannot read version 1. Deployment must quiesce old producers
and consumers before starting this version, or first introduce a separate
reader-only compatibility release before enabling version-1 producers. Do not
roll back to a raw-only reader while versioned rows remain eligible for delivery
or replay; retain a compatible reader or explicitly convert those rows first.
Dead-letter API payloads expose the stored JSON, so clients must tolerate the
new envelope. These are deployment requirements, not completed rollout evidence.

### Alert transitions, intervals, and retention

PostgreSQL locks the tenant-scoped alert row before checking the core transition
precondition; Turso uses its writer transaction. Resolve persists its cooldown in
the same transaction; reactivate clears the associated cooldown there. Handlers
no longer spawn best-effort persistence after the response. Bulk transitions
normalize IDs and lock them in sorted order to avoid inconsistent lock ordering.

Alert list intervals are `[since, before)`, with `created_at DESC, id DESC`
ordering on both engines. This deliberately makes PostgreSQL's previously strict
`since` comparison inclusive. Summaries explicitly sort by status and severity.
Retention is system-scoped: delete only resolved alerts with `resolved_at < cutoff`
across every tenant, leaving null resolution timestamps untouched. The unused
legacy helper that deleted by creation time was removed. Runtime boundary and
concurrency evidence remains deferred.

Local alert/cooldown/zone-entry maps still exist as transitional runtime state.
Authoritative rule evaluation/state reservation and
transactional coupling of that state with action intent must finish before R06
can be marked implemented. This ADR does not claim that those maps are already
safe for multi-process duplicate prevention.

### R06 follow-up: durable alert creation receipts

`AlertWorkerApplication` now constructs a rule alert from the durable action ID.
The repository atomically returns an existing receipt, reuses an active or
acknowledged tenant/rule/device alert, or inserts an alert and records delivery.
PostgreSQL serializes creators on the tenant-scoped device row; Turso acquires a
database write transaction before checking active state. Both choose the newest
active alert with an ID tie-breaker if historical duplicates already exist.
The worker no longer reserves or suppresses creation through a process-local map.
Local updates after creation are hints, and a poisoned hint lock cannot block the
persisted creation decision.

A new `rule_alert_deliveries` table records the resulting alert ID for each action,
including actions that reused another alert. Its receipt is committed in the same
transaction as creation/reuse. Retries remain no-ops after resolution, retention,
or device deletion: a missing retained alert returns a successful empty result.
Receipts cascade only when their owning outbox row is deleted; alert IDs have no
foreign key so alert retention cannot erase delivery history. This addresses
creation retries. Manual reactivation now shares the device serialization boundary
(described below); other rule decisions still need the authoritative runtime-state
design before R06 is complete. Older workers
must be quiesced: they do not participate in receipt-based creation.

PostgreSQL migration `20260914010000_rule_alert_deliveries` and Turso migration
`0010_rule_alert_deliveries.sql` add the table without rewriting historical rows.
Existing pending actions use the normal active-alert lookup on first delivery;
there is no inferred backfill of historical completion receipts. New Turso logical
archives include receipts after outbox rows. Its existing exact-schema archive
policy rejects older table sets; restore old archives with their compatible
binary first, then upgrade the restored database. PostgreSQL's down migration
removes receipt history, so do not downgrade/replay previously applied actions
under an assumption that receipt-based deduplication survives that downgrade.
Migrations, backups, concurrency, and retry behavior have not been executed in
this phase because tests remain deferred.

### R06 follow-up: reactivation shares the active-alert boundary

Manual reactivation cannot create a second active/acknowledged alert for the
same tenant, rule, and device. Core identifies transitions requiring an active
slot and returns an explicit competing-alert outcome. A single reactivation maps
that outcome to HTTP 409, naming the competing alert. Bulk reactivation preserves
the existing count response and skips conflicts just as it skips missing alerts
or invalid statuses; sorted alert IDs determine which requested alert gets an
empty slot. Rule-less alerts have no rule slot and retain their previous behavior.
Existing historical duplicates are not deleted or resolved automatically.

PostgreSQL transitions lock the device before the alert, matching creation's
serialization boundary, then reload the alert and inspect competing active rows.
Bulk operations lock all affected device rows in ID order before locking any
alert rows, avoiding opposite parent-lock ordering between requests. Turso obtains
its database write transaction before the precondition and competing-alert check.
A conflict commits no alert/cooldown mutation for that target. Successful
reactivation still clears cooldown state in its transaction. This closes the
manual-reactivation race with new alert creation; evaluator cooldown/zone-entry
state and atomic action-intent decisions remain unfinished.

### R06 follow-up: explicit evaluation time

Core passes one host observation timestamp into telemetry/status/geofence
calculations. All cooldown comparisons, action timestamps, and zone-entry times
in that evaluation use that value. Telemetry and heartbeat use the same timestamp
for their ingress write; contract events use their received timestamp. Offline
sweeps use one timestamp across the candidate batch. The explicit-time engine
functions do not read the wall clock. Existing convenience entry points capture
it once and delegate, preserving their public signatures.

The purpose is to make a decision reproducible when adapters later evaluate under
a device/runtime-state transaction lock. Passing a timestamp alone does not make
the current local runtime hints authoritative, and does not close the pending
atomic state/action-intent work.

### R06 follow-up: status-rule decisions join the ingress transaction

Heartbeat and offline writes carry immutable definitions and prepared inputs.
After acquiring the device serialization boundary, adapters load current alert
and cooldown rows and current device targeting, then call core's status decision
function. That function uses a fresh runtime view plus the shared definition
index, never snapshot-local runtime hints. Its cooldown mutations and queued
external/alert delivery intents commit in the same transaction as ingress.
A queue insertion failure rolls back all three. Cooldown actions are no longer
queued by these status paths; delivery counts therefore count actual outbox rows.

Adapter transaction participants are temporarily exposed under `migration-bridge`
while ingress ownership remains in the host. Their caller must already hold the
device/write transaction and enqueue the returned deliveries before committing.
They neither create a connection nor commit independently. R08 folds them into
the extracted adapter ingress operation. The additional device-scoped reads add
transaction work; representative-load measurement remains deferred.

Cooldown upserts keep the maximum existing timestamp on both engines. PostgreSQL
legacy cooldown delivery also joins the sorted device-lock protocol; offline
batches sort by tenant/device to match it. Clearing a cooldown is still a separate
state transition: an old legacy action can recreate a removed row until action
retirement or a durable reset fence is implemented. Telemetry, contract events,
and geofence state have not yet switched to this transaction-owned evaluation.
R06 cannot be closed until those paths and legacy runtime actions are handled.

### R06 follow-up: all live ingestion uses transaction runtime state

The core request is now `DeviceRuleEvaluation`: status changes, telemetry with
optional geofence processing, and contract-event metrics use the same adapter
transaction participant. It combines shared rule definitions with database-loaded
alerts, cooldowns, and zone entries. Current device targeting is read inside the
transaction. Host snapshot APIs that evaluated telemetry/geofences against local
runtime hints have been removed.

Zone-entry updates now persist alongside cooldown changes and outbox deliveries,
inside the ingestion transaction. PostgreSQL event ingestion acquires its device
lock before inserts; Turso uses the existing database write transaction. Duplicate
contract-event inserts skip evaluation. A failed delivery enqueue rolls back the
input write and all runtime state changes. No new live evaluator produces queued
runtime-update actions; these actions remain readable for legacy queue handling.

PostgreSQL migration `20260914020000_rule_zone_entries` and Turso migration 11 add
`rule_zone_entries`, keyed by tenant/rule/device. Both enforce tenant-qualified
rule/device ownership with cascade deletion and index tenant/device reads.
PostgreSQL needs tenant-qualified unique parent indexes for these foreign keys;
global parent IDs were already unique. Turso logical archives include the table.
These are new unapplied migrations, and migration/rollback execution is deferred.

No historical entry-state backfill is inferred from telemetry: old entries were
process-local, so the table begins empty and the next valid observation establishes
entry state. This can produce an entry action at the upgrade boundary, just as the
old implementation could after losing its local map on restart. Thereafter state
survives restart. PostgreSQL downgrade discards this history; older logical archive
table sets require their schema-compatible restoration path before upgrade.

Legacy queued cooldown/zone-entry effects and unused local-map plumbing still
need closure. In particular, old cooldown delivery can recreate a cleared row;
zone-entry delivery still targets the obsolete local map. Do not mark R06 complete
until this legacy path is fenced or retired explicitly and the maps are removed.

### R06 follow-up: snapshot loading contains definitions only

`RuleSnapshotRecords` no longer contains active alerts or cooldowns. Both adapter
snapshot loaders read only rules, condition/action children, and zones in their
existing consistent transaction. Runtime rows are loaded per device by ingress
transactions. HTTP alert management and normal delivery no longer update local
alert/cooldown hints, and definition reload does not copy those maps.

The temporary mutable snapshot sink remains solely for legacy queued zone-entry
updates. Legacy cooldown delivery still writes the database. Their reset-safe
handling is the remaining prerequisite to removing the sink and closing R06;
this cleanup does not silently discard queued actions.

### R06 follow-up: cooldown resets survive deletion of the cooldown row

Reactivation now persists the maximum reset timestamp in `rule_cooldown_resets`
before deleting its cooldown, under the existing device/write transaction.
Legacy queued updates use a guarded upsert: a firing time at or before the reset
is an acknowledged no-op. A newer accepted update still cannot move an existing
cooldown backward. Current transaction decisions and resolve operations use a
separate helper path and remain authoritative even at the same microsecond as a
reset. Reset comparison applies to delayed legacy intents, not current decisions.

PostgreSQL migration `20260914030000_rule_cooldown_resets` and Turso migration 12
add tenant-qualified rule/device foreign keys and a device lookup index. Markers
outlive cooldown retention; cascade deletion follows rule/device lifetime, so
repeated resets do not add unbounded rows per key. Turso logical archives include
the marker table. Migrations and runtime/concurrency checks remain unexecuted.

There is no historical reset-time backfill because the old schema did not record
that event. Protection begins with recorded resets under this implementation.
Old workers must be quiesced during rollout: their unconditional upserts cannot
participate in the protocol. Downgrade discards reset history; previously applied
cooldown replay protection cannot be assumed after that downgrade. Legacy zone
updates and the final local-map sink remain a separate unfinished R06 item.

### R06 follow-up: ordered legacy zone state and live ownership

Legacy `UpdateZoneEntry` deliveries remain readable and now call a core system
operation backed by adapter transactions. Before live evaluation takes ownership,
updates are ordered by original outbox creation time and binary event ID, retained
across retry/replay. Older or identical deliveries are acknowledged no-ops. State
mutation and the ordering marker commit together under the device/write lock.

Every valid location observation marks each applicable geofence rule/device pair
as live-owned, including observations that produce no entry/exit. This is needed
because an outside observation against empty state must still supersede a delayed
legacy entry. Invalid/missing coordinates do not claim ownership. Once live-owned,
all legacy deliveries for that pair become no-ops regardless of timestamp; new
producers never emit queued zone-state actions. Old producers must be quiesced at
rollout. Markers cascade with rule/device deletion and survive zone exits.

PostgreSQL migration `20260914040000_rule_zone_handoffs` and Turso migration 13 add
the table, tenant-qualified parent references, device index, and logical archive
inclusion. Existing persisted zone entries seed live ownership. The old schema
cannot reveal previously exited locations, so no additional historical handoff
is inferred. Downgrade removes ordering/ownership history and requires compatible
queue/backup handling. Migration and concurrency execution remain deferred.

The host no longer has a mutable rule-runtime map API or an action-worker snapshot
dependency. The obsolete rule-update branch that cleared local alert hints is
removed: database alert state governs updated definitions in every process.
Snapshot reload contains only immutable definitions and freshness/version state.
Final R06 ownership and stale-definition/foreign-key review remain outstanding.

## Architecture exception and removal ledger

P0-C should encode only these bounded temporary exceptions. A source allowlist entry must reference the exception ID, exact path/pattern, owner work package, and removal condition. New violations are not covered merely because they resemble an existing row.

| ID | Temporary exception and current evidence | Allowed scope | Owner/removal work package | Removal condition |
|---|---|---|---|---|
| EX-001 | JWT/default-tenant mapping is duplicated in `auth/context.rs` and `rate_limit.rs` | Existing lines only | P1.2; final branch review P6 | One host compatibility mapper remains; duplicate/fail-open mappings are gone; legacy branch satisfies ADR-001 removal rule |
| EX-002 | Runtime default tenant use in `middleware.rs` and `background.rs` | Existing audit and alert-retention call sites only | P1.2 | Audit never invents a tenant; retention is explicitly system-scoped or enumerates tenants |
| EX-003 | Domain-shaped modules import Axum, JWT `Claims`, and transport `AppError` | Existing `crates/backend/src/domains/**` imports only | P1.1, then P4.1 | Domain/application modules use transport-independent types; Axum and `ApiError` remain in host HTTP modules |
| EX-004 | `AppState` publicly exposes repositories and concrete runtime dependencies in `state.rs` | Existing fields only; no new public fields | P1.4, completed P5.2 | Runtime substates have private fields and transports receive application façades, not repositories |
| EX-005 | `Persistence` is a broad public business/lifecycle aggregate in `persistence/mod.rs`; bootstrap mixes health/migration/bootstrap | Existing aggregate/methods only | P1.3, fully removed P3/P5.1 | `RepositorySet` contains business ports only; `DatabaseRuntime` owns lifecycle/maintenance; old aggregate is gone |
| EX-006 | PostgreSQL/Turso implementation modules, schema types, and migrations live in the host crate | Existing `db`, `persistence/postgres`, `persistence/turso`, and migration paths | P2.2 foundations; each P2.4/P3 slice; cleanup P6 | Foundations and every repository implementation live in the adapter crates; no adapter-to-host edge exists |
| EX-007 | Planned `migration-bridge` exposes adapter foundations to host-local legacy wrappers | Only exports marked `migration-bridge`; no second pool/database and no writes in two implementations | Introduced P2.2; removed P6 | Last legacy repository slice is deleted and bridge feature/exports no longer exist |
| EX-008 | HTTP/Zenoh/worker entry points coordinate multiple repositories directly | Existing handler/consumer/worker call sites only | Slice-by-slice P3; transport cleanup P4.1-P4.3 | All orchestration is reached through narrow application operations |
| EX-009 | Concrete Zenoh, Reqwest, firmware-store, and environment/key-protection work appears in domain-shaped code | Existing command, rule-action, firmware, and certificate paths only | P3.3, P3.4, P3.6, then P4.2-P4.4 | Core depends only on approved outbound ports; SDK/configuration code is host-owned |
| EX-010 | Process-local mutable rule cache is exposed and directly changed across handlers/workers | Existing `rule_cache` field and rule-engine cache call sites only | P3.3; supervision P4.3 | All evaluation uses `RuleSnapshots`; mutable correctness is database-authoritative; ADR-005 tests pass |
| EX-011 | `apps/extrittio/src/commands/service.rs` constructs adapters and imports Turso-specific lifecycle APIs | Existing service command paths only | P5.1 and P5.3 | App imports only host lifecycle/service façades and process-level libraries |
| EX-012 | App shell owns OpenThread service orchestration that belongs in the host | Existing direct app imports/calls only | P4.2 and P5.3 | OpenThread integration is host-owned and absent from app dependency edges |
| EX-013 | Host requires a database adapter at crate scope and owns a duplicate executable/default adapter feature behavior | Existing manifest, crate `compile_error!`, and binary target only | P5.3-P5.4; future graph checks activated P2.5 | No-adapter host/OpenAPI builds; app validates deployment profiles; one executable owner remains |
| EX-014 | Core-shaped code may currently pull Prost through broad `extrittio-common` features | Existing dependency closure only; no new generated-wire use | P1.5 and P2.3 | Core closure contains no Prost/generated transport types and uses lean shared features/types |
| EX-015 | Legacy Diesel repository helpers remain under domain/repository paths | Existing helpers only; no new callers | P1.5; removed with matching P2.4/P3 slice | Each helper moves behind PostgreSQL adapter ownership or is deleted with its migrated slice |

Approved target composition paths such as host `database`, `boot`, and `maintenance` modules are not exceptions: they are the intended concrete-adapter boundary. Default-tenant bootstrap in `init.rs` and explicit default-tenant login selection are also not blanket exceptions; the architecture verifier should allow their exact semantic locations, not the constant throughout the crate.

## Decision maintenance

- A work package that changes one of these decisions must update this file and its compatibility fixtures in the same pull request.
- Each pull request removes its owned exception rows from the architecture allowlist when the removal condition is met.
- P6 fails if any EX-002 through EX-015 temporary exception remains. EX-001 must be removed unless ADR-001's external-issuer policy is separately approved and the exact compatibility mapper is reclassified as target behavior; exact bootstrap/login allowlist locations may remain as target behavior.
- The unresolved stronger guarantees identified here—compliance audit, durable command dispatch, crash-safe firmware workflow, and partition-safe rule propagation—require separate approval because they change product/data semantics rather than crate ownership.


### R06 final ownership and stale-definition closure

Core worker applications now handle alert outcomes and legacy cooldown application;
core maintenance owns checked cutoffs and explicit global resolved-alert retention.
Adapter transaction participants exclude missing/disabled candidate rules before
evaluation, with PostgreSQL key-share locks protecting surviving identities and
Turso's write transaction protecting the same interval. This prevents deleted rule
snapshots from creating runtime foreign-key failures; it does not strengthen the
snapshot freshness guarantee for definition edits.

Both adapters now own outbox insertion SQL as well as claims and outcomes. Host
ingress delegates through transaction participants until R08 migrates the enclosing
operations. R06 implementation is complete; database, migration, concurrency, and
external-delivery behavior remain unverified under the current test policy.


### ADR-003 implementation follow-up: core dispatch connected

User command creation/history and restart routes now invoke core `CommandApplication`.
The host `ZenohDeviceBus` owns protocol encoding, topics, publication, and metrics.
The core communication error maps to the existing safe 502 response; a publication
failure retains the recorded sent row, and ordinary user sends do not retry.

Durable rule-action delivery is a separate core operation: it checks any existing
stable-ID row for exact tenant/device/command/parameter identity and reuses it.
A competing insert is accepted only after reading a matching row. Delivered or
terminal rows skip publication; sent rows may republish through outbox retries.
Concurrent deliveries are still at least once externally. New/retried sent rule
commands now share current assigned-contract validation with user sends; this can
reject actions that the old raw command producer published without validation.
No HTTP wire shape or stored history schema changed. Behavioral verification remains
deferred under the user's test policy.


## ADR-015: Operational metrics aggregation and time windows

**Status:** Implemented in R10; behavioral verification deferred.

Operational metrics remain server-wide diagnostics guarded by `ReadServerMetrics`.
No response fields, endpoints, collection intervals, tenant scope, or schema change.
This resolves PI-05 and the metrics portions of PI-03/PI-04.

| Value | Bucket operation |
|---|---|
| Network RX/TX deltas; request, error, and Zenoh counts | Sum |
| CPU/load and average latency | Arithmetic mean of stored samples |
| Memory/disk bytes and pool counts | Integer sum/count, truncating toward zero |
| Sampled p95 latency | Maximum stored p95 |

PostgreSQL's counter and p95 choices are retained; Turso's averaging is corrected.
Latency remains an unweighted mean of sample means. Maximum sampled p95 is not a
recomputed percentile across all requests. Integer gauges avoid floating conversion
of large byte counts. Supported intermediate integer sums must fit i64; final app
counts must fit i32, and byte values i64. Overflow fails instead of wrapping or
saturating, even if the mathematical average would fit. PostgreSQL now explicitly
casts aggregate outputs to the SQL types declared by Diesel. Finite floating means
return f32; deferred cross-engine acceptance should use absolute tolerance 1e-4 plus
relative tolerance 1e-5. Exact bit equality and non-finite/corrupt legacy data parity
are not promised; no sanitization or backfill is introduced.

History includes `recorded_at >= since`. Core rounds sub-microsecond bounds upward,
so truncation cannot include an earlier stored sample. Raw HTTP mode stays at ten
seconds, returning the first 10,000 rows per stream in timestamp/ID ascending order.
Latest uses both keys descending. Empty tables still return independent empty
arrays or absent latest records.

Downsampling filters before aggregating, then groups by UTC
`floor(epoch / resolution) * resolution`. Negative epochs floor correctly and
PostgreSQL buckets no longer depend on a timezone-sensitive origin literal. The
first bucket may be partial and its label may precede `since`. There is no gap
filling, upper bound, or downsampled bucket cap. Core rejects strides that cannot
fit i64 microseconds. Unrepresentable bucket timestamps fail rather than silently
becoming epoch zero; extreme strides and old dates can still produce such errors.

System and app reads intentionally remain separate statements without a joint
snapshot. They are independently sampled diagnostics and may observe different
concurrent commits. Stored clocks remain PostgreSQL's database default and Turso's
adapter UTC clock. Retention keeps strict `< cutoff` and atomic two-table deletion.
Runtime numeric, boundary, timezone, overflow, and rollback acceptance is deferred;
production compilation does not execute these SQL queries.


### R10 analytics follow-up: stable reads and bucket membership

Analytics adapter queries now use one database read snapshot for scope counts,
compatible devices, and buckets (PostgreSQL read-only repeatable read; Turso read
transaction). Catalog lookup is intentionally separate and resolves the latest
immutable revision metadata visible at that lookup. Samples still span matching
historical revisions of the selected blueprint; no latest-revision-only filter is
introduced.

Corrected differences are binary name/ID ordering, binary latest-event ID ties,
flooring negative UTC epoch buckets in Turso, and stable ID ties for same-label
core device series. Both `[start, end)` persistence bounds round upward to storage
microseconds, preserving requested membership. Response bounds and point-budget
calculations remain unchanged. Existing integer-to-f64 sample conversion and
weighting/coverage algorithms are retained, including precision loss for large
integers. Read snapshots can live for the duration of a bounded analytics query;
no writer mutex is acquired for Turso reads. Runtime acceptance is deferred.
