# Backend Architecture Review — Scalability & Refactoring

The codebase has a solid foundation — clean layering (`api → services → repositories → db`), proper separation of concerns, and consistent patterns. Here are the refactoring opportunities that matter most as the system grows, ordered by impact.

---

## CRITICAL: Missing Database Transactions

**The single biggest correctness issue.** No multi-step write operation uses `conn.transaction()`, despite Diesel fully supporting it.

| Location | Risk |
|---|---|
| `services/device_service.rs` — `create_device` | Device created without shadow → 404s on shadow lookups |
| `services/device_service.rs` — `trigger_ota` | Shadow updated but deployment record missing |
| `services/firmware_service.rs` — `upload_firmware` | Firmware row exists with empty URL if blob insert fails |
| `services/shadow_service.rs` — `update_desired`/`update_reported` | Concurrent shadow updates race (read-modify-write without isolation) |
| `repositories/config_repo.rs` — `upsert_config` | TOCTOU race on the manual select-then-insert-or-update |

**Recommendation:** Wrap all multi-step service operations in `conn.transaction(|conn| { ... })`. For `upsert_config`, use SQLite's native `INSERT ... ON CONFLICT DO UPDATE` instead of the manual check.

---

## HIGH: Synchronous DB Calls on Async Threads

Every Axum handler and every Zenoh subscriber calls `db_pool.get()` and runs Diesel queries synchronously on Tokio worker threads. With only 4 pool connections and multiple concurrent handlers + 6 subscriber loops competing, this will block the async runtime under load.

**Recommendation:** Wrap DB-touching code in `tokio::task::spawn_blocking` or use `deadpool` (async pool) instead of `r2d2`. This is the most impactful scalability fix.

---

## HIGH: No Input Validation Layer

There is essentially zero application-level validation. Empty strings, negative numbers, and arbitrary-length inputs are all accepted. As the API surface grows, this will become a persistent source of bugs.

**Recommendation:** Introduce a validation trait or use a crate like `validator` with derive macros on request structs. Validate at the API boundary, fail fast with 400/422.

---

## HIGH: Background Tasks Bypass the Repository Layer

`background.rs` contains inline Diesel queries instead of calling through `repositories/`. This means:
- Query logic is duplicated outside the canonical location
- If table/column names change, two files break instead of one
- These queries can't be unit-tested via the repo interface

**Recommendation:** Add `device_repo::mark_offline_devices(conn, cutoff)` and `command_repo::timeout_stale_commands(conn, cutoff)` and call those from `background.rs`.

---

## MEDIUM: No Role-Based Access Control

The `role` field exists in `users` table and `Claims` JWT struct but is never checked. Any authenticated user can create/delete other users, modify firmware, etc.

**Recommendation:** Add a middleware or extractor that checks `claims.role` against required permissions per route (or route group). This is essential before multi-user deployments.

---

## MEDIUM: Duplicated Code Patterns

Several patterns are copy-pasted across handlers:

| Pattern | Files | Fix |
|---|---|---|
| `since` date parsing (try NaiveDateTime, fallback RFC3339) | `telemetry.rs`, `logs.rs` | Extract to a `parse_since(s: &str) -> Result<NaiveDateTime, AppError>` helper |
| `UniqueViolation → Conflict` mapping | `users.rs`, `firmware_updates.rs` (x2) | Extract `map_unique_violation(e, msg) -> AppError` |
| `FirmwareUpdateResponse` construction | `firmware_updates.rs` (x3) | Add a `From<(FirmwareUpdate, ...)>` impl or builder |
| Device-exists guard | Uses `find_device`, `find_device_with_joins`, or `device_exists` inconsistently | Standardize on `device_exists` for guard checks, `find_device` only when data is needed |

---

## MEDIUM: Race-Prone Insert-Then-Re-Read

`fleet_repo::insert_fleet` and `device_type_repo::insert_device_type` re-read via `ORDER BY id DESC LIMIT 1` after insert. Under concurrent inserts, this returns the wrong row.

**Recommendation:** Use `last_insert_rowid()` or re-read by a unique natural key (name) instead of relying on max ID.

---

## MEDIUM: No Request-Level Observability

There is no request logging middleware, no request ID propagation, no metrics. As the system grows, debugging production issues will be very difficult.

**Recommendation:** Add `tower-http::trace::TraceLayer` for per-request structured logging. Consider adding a request ID header (X-Request-Id) that propagates through to DB operations and Zenoh publishes.

---

## MEDIUM: Firmware Blobs in SQLite

Binary firmware files are stored as BLOBs in SQLite and loaded entirely into memory for upload/download. No size limit is enforced.

**Recommendation:** For now, add a configurable max file size check in the upload handler. Long-term, store blobs on the filesystem with only the path in the DB.

---

## MEDIUM: Silent Subscriber Death

If a Zenoh subscriber channel closes at runtime, the handler loop breaks silently. The process keeps running but stops processing that message type with no alerting.

**Recommendation:** Add a health-check mechanism — either a `/health` endpoint that verifies all subscribers are alive, or restart logic with exponential backoff.

---

## MEDIUM: No Topic-Key Device ID Validation

Zenoh handlers read `device_id` from the protobuf body, not from the topic key. A compromised device can spoof another device's ID.

**Recommendation:** Extract the device ID from the Zenoh key expression and cross-validate against the protobuf field.

---

## LOW: Naming & Consistency Issues

- `dashboard_repo::get_total_firmware` actually counts telemetry records — rename to `get_total_telemetry_count`
- `firmware_updates.rs` line 261: `firmware_update_id: 0` sentinel in `NewFirmwareBlob` construction is fragile
- `connection held across .await` in `shadow_service.rs` — acknowledged in a comment but reduces pool throughput

---

## LOW: Missing Pagination

`firmware_repo::list_firmware_updates` and `fleet_repo::list_fleets` return all rows. Other list endpoints have pagination. Apply consistently.

---

## Suggested Architecture Evolution

As the codebase grows, consider these structural improvements:

1. **Error helper module** — centralize common error mapping patterns (`unique_violation_to_conflict`, `not_found_to_unauthorized`, etc.)

2. **Shared DTO module** — move request/response types to `api/dto.rs` or per-domain DTO modules. Currently they're file-local, which leads to `use super::auth_routes::UserResponse` cross-imports.

3. **Config as part of AppState** — `AppConfig` is consumed and thrown away in `main.rs`. Store it in `AppState` so handlers can access runtime config (e.g., timeouts) without hardcoding.

4. **Graceful shutdown** — background tasks have no cancellation token. Use `tokio_util::sync::CancellationToken` or `tokio::select!` with a shutdown signal so tasks clean up properly.

5. **Test infrastructure** — there are no integration tests for the backend (only proto crate tests). Adding a test harness with an in-memory SQLite DB and mock Zenoh session would catch transaction/race issues early.

---

The architecture is genuinely well-structured for its current size. The biggest wins are **transactions**, **spawn_blocking for DB calls**, and **input validation** — fixing these three will put the foundation on solid ground for growth.
