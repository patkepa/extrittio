# Backend identity implementation and deferred evidence

Source audit: 2026-09-14. This closes R13 implementation reconciliation, not
behavioral verification. Tests and migrations were not executed. The active
[execution plan](backend-refactor-execution-plan.md) controls scope; ADR-008 and
ADR-009 in [the decisions](backend-crate-refactor-decisions.md) define contracts.

## Session identity (PI-17)

| Requirement | Current implementation | Remaining evidence |
| --- | --- | --- |
| Backfill distinct epochs and require nonempty values | PostgreSQL migrations `20260831010000_user_auth_epoch` and `20260831020000_user_auth_epoch_nonempty` add random UUID defaults, uniqueness, NOT NULL, and a nonempty check. Turso `0008_user_auth_epoch.sql` backfills random values, adds uniqueness, and rejects null/empty inserts and updates through triggers. | Execute migration/backfill and constraint cases on both engines after verification resumes. |
| Fresh epoch for each principal | Both adapter `users.rs` creation paths and `bootstrap.rs` owner creation supply fresh UUIDs. Epochs remain stable across password/role updates, which invalidate permission versions instead. | Execute shared creation/delete/recreate contracts, including Turso maximum-ID reuse. |
| JWT binds the persisted principal | Host `domains/identity/auth_routes.rs` passes persisted tenant, ID, permission version, and epoch into `auth.rs` token issuance. Epoch stays out of public user DTOs. | End-to-end login, token inspection, and replacement-principal rejection. |
| Missing/empty epochs fail closed | `auth/context.rs` rejects them before tenant compatibility mapping; middleware also checks epoch presence before core session resolution. | Host cases covering missing and empty claims, both bearer and cookie credentials. |
| Same ID/version cannot revive an old session | Core `application/users.rs::resolve_session` compares active status and exact tenant, ID, permission version, and epoch from persisted details. | Core replacement cases exist in source; execute them and a complete host path after tests resume. |
| Security reads use one snapshot | PostgreSQL `users.rs` wraps credential/details hydration in read-only repeatable-read transactions. Turso wraps both in read transactions and hydrates through the transaction handle. | Concurrency evidence showing verifier, epoch, roles, and permissions cannot span principal revisions. |

Existing test source includes core epoch mismatch/replacement cases and shared
adapter epoch uniqueness/delete-recreate cases in
`crates/backend-adapter-tests/src/users.rs`. Their presence is not a passing
result. No host integration evidence was found in the retained backend integration
targets; migration, concurrency, and complete authentication-path evidence remain
explicitly deferred to R14. Do not implement epochs again to satisfy old plans.

## Tenant compatibility (EX-001)

Tenant selection must precede the database lookup because that lookup requires a
tenant. The mapper checks epoch presence first; persisted epoch equality is then
checked by core before an authenticated RequestContext is created. A nonempty but
mismatched epoch can reach preliminary tenant mapping, but cannot authenticate.
Rate limiting shares that mapper and includes the epoch in its signed-claim key;
unusable claims fall back to IP. It performs no independent default-tenant mapping.

The missing-tenant counter records preliminary compatibility mapping, not proven
successful authentication. The audit corrected one mismatch: missing/empty epochs
previously reached that counter before downstream rejection. They now fail at the
mapper without incrementing it; authorization and IP-fallback outcomes stay the same.

Keep the epoch-bound missing-tenant compatibility branch until ADR-001's rollout
criterion is met: all issuers write tenant claims and no compatibility hits occur
for one supported token lifetime (currently 24 hours). No deployment telemetry was
provided or collected in this refactor. Branch removal therefore remains a deferred
rollout action, not an invented assumption of zero use. Default login/bootstrap
tenant selection remains supported behavior.

## Portable usernames (PI-16)

Core rejects U+0000 before hashing or database calls. User creation keeps permission,
trim, and empty-name checks first, then returns InvalidInput with
`Username must not contain NUL characters` (HTTP 400 / `bad_request`). Bootstrap
uses the same rejection after its existing empty-name check. The exact local
`admin` bootstrap exception remains. Authentication returns generic Unauthorized
(HTTP 401) for NUL input; other usernames retain exact, untrimmed lookup.

This deliberately closes PostgreSQL/Turso input divergence. Existing Turso rows
are not rewritten or deleted; NUL-bearing names can no longer log in by username.
Already issued sessions still follow epoch validation. No storage encoding, schema
migration, username normalization, or account-remediation flow is added. Runtime
create/bootstrap/login coverage remains deferred.

## API keys and CI ingestion

Core `application/api_keys.rs` implements create/list/delete. Keys are generated
through the existing outbound generator, plaintext is returned on creation, and
repositories persist hashes/prefixes. There is no API-key nonce consumption or
rotation operation in the current port, application, or route. Historical nonce
and rotation work is deferred product scope, not an unfinished extraction.
Certificate encryption nonces and device credential rotation are separate flows.

Preserve the documented engine differences: Turso baseline schema enforces unique
`(tenant_id, name)` for API keys; PostgreSQL has hash uniqueness but no equivalent
name constraint. PostgreSQL CI ingestion retains best-effort last-used updates
outside the firmware insert transaction; Turso performs its existing write sequence
in one transaction. This audit does not introduce convergence, automatic key
rotation, or new nonce state. Shared API-key/ingest runtime and concurrency evidence
is deferred under the same test policy.
