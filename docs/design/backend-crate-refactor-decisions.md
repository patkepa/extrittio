# Backend Crate Refactor Decision Records

- **Status:** Accepted for the backend crate refactor
- **Scope:** P0-A decisions required by `backend-crate-architecture-plan.md`
- **Decision date:** 2026-08-31
- **Compatibility rule:** Preserve externally observable behavior unless this document explicitly approves a correction

This document resolves the behavior choices that must be stable before code moves between crates. It is subordinate to the architecture plan, but it is the implementation authority for the decisions below. Evidence paths describe the current tree; they are not target ownership.

| ADR | Decision | Primary owner |
|---|---|---|
| ADR-001 | Missing-tenant compatibility is restricted to legacy signed user JWTs | P1.2 |
| ADR-002 | Request/access audit remains best effort and never invents a tenant | P1.2, P3.7 |
| ADR-003 | A failed command publish returns 502 and retains the recorded dispatch attempt | P3.4 |
| ADR-004 | Firmware upload/delete retains the current best-effort compensation order | P3.6 |
| ADR-005 | Rule snapshots reload every 5 seconds by default, with a bounded healthy-state contract | P3.3 |
| ADR-006 | Tenant zone lists use binary `name ASC, id ASC` order | P2.4 |
| ADR-007 | Roles use system-first binary order, atomic permission invalidation, and one timestamp update per successful mutation | P3.1 |
| ADR-008 | Users use exact-tenant atomic role/password semantics with injected host password hashing and clock ports | P3.1 |

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

The host owns one compatibility mapper from validated user JWT claims to the non-optional core `TenantContext`:

1. A present, valid tenant claim maps to that tenant.
2. An absent tenant claim on an otherwise valid signed **user JWT** maps to `DEFAULT_TENANT_ID`.
3. A present but empty or invalid tenant claim is rejected as unauthorized; it never falls back.
4. API keys, device identities, worker/system actors, repository calls, and arbitrary optional tenant inputs never use this fallback.
5. Login with an omitted tenant continues to authenticate against the default tenant for HTTP compatibility. The token issued by a successful login contains an explicit tenant claim.
6. Rate limiting must consume the same mapped identity where available. It must not implement a second tenant fallback. Before authentication mapping is available, it may fall back to an IP key, not a fabricated tenant identity.

The compatibility mapper records a counter and a structured warning without logging the token. Its result is a non-optional `TenantId` before any application use case is invoked.

### Consequences

- Old signed user tokens remain usable during a rolling upgrade.
- Malformed tenant claims fail closed instead of gaining default-tenant access.
- Core identity types do not know about JWTs or `DEFAULT_TENANT_ID`.
- Default-tenant bootstrap and default login selection remain valid product behavior but are separate from runtime tenant fallback.

### Compatibility and rollout

- P1.2 introduces the mapper and routes authentication plus rate-limit identity through it.
- Keep the missing-claim branch through mixed-version deployment and for at least one maximum token lifetime, currently 24 hours, after every token issuer writes tenant claims.
- Remove the branch in P6 only after no compatibility-hit metric has been observed for one full supported token lifetime. If external issuers still require it, retain it only through a separately approved, time-bounded compatibility policy.
- Removing default-tenant substitution from audit and workers is an intentional tenant-isolation correction, not an API compatibility break.

### Required tests

- A signed JWT with a valid tenant maps to that exact tenant.
- A signed legacy JWT with no tenant maps to the default tenant and increments the compatibility metric.
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

- **Status:** Accepted for healthy dependencies; no partition-time guarantee
- **Owner work packages:** P3.3 for snapshot semantics; P4.3 for supervision/readiness

### Context and evidence

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
- Password validation and Argon2 execution were host service functions. Moving orchestration to core required a stable password policy without moving concrete crypto, randomness, Tokio blocking work, or environment access into core.
- Password changes and role replacement invalidate signed authorization state through `permission_version`. Last-owner deletion or demotion is a tenant security invariant that must remain correct under concurrent writers on both engines.

### Decision

The core `UserApplication` and `UserRepository` contract is:

1. Every repository operation requires the exact tenant. A user or role owned by another tenant behaves as missing and no rejected operation mutates either tenant.
2. User creation trims leading and trailing username whitespace once and rejects an empty result. Otherwise usernames remain exact and case-sensitive; authentication passes the supplied username through without trimming or case normalization. Exact usernames are unique per tenant and duplicate writes map through the stable `users.tenant_username` constraint. The same username may exist in another tenant.
3. User pages sort by binary `username ASC, id ASC`. Hydrated assigned roles sort by binary `name ASC, id ASC` without the system-first grouping used by role-management lists. Effective permission keys are distinct and binary ascending. The compatibility primary-role projection chooses `owner`, then `admin`, then the first binary-sorted assigned role.
4. `role_ids: None` selects the exact tenant's built-in `viewer` role. An explicit empty role list is invalid at the application boundary. Explicit IDs are sorted and deduplicated before persistence; a missing or cross-tenant ID returns the typed roles-not-found outcome without a partial user or assignment write.
5. User creation, selected-role validation, assignments, the compatibility primary role, and the initial permission-version update are one transaction. A successful create is returned fully hydrated and has `permission_version == 2`, preserving the existing insert-then-assignment behavior.
6. `set_roles` is a complete atomic replacement. Missing users, missing roles, and last-owner rejection have separate typed outcomes and roll back every assignment, primary-role, and version change. Every successful call increments `permission_version` exactly once, including an identical-value retry.
7. Only the `owner` role has a last-member guard; a sole `admin` may be demoted. All owner assignments count, including inactive users. Delete and role replacement serialize around the tenant's owner state so concurrent delete/delete, demote/demote, or delete/demote operations cannot remove every owner. Rejected operations do not increment versions. A successful delete retry returns not found.
8. Every successful password write replaces the encoded verifier and increments `permission_version` exactly once, including a retry with the same verifier. Missing and cross-tenant users return the typed not-found outcome and do not mutate state.
9. Successful-login bookkeeping stores the caller-supplied UTC instant at microsecond precision, is last-write-wins, and never changes `permission_version`. After credentials have verified, a concurrent delete that makes this best-effort bookkeeping update return not found does not retroactively fail authentication.
10. User list, credential lookup, and hydration remain multi-statement reads without a snapshot transaction. This preserves current behavior: a concurrent writer may make total/page/hydrated projections span revisions. The port does not promise snapshot consistency.
11. Core password validation uses UTF-8 byte length of at least 12 and requires at least one ASCII lowercase character, ASCII uppercase character, ASCII digit, and character outside the ASCII alphanumeric set. Consequently ASCII punctuation, whitespace, and non-ASCII characters satisfy the symbol category. Existing validation order and public messages remain stable.
12. Core receives a `PasswordHasher` and UTC `Clock` as outbound ports. The host implementation uses Argon2's existing default parameters, a fresh `OsRng` salt, PHC encoding, and blocking-task isolation. Plaintext request types deliberately lack `Debug`, `Clone`, and serialization; plaintext is zeroized after host hashing/verification; encoded hashes have redacted `Debug`, no serialization, and appear only at the credential-specific boundary.

The contract does not select behavior for embedded NUL in usernames. PI-16 must decide whether core rejects it with a stable invalid-input error or both storage adapters are made to accept the same representation.

### Consequences

- HTTP routes, DTO fields, status codes, JWT claims, cookie behavior, validation messages, and OpenAPI schemas remain unchanged while orchestration moves through `Application`.
- General user reads cannot expose an encoded password hash; authentication receives it only through `UserCredentials`.
- PostgreSQL row locks and Turso's serialized writer implement the same owner invariant and atomic aggregate outcomes without a generic cross-engine transaction abstraction.
- Exact login lookup intentionally differs from create-time trimming. A client that creates `" alice "` persists `"alice"` and must authenticate as `"alice"`.
- Same-role and same-password retries intentionally invalidate previously issued authorization once per successful call; they are not version-idempotent.
- Multi-statement read consistency and PI-16 remain explicit limitations, not accidental guarantees.

### Implemented evidence and remaining proof

- Core unit tests cover permission-before-validation ordering, create-time trim, empty-role rejection, role-ID normalization, the byte/ASCII password policy, stable error mappings, exact untrimmed login lookup, inactive/wrong/malformed credential rejection, injected clock use, and secret-safe encoded-hash debugging.
- The shared user adapter contract covers exact tenant isolation, cross-tenant same-name users, binary ordering and pagination, stable repeated lists, default viewer and primary-role priority, distinct permission ordering, atomic missing-role rollback, stable duplicate mapping, repeated password and same-role version increments, delete retry, and concurrent create/role-delete plus all combinations of final-owner deletion/demotion.
- Host tests verify a pre-existing PHC Argon2 verifier, malformed-verifier failure, password round trips, wrong-password failure, and distinct random salts for identical plaintext.
- The owner-count queries deliberately omit an `is_active` filter, but the shared harness does not yet mutate a fixture to inactive and assert that it still protects the tenant. The login contract asserts one caller timestamp and no version bump, but does not yet perform two writes to demonstrate last-write-wins. Add those focused regression cases when extending the harness; do not describe them as current test coverage.
- Embedded-NUL parity is intentionally untested until PI-16 selects a portable contract.

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
