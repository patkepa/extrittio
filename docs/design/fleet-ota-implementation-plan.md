# Fleet OTA implementation plan

Status: proposed implementation; no product changes are implied by this document.

Baseline: `d292304` (2026-09-07). This checkout includes mandatory checksums,
deployment IDs, signed download grants, transactional OTA shadow handling, native
installation journals, and ESP32 boot confirmation. Preserve and extend these
protections. They are not new work to rebuild from scratch.

Purpose: evolve Extrittio into an OTA system that can safely operate large fleets,
with measurable delivery limits, controlled rollouts, reliable recovery, release
authenticity, and sufficient evidence for operators to understand every result.

## 1. Scope and intended milestones

Deliver the work as vertical slices within the existing modular monolith. Keep
PostgreSQL for the production service and Turso for the edge deployment. Both
adapters must implement the same business semantics; they do not need identical
throughput or database execution strategies.

Initial supported update strategies are native executable replacement and ESP32
application partition swap. The existing blueprint values `package` and `external`
must be reported as unsupported until their handlers and recovery contracts exist.
Whole-OS updates, bootloader/partition-table replacement, multi-component atomic
updates, binary deltas, and gateway caching are later work, not implied support.

| Milestone | Deliverable | Conditions before enabling it |
| --- | --- | --- |
| M1: trusted device updates | Signed immutable releases, enforced compatibility, tested boot recovery, refreshable download credentials | A qualified device family and mixed-client migration tests |
| M2: controlled fleet pilots | Durable campaigns, retries, timeouts, staged rollout, pause/cancel, aggregate status | Multi-worker correctness and failure-injection tests |
| M3: large-fleet release | Scalable device authorization, artifact delivery, admission limits, HA recovery, operator tooling | Load tests, hardware recovery tests, security review, and a production pilot |
| M4: extended capabilities | Delta images, edge caching, additional installers and richer governance | Separate measurements and qualification per capability |

These are proposed capacity targets for M3, not claims about the current system:

- 100,000 registered devices, a 100,000-device campaign snapshot, and 10,000
  concurrently connected simulated devices in the first production qualification.
- A tested configuration admitting 1,000 new attempts/minute with at most 1,000
  outstanding reservations/active attempts; configurable lower tenant and campaign
  limits apply. With a two-minute mean execution time, that concurrency cap limits
  sustained completions to roughly 500/minute. Publish measured throughput.
- At least 1,000 simultaneous artifact downloads through the selected object-store
  or CDN path. Qualify bandwidth independently from control-plane throughput.
- Ten million historical attempt events when testing query/index behavior.
- Under the qualified load: p95 campaign creation acknowledgement below one second,
  p95 current campaign page/summary reads below 500 ms, and p95 ready-work dispatch
  below five seconds when capacity and network are available.
- A committed pause prevents subsequent admission transactions immediately;
  externally visible worker quiescence within five seconds under the tested load.
  Previously issued device permits remain subject to their documented expiry.

Record instance sizes, database settings, artifact sizes, latency/loss, fleet
heartbeat rates, worker counts, and test duration with every result. Increase
capacity only after measuring the same workload with the proposed configuration.

## 2. Current implementation and required changes

| Existing anchor | Keep | Extend or change |
| --- | --- | --- |
| `crates/backend/src/domains/firmware/types.rs` | Required artifact fields and forward-only state transitions | Typed outcomes, retry classification, rich compatibility and protocol versions |
| `crates/backend/src/persistence/{postgres,turso}/firmware.rs` | Tenant/device/attempt matching and transactional completion | Core-owned business ports, admission control, durable work, no silent superseding of a running install |
| `crates/backend/src/domains/devices/devices.rs` | Permission checks and filtered selection | Add asynchronous campaigns; current 500-device HTTP loop is not the fleet scheduler |
| `crates/backend/src/domains/firmware/download.rs` | Purpose-specific grant validation | Dedicated signing keys, refresh, shorter lifetime, exact artifact binding |
| `crates/backend/src/domains/firmware/firmware_store.rs` | Local/S3 storage and readiness checks | Streamed IO, immutable objects, direct delivery, resumable transfers |
| `clients/rust/sdk/src/native_ota.rs` | Synced journal, previous image and attempt-aware rollback | Recovery across whole-machine reboot, independent updater supervision, versioned journal |
| `clients/rust/runtime/src/lib.rs` | Download verification and lifecycle integration | Stop patching device identity into downloaded firmware; durable report delivery and reconciliation |
| `clients/{rust/esp32,c/esp32-idf-c,arduino/Extrittio}` | Partition swap and delayed boot validation | Signatures, capability negotiation, explicit application health, failure qualification |
| `crates/backend/src/init.rs` | Deny-by-default certificate/topic binding | Remove dependence on loading every device into a static startup ACL |
| `apps/frontend/src/pages/updates.tsx` | Device status and history access | Campaign controls, fleet-wide statistics, truthful transfer progress |
| PostgreSQL and Turso migrations | Tenant isolation and legacy IDs | Retained history, cross-adapter deletion semantics, queue indexes |

Correction to the earlier audit wording: existing download grants are tenant- and
firmware-scoped bearer credentials. They are not currently bound to a device.
Embedding an identifier in a presigned URL would also not make its bearer
credential non-transferable. Distinguish issuance authorization from possession
of the resulting download URL throughout the implementation.

## 3. Architecture and ownership

Follow [the backend architecture plan](backend-crate-architecture-plan.md).
Coordinate the firmware extraction with its P3.6 slice and the device identity
work with P3.1. Land small prerequisite ports where needed; do not wait for the
entire refactor or add new HTTP-handler-to-repository exceptions.

| Owner | Proposed responsibility |
| --- | --- |
| `extrittio-backend-core` | Release/campaign/attempt types, compatibility policy, authorization, state transitions, retry and wave decisions, narrow repository ports |
| `extrittio-backend-postgres` | SQL, migrations, atomic admission, worker claims, fencing, indexed reads and production concurrency |
| `extrittio-backend-turso` | Same business operations using bounded serialized writes and adapter-owned migrations |
| `extrittio-backend` | HTTP and Zenoh translation, device authentication, supervised workers, object-store/CDN integration, cryptographic service adapters, telemetry |
| `crates/common` and `clients/rust/sdk` | Versioned wire types, manifest parsing and client verification/recovery support |
| C and Arduino SDKs | Equivalent constrained-device protocol and verification behavior |
| Web, CLI and iOS | Operator workflows through the public application API |
| CI/release tooling | Build once, record provenance, sign approved metadata and publish immutable artifacts |

Proposed modules include `backend-core/src/firmware/`,
`backend-core/src/ota/`, application façades for each, and adapter-owned firmware
and OTA modules. Choose final filenames with the existing refactor conventions.
Do not introduce another generic repository collection, generic workflow engine,
or a broker just to implement OTA.

Use a dedicated OTA work table initially. Reuse proven outbox patterns and worker
supervision, but do not insert OTA actions into the rule-action outbox without a
deliberate shared abstraction and a migration of its existing contract.

The target flow is:

1. CI publishes an immutable signed release.
2. An operator previews and approves a frozen campaign target set and policy.
3. A scheduler transaction reserves capacity and creates an exact device attempt,
   assignment, event, audit record, and delivery intent.
4. A worker sends an update-available hint. The authenticated device also checks
   periodically and on reconnect, so a missed hint cannot strand an assignment.
5. The device obtains its assignment, verifies the release and local eligibility,
   obtains a short-lived download URL, and stages the image.
6. A final start authorization and local safety check precede activation.
7. The device validates startup, commits or rolls back, and retransmits its result
   until the service acknowledges that exact event.
8. Campaign policy evaluates the result and subsequent health before advancing.

## 4. Durable model and invariants

Use UUIDs serialized as strings for new public identities. Preserve existing
integer firmware/deployment IDs for legacy APIs and fixtures; add a mapping to new
records rather than silently changing ID types. Use UTC timestamps with the
existing adapter time-precision contract, signed 64-bit byte counts, and typed
states with database checks. Server receipt time drives deadlines; device time is
evidence, not an authority for server scheduling.

### 4.1 Proposed tables

| Table | Essential fields and constraints |
| --- | --- |
| `firmware_releases` | Tenant, ID, product/device family, display version, monotonic security version, immutable compatibility document, installer strategy, manifest digest, lifecycle, creator, approval/provenance references |
| `firmware_artifacts` | Tenant, ID, release ID, digest, byte length, format, architecture/board variant, storage backend/key/object version; immutable after verification |
| `firmware_trust_metadata` | Tenant/product trust namespace, role/key ID, algorithm, public material, version, expiry, metadata digest; private release keys are not database fields |
| `device_ota_capabilities` | Tenant/device, protocol version, installer versions, board/hardware revision, architecture, flash layout, rollback/signature features, free-space/power evidence and freshness timestamps |
| `ota_campaigns` | Tenant/ID, lifecycle, release set, selector, immutable policy version, target snapshot digest, creator/approver, schedule, next evaluation, optimistic version, aggregate freshness |
| `ota_campaign_waves` | Tenant/campaign/wave index, cumulative boundary, frozen seed, gate policy, current gate outcome and timestamps |
| `ota_campaign_targets` | Tenant/campaign/device unique, selected artifact, frozen eligibility evidence, wave, current target outcome, attempt count, next eligible time |
| `ota_attempts` | Tenant/ID, target, ordinal, artifact/manifest digest, prior installed digest, observed execution phase, orchestration disposition, last sequence, deadlines, error code, reservation/permit fields |
| `device_ota_slots` | One row per tenant/device; current attempt, monotonic fencing generation, last assignment; admission uses a compare-and-swap/row lock |
| `ota_attempt_events` | Tenant/attempt/event ID unique, sequence, phase, bytes, error, health evidence, device/receipt timestamps; deduplicated retained history |
| `ota_work_items` | Tenant/ID, type, aggregate ID/version, unique deduplication key, availability, lease owner/generation/expiry, delivery count, bounded error |
| `ota_idempotency_keys` | Tenant/actor/operation/key unique, canonical request hash, resource/result, retention deadline |
| `ota_campaign_audit` | Tenant/event, actor identity, action, campaign/attempt/release references, approved snapshot and policy digests, before/after version, reason, timestamp |

Do not depend on cascade deletion to retain history. Archive referenced releases
and tombstone deleted devices. Preserve an identity snapshot in history. A
separate retention job removes eligible records only after references, recovery
requirements, active grants, and retention policy permit it.

All tenant-owned relationships use composite tenant/ID foreign keys where the
adapter supports them. Both adapters must reject cross-tenant references even
when IDs happen to exist elsewhere. Object paths remain tenant-separated; do not
introduce global cross-tenant deduplication or digest-existence probing.

### 4.2 Atomic operations

Repository methods express entire business mutations, for example:

- `finalize_release`: seal validated artifact metadata and append release audit.
- `freeze_campaign_targets`: persist one consistent selector result and its digest.
- `approve_campaign`: bind approval to immutable release, policy and target digests.
- `admit_attempt`: validate campaign revision, wave, schedule, eligibility, quotas
  and device slot; reserve capacity; create attempt, assignment and work together.
- `record_attempt_event`: deduplicate, verify identity/sequence/transition, update
  attempt, release or quarantine its slot, and enqueue aggregate work atomically.
- `change_campaign_state`: compare expected revision, persist pause/cancel/reason,
  and invalidate unissued work in the same transaction.
- `claim_work` / `finish_work`: lease with a new fencing generation; completion
  requires the current owner and generation, not just a reusable worker name.

An event and its necessary state changes either all commit or none do. A publish
failure never deletes durable intent. No DB transaction spans network, KMS,
signature-service, object-store, or device calls. Delivery is at least once;
device and backend deduplication make repeated delivery safe. Do not promise
exactly-once execution of a physical flash operation across power failures.

Keep a documented lock order for operations that share quota, campaign, target,
device-slot and shadow rows. All writers, including legacy OTA and shadow repair
paths, must respect the same order. Reports should enqueue aggregate evaluation
instead of updating a single hot campaign counter for every progress message.

### 4.3 States and uncertainty

Campaign states: `draft`, `resolving`, `awaiting_approval`, `scheduled`, `running`,
`paused`, `cancelling`, `completed`, `cancelled`, `aborted`, `failed`.
An auto-abort records a terminal campaign decision while tracking issued attempts
until they reconcile; do not erase their ongoing execution.

Device execution phases: `accepted`, `downloading`, `verifying`, `staged`,
`installing`, `rebooting`, `validating`, then `succeeded`, `failed`, `rolled_back`
or `rejected`. Unstarted targets may become `skipped` or `cancelled`.

Track orchestration disposition separately: `queued`, `active`, `timed_out`,
`cancel_requested`, `reconciliation_required`, `settled`. A server timeout is
not proof that an offline device stopped installing. Accept authenticated late
observations as reconciliation evidence without rewriting a terminal campaign
decision or counting a late success twice.

One device has at most one live installation authorization. Do not free its slot
merely because a server timer expired during install/reboot. Quarantine uncertain
attempts until the device reports, its unstarted permit safely expires, or an
explicit recovery process resolves the device. New campaigns queue behind this
slot. Replace today's automatic superseding with explicit safe cancellation for
unstarted work and a conflict/queued outcome for in-flight installation.

### 4.4 Index and query plan

Starting indexes, subject to `EXPLAIN (ANALYZE, BUFFERS)` on realistic data:

- Tenant/campaign/status/device on targets, and tenant/campaign/wave/device for
  stable target traversal; unique tenant/campaign/device.
- Tenant/device/created-at/ID for attempt history and tenant/campaign/created-at/ID
  for campaign events. Cursor pagination includes all ordering columns.
- Ready work on `(available_at, id)` with a partial predicate for runnable states;
  tenant-ready work on `(tenant_id, available_at, id)` for fair scheduling.
- A separate partial lease-expiry index for work currently leased. Avoid a broad
  `OR` over runnable and expired work if it prevents efficient plans.
- Active attempts indexed by deadline and tenant; do not scan historical attempts
  to detect timeouts.
- Explicit indexes on referencing tenant foreign-key columns used in deletes,
  joins and admission.

PostgreSQL workers claim bounded batches with `FOR UPDATE SKIP LOCKED`. Fairness
uses bounded per-tenant queues/quotas, not a window sort over the entire backlog
on every claim. Turso claims through short serialized transactions. Both use the
same lease and fencing contract. Partition history only if measurement justifies
it; define archive/retention and uniqueness implications before partitioning.

## 5. Device protocol, signing and compatibility

### 5.1 Versioned negotiation

Add an OTA protocol v2 capability report and separate versioned v2 device routes
or messages. Keep current payloads and endpoints available during migration.
Unknown protocol/schema versions are rejected explicitly. An absent capability
report means legacy/unknown, never implicitly v2-ready.

The v2 assignment carries attempt ID, device identity/incarnation, assignment
generation, release/manifest digest, artifact reference, installer requirements,
and start policy. Separate immutable release metadata from expiring assignment
and download credentials; refreshing a URL must not require resigning a release.

Reports carry attempt ID, persistent event ID and sequence, installed digest,
execution phase, byte progress, error code, and relevant boot/health evidence.
Persist critical transitions and sequence allocation before publishing. The
backend acknowledges the accepted event/sequence. Retain terminal events until
acknowledged and resend on reconnect or a bounded timer.

Use authenticated HTTPS device endpoints as the proposed v2 source of assignments
and credentials; Zenoh remains a low-latency notification mechanism. Authenticate
devices with provisioned client certificates on a dedicated listener or trusted
ingress. Strip untrusted identity headers, and never authorize by a device ID in
the request body alone. A constrained-device spike must prove this path on each
supported board before protocol freeze; any Zenoh request/reply alternative must
offer the same authenticated identity, correlation and replay protections.

Check on startup, reconnect, after attempt completion, and periodically with
jitter. A proposed default is 15 minutes with ±20% jitter for idle devices,
shorter bounded intervals for active work, and exponential backoff on errors.
At 100,000 idle devices that is about 111 checks/second on average; qualify reconnect
bursts separately. Coalesce hints and bound queues on constrained clients.

### 5.2 Signed release trust

Use the TUF trust model: offline root keys, delegated release-signing authority,
versioned signed metadata and expiry. Select maintained verification libraries
and supported signature algorithms after a native/ESP32 compatibility spike.
ECDSA P-256/SHA-256 is the proposed common algorithm; freeze its signature encoding,
key representation, metadata encoding and limits in golden fixtures. Use existing
cryptographic primitives, not custom cryptography.

The spike must choose and record the precise verification profile for every
client. Prefer complete TUF verification where feasible. A constrained profile
must enumerate omitted protections and cannot be advertised as TUF-compliant.
If no acceptable constrained verifier fits, that device family remains outside
the signed fleet release until the gap is resolved.

The signed metadata binds artifact digest and length, product/hardware selectors,
installer strategy, security version and release identity. Reject unknown keys,
unsupported algorithms, duplicate/ambiguous fields, malformed signatures,
oversized metadata, stale trust versions and invalid target bindings. Stream
artifact hashing and verify before activation, even when transport TLS succeeds.

Keep signing authority separate from the API server and user-session JWT secret.
The API can choose an already-approved release; it cannot forge an arbitrary
release. Use an offline root quorum (proposed 2-of-3), protected release keys,
independently scoped online freshness/assignment keys, and rotation/revocation
runbooks. Key-management integration is configuration, not a reason to store raw
private release keys in application tables.

Persist trusted root versions and a security-version floor on the device.
Root rotation must bridge old/new trust and support devices offline for months.
Expiry requires a trustworthy time strategy: authenticated time, a persistent
last-trusted-time floor, and explicit cold-start behavior. Never disable expiry
because a device clock starts at zero. Distinguish expired download credentials,
which are refreshable, from expired release/trust metadata, which needs current
signed metadata.

Separate display version from security version. Boot recovery may return to the
last known-good image while validating a candidate. Do not irreversibly raise a
hardware anti-rollback floor before safe confirmation if that would prevent
recovery. Planned downgrade needs a separately authorized recovery policy and
must still respect hardware-enforced floors. Firmware signatures do not replace
secure boot or protected key storage; qualify those platform guarantees separately.

### 5.3 Immutable artifacts and device identity

Build one artifact per supported hardware variant. Remove the native runtime's
post-hash device-ID patching. Migrate embedded IDs into a durable provisioned config
before the first immutable update. Confirm identity, certificate paths and
device-contract references survive replacement and rollback. Fail activation if
identity preservation cannot be proven; do not log a warning and continue.

Retain the existing object-key layout for legacy artifacts. New artifacts can use
tenant/release/digest-based immutable paths with object version pinning and
conditional creation. Verify hash/length while finalizing uploads. Do not permit
overwriting a published artifact or reusing its release identity for different
bytes. External URLs need the same signed digest/length and verified immutable
identity; their delivery capabilities must be reported explicitly.

### 5.4 Eligibility and local safety

Replace unvalidated free-form compatibility decisions with a versioned typed
policy, retaining additional metadata only as non-authoritative annotations.
Evaluate:

- Product/board, hardware revision, architecture, image format and installer.
- Minimum bootloader/updater/protocol version, flash layout and partition capacity.
- Supported source releases/security versions and required intermediate releases.
- Required features, application/data-schema compatibility and rollback support.
- Device power/battery, free space, busy/safe state and maintenance window.

Preview compatibility against provisioned facts plus fresh capabilities. Recheck
at admission and on the device immediately before activation. Missing critical
hardware facts mean blocked/unknown. Transient conditions mean deferred with a
reason and next check, rather than permanent install failure.

Do not match only the *desired* blueprint revision: record the actual running
contract/firmware capabilities and whether assignment convergence has completed.
Unknown or unsupported installer strategies fail before allocating installation
work. Server policy and device verification must agree on the selected artifact.

## 6. Rollout policy and orchestration

### 6.1 Target snapshots and approval

Default to snapshot campaigns. A preview stores the selected IDs, exclusion
reasons, inventory revision/time, compatibility results, release/policy digests
and expiry. Approval binds the frozen snapshot. Inventory changes afterwards
cannot silently add recipients; admission can defer/exclude a target that is no
longer safe. Any release, target or policy expansion creates a new revision and
invalidates approval.

For the first 100,000-target implementation, use one indexed, bounded database
snapshot operation to materialize targets; qualify its transaction duration and
abort cleanly if it exceeds limits. Do not claim a point-in-time snapshot from
unversioned keyset reads over changing inventory. If larger fleets require
chunked snapshots, first introduce a stable inventory revision/read model.
No dispatch occurs while snapshot resolution is incomplete.

Continuous/dynamic targeting is M4. Its membership changes, approval semantics
and wave assignment require a separate contract.

### 6.2 Wave policy

Proposed conservative template: one explicit lab wave, followed by cumulative
1%, 5%, 25%, and 100% of the frozen eligible production set. Keep an immutable
random seed and persist membership so retries/restarts do not reshuffle devices.
Stratify by hardware revision/site where needed to avoid an unrepresentative
canary. Define rounding for small fleets, deduplicate targets, and skip empty waves.

Each wave defines a rate, outstanding-attempt limit, minimum completed sample,
minimum healthy sample, soak duration and maximum waiting time. Proposed pilot
gate: at least 20 settled installations, at least 99% successful among settled
installations, no verification/security failures or rollbacks, and 30 minutes
of application health. For fewer than 20 devices, require all selected devices
to settle and explicit approval to advance.

Define denominator semantics in code/tests: queued/offline devices are not
successes and do not satisfy the minimum sample. Timeouts and health regressions
have separate counters and policies. Soak begins only after the required healthy
sample exists. Missing/stale telemetry blocks promotion. A later regression in
an earlier wave can pause the whole active campaign.

Record every gate's input population, data freshness, computed rates and decision.
Never promote on rounded UI percentages or on a page of deployment results.

### 6.3 Admission, retries and cancellation

Persist cluster-wide rate reservations so adding workers does not multiply the
allowed rollout rate. Enforce global, tenant, campaign, site and device limits
where configured. Reserve capacity before issuing work; use expiring unstarted
reservations and bounded ready batches so a large offline fleet does not reserve
all active capacity indefinitely.

Separate transport delivery retry from a new device installation attempt. Retry
lost hints/reports using the same identity. A new execution retry gets a new
attempt ID linked to its target and predecessor. Proposed retry policy: transient
network/storage errors only, up to three new attempts with full-jitter exponential
backoff, constrained by campaign deadlines and windows. No automatic retry for
invalid signature, incompatible hardware, corrupt trust, or repeated boot failure.

Use separate acceptance, download-idle, maximum-download, install, reboot/health,
and offline-target deadlines. Profiles vary by device family and artifact size.
Server timeout marks uncertainty as described in section 4; it never authorizes
an overlapping flash operation.

Pause blocks new starts and promotions; already-started work can complete safely.
Cancel stops unstarted targets, requests cancellation at safe checkpoints, and
continues tracking running installs. Do not forcibly kill a device during flash
or claim immediate cancellation of an offline device. A short-lived one-use
activation permit, bound to device/attempt/generation/digest, reduces stale starts.
Define its maximum offline validity (proposed two minutes) and consume it durably
before activation. Once issued it cannot be revoked instantly on an unreachable
device; show this residual bound to operators.

An emergency rollback is a new signed campaign to an eligible known-good release.
It uses fresh preflight and device slots, its own approval and audit, and cannot
bypass security floors or data compatibility. Disabling further rollout is always
possible even if rollback is not.

## 7. Artifact delivery and platform recovery

### 7.1 Delivery path

Add authenticated attempt-scoped credential issuance. Verify device identity,
current assignment generation, campaign policy, release state and artifact binding
before issuing or refreshing a URL. Use dedicated key IDs/rotation for local
download grants. Keep user-session tokens out of device firmware and download URLs.

For S3/CDN, redirect or return a short-lived URL for an exact immutable object
version. For local storage, stream a file with bounded buffers. Implement and test
`HEAD`, byte `Range`, `206`, `416`, stable validators and resume. Direct presigned
URLs remain bearer credentials until expiry; cancellation/revocation prevents
renewal but cannot immediately invalidate already issued URLs at every CDN.
Cache object bytes by immutable identity behind authorization; never share a
cached authenticated response across tenants accidentally.

Upload to staging with bounded streaming and incremental hashing, then finalize
verified metadata. If direct multipart upload is used, the service must verify the
completed object rather than trust a caller-supplied checksum. Failed metadata
commit leaves a recoverable staged object for a grace-period cleanup job. Garbage
collection must respect concurrent finalization and immutable release references.

Clients checkpoint resumable transfers against digest, expected length and validator.
Verify the completed bytes before installation. Bound download size even if
`Content-Length` is absent or false. Handle a refreshed URL, changed object,
ignored Range, partial response, redirect and corrupt cached chunk explicitly.
Devices without flash/RAM budget for resume advertise that limitation and retry
the full image under policy. Delta updates remain a separate later feature.

Remove Arduino's implicit `setInsecure()` fallback. Use an explicit trust store or
supported certificate bundle. Production profiles require authenticated control
transport and verified HTTPS downloads; local development exceptions must be
explicit and cannot silently carry into a production build/profile.

### 7.2 Native recovery

The existing watchdog is a separate process, but a machine reboot kills it. Add a
small independently supervised updater/recovery entry that runs before the main
application on every boot and can restore a candidate that never executes.
Retain the current journal/backup concepts; version their format and support
reading old journals during bootstrap.

Specify crash-safe ordering for staging, signature verification, backup retention,
journal commit, activation, health validation and final commit. Recover from
power loss between every pair of writes/renames; handle full disk, read-only
filesystem, corrupt journal/backup and stale temporary files. Protect update
directories from symlink/path substitution and preserve required ownership/mode.

The updater owns exclusive installation locking. Verify digest at the activation
boundary, and tie health confirmation to the candidate process/attempt/boot
identity, not merely a pathname that another process could replace. Avoid doing
blocking disk/hash work on a shared async executor.

Application health should prove required services/config/data are usable. A
30-second sleep alone is insufficient. Report local boot commit separately from
fleet soak health so loss of the cloud connection does not by itself cause a
permanent reboot loop. Retry the success report after local confirmation.

Keep application config/identity outside the binary. Document reversible data
migrations; reject an automatic rollback policy if the candidate irreversibly
changes data that the previous application cannot read. Qualify Linux/systemd
and macOS/launchd separately, including required platform code-signing behavior.
Updating the recovery component itself needs a separate protected procedure.

### 7.3 ESP32 recovery

Use NVS to persist a versioned attempt journal before switching the boot partition.
Bind it to image digest and target partition, not only a partition address that
can be reused. Require usable A/B partitions and rollback-enabled bootloader in
capabilities and build validation. Confirm only after product-specific local
health passes; reboot watchdog protection starts early enough to cover startup.

Test C, Rust and Arduino separately on the actual supported boards. Validate
secure-boot/image-signature integration, brownouts, NVS recovery, watchdog resets,
network outages and rollback reporting. Select a fitting partition layout for the
Rust ESP32-C6 image before enabling it. Do not promise an OTA fix for an already
flashed bootloader/partition layout that requires physical reprovisioning.

## 8. Public API and operator experience

The following are proposed additions, not existing endpoints. Use a new version
for changed payload contracts; preserve old API behavior during deprecation.

| Endpoint family | Intended contract |
| --- | --- |
| `POST /api/v2/firmware-releases` and artifact upload/finalize routes | Create draft release, verify bytes, seal metadata; separate approve/publish/archive actions |
| `POST /api/v2/ota-campaign-previews` | `202` with preview job ID; resolve and freeze targets asynchronously |
| `GET /api/v2/ota-campaign-previews/{id}` | Counts, exclusions, digest, freshness and completion state |
| `POST /api/v2/ota-campaigns` | Preview ID/digest and immutable policy; idempotent `202` with campaign ID |
| `POST /api/v2/ota-campaigns/{id}/{approve,start,pause,resume,cancel}` | Authorized action with expected revision and reason; auditable response |
| `GET /api/v2/ota-campaigns/{id}` | Aggregate state, gates, policy, counts and freshness |
| `GET /api/v2/ota-campaigns/{id}/{targets,attempts,events}` | Filtered cursor pagination and export support |
| `POST /api/v2/ota-campaigns/{id}/retry` | Explicitly selected retryable targets; preserve predecessor history |
| `POST /api/v2/ota-campaigns/{id}/rollback` | Create a draft recovery campaign with compatibility preview |
| `GET /device/v2/ota/assignment` | Authenticated device's current generation/attempt; no caller-selected tenant |
| `POST /device/v2/ota/attempts/{id}/{download,authorize-install,events}` | Refresh credential, authorize safe activation, accept/deduplicate reports |

All mutation retries use tenant/actor/operation-scoped idempotency keys. Same key
and same payload returns the original resource; a different payload conflicts.
Store request digest and created resource atomically. Define retention and surface
expiry rather than allowing a delayed client retry to silently create a new fleet
campaign. `If-Match` or an equivalent expected-version field prevents stale edits.

Legacy single/bulk endpoints can eventually adapt to safe campaign/attempt
admission while retaining their existing status code and response schema. Do not
silently turn the existing `200` response into `202`. Their "succeeded" count
continues to mean accepted requests, not successfully installed devices; new
interfaces use clearer names. No legacy endpoint can bypass v2 device locks,
release policy or tenant quotas once the shared admission path is enabled.

Web campaign creation: select release, preview eligible/excluded targets, choose
waves and windows, review impact, approve, schedule/start. The detail view shows
global counts, success rate with denominator, actual bytes/phase progress, gate
evidence, failures by reason, offline/uncertain devices and the action timeline.
Label unknown progress honestly; remove fixed status-to-percent values as a claim
of measured download progress. Add accessibility and browser interaction tests for
pause, retry, conflicting edits and partially completed cancellation.

CLI adds campaign create/preview/watch/pause/resume/cancel/retry/rollback, stable
JSON output and watch by exact campaign/attempt ID. Generate OpenAPI and client
types in the same changes. iOS gets campaign list/detail, approval and emergency
controls after web/CLI semantics stabilize; it must not retain a bypass through
old direct-deploy controls.

Separate permissions for artifact upload, release publication, campaign creation,
approval, start, pause/cancel and recovery. Keep existing permission mappings during
migration. A production policy can require a distinct approver for broad rollouts;
an emergency stop should not require the same approval delay as deployment.
Capture release, policy and target digests in domain audit records within the
mutation transaction. Existing generic HTTP audit remains useful supplementary
evidence, but it does not provide those deployment facts.

## 9. Scalable identity and deployment topology

The current per-device Zenoh ACL builder is an operational scaling constraint;
it is not proof of a measured maximum fleet size. Benchmark authorization startup,
memory, connection churn and certificate lifecycle on the pinned Zenoh version.

Preferred topology: stateless authenticated device HTTP APIs for authoritative
assignment/report handling, dedicated Zenoh routing/notification infrastructure,
and independently supervised OTA workers sharing durable PostgreSQL state.
Clients poll even if a hint is lost, so HTTP replicas do not need to own the same
Zenoh session as every target device.

Prototype a dynamic principal-to-topic authorization mechanism that binds the
certificate identity to allowed device topics, updates provisioning/revocation
without restarting the whole backend, and rejects device-originated OTA commands.
If the pinned Zenoh stack cannot safely support it, use bounded router shards with
automated membership reload and qualified reconnect behavior as an interim
topology, or migrate device control to the authenticated HTTP path. Do not disable
ACLs or treat one common wildcard rule as equivalent device isolation.

Test revocation/rotation on established and new connections. Map certificates to a
stable device incarnation so delete/recreate cannot revive old assignments or
credentials. Tenant/device binding must survive broker routing and trusted-ingress
translation. Scope HA claims to the topology actually tested: multiple API workers
alone do not provide broker, database or object-store HA.

## 10. Implementation work packages

Each package includes both adapters where persistence changes, regression tests,
generated contracts when affected, migration notes, and a short operator document
for newly enabled behavior. The checklist starts unchecked even when underlying
primitives already exist; its purpose is to track completion of this plan.

### P0 — Baseline, contracts and qualification design

- [ ] Reproduce current OTA tests with features enabled; freeze existing wire/API,
  journal, object-key and database fixtures. Record the exact starting commit.
- [ ] Add missing PostgreSQL parity/concurrency coverage for the existing attempt
  identity, terminal cleanup and trigger/report race behavior.
- [ ] Write ADRs for protocol v2, IDs/legacy mapping, execution versus orchestration
  state, lock order, signing profile, trusted time and native recovery ownership.
- [ ] Inventory deployed client versions, provisioning modes, boards, flash layouts,
  data migration constraints and rollback capability; mark unknowns explicitly.
- [ ] Define reproducible load/HIL fixtures and the M1/M2/M3 acceptance matrix.

Exit: contract fixtures and migration rules are reviewable; the signing/device API
and recovery prototypes demonstrate feasibility on at least one native and one
MCU target. User-authentication security closure is a prerequisite for production
rollout, not something OTA can bypass.

### P1 — Extract firmware ownership and retain evidence

- [ ] Move firmware/OTA types and complete transactional operations behind core
  application façades and adapter-owned implementations. Remove replaced host
  repositories/bridges as each operation moves.
- [ ] Add release/artifact/capability tables and legacy ID links through additive
  migrations. Adopt archive semantics; remove destructive cascade paths to history.
- [ ] Version compatibility policy; implement pure eligibility and typed error codes.
- [ ] Route generic shadow repair/reset through rules that preserve active OTA
  ownership or explicitly cancel safely; no accidental deletion of pending work.
- [ ] Add shared adapter contracts for isolation, idempotency, compatibility,
  archiving, concurrent trigger/report and lossless history.

Exit: both engines preserve the same business invariants and the architecture
verifier passes. Old APIs still work against the extracted application boundary.

### P2 — Immutable signed releases and device trust

- [ ] Implement staging/finalization, immutable manifests, protected publication
  and signing-tool integration; record build provenance and optional SBOM links.
- [ ] Implement root bootstrap/rotation, metadata verification, security-version
  policy and trusted-time handling in Rust, C and Arduino.
- [ ] Ship the native identity-migration/bootstrap release and remove subsequent
  mutation of release bytes.
- [ ] Enforce typed compatibility at preview/admission and locally on device.
- [ ] Remove implicit insecure TLS fallback; prove key/URL redaction at application,
  proxy, tracing and error-reporting boundaries.

Exit: malicious/expired/replayed/oversized metadata and altered firmware cannot
activate; a long-offline device can rotate trust safely. Each enabled family has
documented verification guarantees and golden cross-language fixtures.

### P3 — Recoverable installers and acknowledged reports

- [ ] Add protocol negotiation, durable event IDs/sequences, report acknowledgement,
  periodic reconciliation and one-attempt installation locking in all clients.
- [ ] Add native recovery supervision across process and machine restart, including
  failure to execute the candidate at all.
- [ ] Version and migrate native/NVS journals; bind installed digest, attempt and
  boot identity; retain reports until acknowledged.
- [ ] Add product health callbacks and explicit local commit versus fleet health.
- [ ] Qualify ESP32 A/B layouts, rollback and supported secure-boot settings.

Exit: M1 recovery tests pass on each enabled platform, including power interruption,
lost terminal report, failed startup and incompatible data migration. A successful
download/reboot alone cannot be presented as a healthy installation.

### P4 — Scalable artifact and credential delivery

- [ ] Stream uploads/downloads with bounded memory and verified finalization.
- [ ] Add authenticated exact-attempt credential refresh, dedicated grant keys and
  S3/CDN direct downloads; preserve legacy grant support during migration.
- [ ] Implement Range/resume and validator behavior; advertise client limitations.
- [ ] Add upload/orphan cleanup and retention that cannot race active releases.
- [ ] Exercise URL expiry, interrupted download, object change, oversized response,
  slow readers and burst traffic under the selected storage backend.

Exit: artifact IO does not allocate a full firmware-sized buffer per concurrent
request in the API host. Offline reconnection refreshes authorization without
creating a second installation attempt.

### P5 — Durable campaigns and scheduling

- [ ] Add campaign/target/wave/attempt/device-slot/work/event/audit/idempotency tables
  and indexes. Implement snapshot preview and digest-bound approval.
- [ ] Implement atomic admission, cluster-wide quota reservations and safe legacy
  adapters. Refuse silent superseding of in-flight installs.
- [ ] Add supervised scheduler, dispatcher, expired-lease reaper, deadline checker
  and reconciliation/aggregate evaluator; expose worker readiness and queue lag.
- [ ] Implement lease generations, stale-worker rejection and bounded tenant fairness.
- [ ] Add versioned asynchronous APIs and CLI create/watch for a single-wave pilot.

Exit: 100,000 targets can be durably accepted without an HTTP per-device loop.
Process crashes and duplicate deliveries neither lose work nor overlap installs.
Two tenants and multiple workers share capacity correctly.

### P6 — Wave safety and operator controls

- [ ] Implement deterministic waves, freshness-aware health gates, soak, automatic
  pause/abort, and historical decision evidence.
- [ ] Add maintenance windows/time zones/DST behavior, safe start permits, explicit
  retry policies and separate deadline classes.
- [ ] Implement pause/resume/cancel and draft recovery campaigns with correct
  treatment of offline/running/uncertain devices.
- [ ] Build campaign web workflows, truthful progress, aggregate metrics and exports;
  update CLI and iOS controls and remove policy bypasses.
- [ ] Add permission/approval binding and revision conflicts throughout the UI/API.

Exit: M2 pilot is operable end to end; injected regression stops promotion and
shows the operator why. Pause/cancel races have executable contract tests.

### P7 — Identity scaling, HA and service operations

- [ ] Qualify the device-authentication topology and dynamic authorization strategy;
  implement revocation/provisioning without global backend restart.
- [ ] Validate replica/broker routing, reconnect storms, worker failover, database
  pool limits, object-store degradation and tenant fairness under sustained load.
- [ ] Add SLO dashboards, alerts, immutable audit export where required, backups,
  restore procedures, artifact retention and key-compromise runbooks.
- [ ] Document operating limits for production and edge separately; enforce those
  limits in admission rather than relying on operator discipline.

Exit: M3 operational and security gates pass for a named infrastructure profile.
All dependencies are qualified; do not equate adding replicas with proven HA.

### P8 — Pilot, cutover and large-fleet qualification

- [ ] Run the full matrix in section 12 and a multi-day soak with representative
  telemetry, artifact sizes and device/network behavior.
- [ ] Roll the bootstrap/trust-enabled client to an internal lab, then a small
  consenting production pilot, then successively larger cohorts under this engine.
- [ ] Require security review and measured capacity/recovery evidence before
  enabling broad production campaigns.
- [ ] Publish supported device/installer/version matrices, known limits and a
  release report; retire legacy write paths only after migration criteria are met.

Exit: M3 can be described using verified guarantees, supported platforms and tested
capacity. Future scale increases and new board families have explicit qualification.

## 11. Migration and cutover sequence

1. Back up database, firmware objects and trust/provisioning material. Rehearse a
   restore into isolation and identify which secrets are recoverable separately.
2. Deploy additive schemas and readers. Mark imported records as legacy with their
   actual evidence; never backfill an old success as a cryptographically verified
   v2 boot confirmation.
3. Backfill metadata in bounded batches with checkpoints and compare counts,
   tenant bindings, hashes and referenced objects. Quarantine missing/corrupt data.
4. Deploy v2-capable backend and disabled-by-default campaign workers. Legacy APIs
   retain their contracts; new workflows are gated by tenant and device capability.
5. Deliver a bridge client through the existing trusted provisioning/update channel.
   It persists native identity, installs/verifies recovery support, provisions the
   trust root and advertises protocol v2. This bootstrap inherits the security of
   the old channel; it cannot retroactively prove an uncompromised device. Physically
   reprovision devices whose trust or flash layout cannot be established remotely.
6. Enable signed-only policy per qualified device/cohort. Persist a minimum protocol
   and trust policy so an attacker cannot force fallback to an unsigned legacy path.
7. Drain or explicitly reconcile legacy in-flight attempts before moving a device
   into v2 admission. Import them into the same device slot while they are active.
8. Run M2 pilots and M3 qualification, then expand rollout permissions/capacity.
9. Remove legacy write behavior only when active attempts are settled, affected
   clients are migrated/retired, and the compatibility window has elapsed. Retain
   history readers and ID mappings as long as their retention contract requires.

Treat schema rollback, server rollback, device rollback and signing-key recovery
as four distinct operations. Old server binaries may be unsafe after new writes;
provide a minimum compatible server version and a disable-admission/roll-forward
runbook. Do not drop newly written campaign history to roll back application code.

Database restore may rewind attempt/generation state while devices have advanced.
Start restored services with admission disabled. Reconcile device journals and
installed digests, invalidate old online permits via an authority-epoch rotation,
and re-establish device generations before issuing work. Do not blindly replay
restored work queues or interpret restored counters as current physical state.

Add large PostgreSQL indexes with an online migration strategy compatible with
the migration runner; concurrent index creation cannot be wrapped in its normal
transaction. Validate constraints after preflight/backfill. Turso migration or
table rebuild requires a bounded edge maintenance window and backup. Do not edit
historical migration files to change installed databases.

## 12. Verification and release gates

| Test family | Required cases |
| --- | --- |
| Pure policy | Every allowed/forbidden transition; compatibility; wave rounding/denominators; retry budget; deadlines; stale health; time-zone/DST boundaries |
| Adapter contract | Tenant isolation, composite FKs, retained history, unique target/attempt IDs, atomic audit/outbox/state, idempotency mismatch, deterministic cursors |
| Concurrency | Two campaigns target one device; report versus admission; cancel versus permit; worker lease expiry/reclaim; stale worker completes late; quota race; archive versus activation |
| Protocol | Mixed v1/v2; duplicate/out-of-order reports; restart sequence persistence; unknown fields/versions; truncated/oversized payloads; reconnect without a hint; lost acknowledgement |
| Security | Wrong signer/product/device/tenant; tampered hash/size/manifest; stale root; revoked key; downgrade/freeze/replay; invalid time; certificate rotation/revocation; forged ingress identity |
| Download | Range honored/ignored; changed validator; expired URL during reconnect; full disk; slow reader; false length; object-store failure; corrupted cache and metadata |
| Native faults | Kill updater/app at each durable boundary; whole-machine reboot; candidate cannot execute; watchdog lost; journal/backup corruption; failed health; identity/data preservation |
| MCU hardware | Power cut/brownout across erase/write/boot/confirm; reset loop; NVS corruption; small partition; wrong board; signature failure; rollback to last known-good image |
| Load and isolation | 100k targets, 10k connected devices, 1k downloads, 10m events; reconnect bursts; noisy tenant; large offline population; connection-pool saturation |
| Operational recovery | API/worker/broker restart; DB failover; backup restore with stale commands; signer outage; revoked release; delayed health metrics; expiry across prolonged offline periods |
| Operator workflows | Preview exclusions, stale approvals, conflicting edits, partial cancellation, retry subsets, rollback compatibility, correct global counts and accessible controls |

Every platform needs evidence for download, verification, installation, reboot,
local health confirmation, rollback and eventual acknowledgement. Backend unit
tests and simulator runs alone cannot qualify physical flash recovery.

Expand the existing simulator with scripted OTA personalities: slow/offline,
duplicate reports, wrong sequence, corrupt download, crash-before-ack, successful
boot with later health regression and device-side cancellation refusal. Inject
the clock/jitter for fast deterministic policy tests; keep real-time soak tests
for reconnect and deadline behavior. Use external fault injection to interrupt
filesystem and hardware writes; tests should not merely call rollback directly.

Run the existing architecture/format/lint/API generation checks and applicable
native, PostgreSQL, Turso and client build matrices for each slice. Test firmware
transactions on actual PostgreSQL as well as Turso. Docs-only planning changes do
not require rebuilding or flashing devices.

Release-blocking invariants:

- Unauthorized or incompatible firmware never activates in qualification tests.
- At most one live install is authorized per device; stale reports/workers cannot
  advance or clear a newer attempt.
- No update is called successful before local boot validation; missed observations
  remain visible as uncertainty.
- No work or deployment audit is lost between durable admission and delivery.
- A paused/aborted campaign cannot admit new work; already issued permits and safe
  completion semantics match the documented bounds.
- Failed candidates recover to a qualified known-good image under the supported
  platform fault model, or surface a precise unsupported/unrecoverable condition.
- Tenant limits, query latency, queue lag and recovery behavior meet the named
  infrastructure profile without unbounded per-artifact application buffering.

## 13. Observability and runbooks

Expose counters for admitted/accepted/started/confirmed/rolled-back/rejected
attempts, transient/permanent error classes, timeout/uncertain devices, retries,
signature failures, stale events and admission denials. Add histograms for queue,
download, boot validation and total execution time; gauges for reservation usage,
worker lease backlog, aggregate freshness and gate waiting time.

Keep device/attempt IDs in structured logs, traces and queryable event tables;
do not use them as unbounded metric labels. Export tenant detail through scoped
queries or bounded aggregation rather than uncontrolled time-series cardinality.
Throttle progress reports (for example every five seconds or meaningful byte
increment) while persisting critical phase changes reliably. Aggregate reducers
must be idempotent and rebuildable from authoritative state.

Required runbooks: pause/abort, recover a failed wave, investigate uncertain devices,
revoke a release, rotate/recover signing and device keys, recover an expired-trust
device, repair storage, handle worker backlog, restore backup, identify an
incompatible data migration, and physically recover unsupported flash layouts.
Each runbook states which guarantees remain possible while dependencies are down.

## 14. Sequencing, effort and deferred work

Dependencies: P0 → P1; P2 and P3 build on P1 and share the protocol contract; P4
builds on release identity and device authentication; P5 builds on the durable
model; P6 requires P3/P5 and usable delivery; P7 begins with a P0 identity spike and
must finish before M3; P8 integrates all required packages. This is scheduling
guidance for the engineering team, not authorization to dispatch agents.

| Package | Rough engineering effort |
| --- | --- |
| P0 | 1–2 engineer-weeks |
| P1 | 2–3 engineer-weeks |
| P2 | 4–6 engineer-weeks |
| P3 | 4–6 engineer-weeks |
| P4 | 2–3 engineer-weeks |
| P5 | 3–5 engineer-weeks |
| P6 | 3–5 engineer-weeks |
| P7 | 3–5 engineer-weeks |
| P8 | 3–5 engineer-weeks |

Total planning range: 25–40 engineer-weeks before contingency. These are engineering
estimates, not a delivery promise; add allowance for verifier portability, hardware
access, signing infrastructure and integration with the existing refactor. With
three suitably experienced contributors, budget roughly four to six calendar
months including integration and field observation. One engineer should expect
roughly six to ten months or more. Re-estimate after P0 prototypes and the first
hardware qualification; reduce supported platform scope if a shorter launch is
required rather than omitting recovery or rollout safety.

M4 candidates, each with its own acceptance criteria:

- Binary deltas: exact source digest, bounded reconstruction resources, final full
  verification, power-failure recovery and full-image fallback.
- Gateway/edge caches: signed metadata remains end-to-end; local cache authorization,
  eviction, disconnected operation and stale-release policy are explicit.
- Package/OS/multi-component updates: installer-specific transactional recovery,
  dependency order and data compatibility; do not map them to binary replacement.
- Continuous targeting, recurring campaigns, site-aware scheduling and richer
  inventory selectors after snapshot campaigns are reliable.
- Approval integrations, audit export/retention policies and release provenance
  verification according to actual customer requirements.

The first implementation PR should be P0's baseline and missing PostgreSQL
attempt-concurrency tests, followed by the core firmware application/adapter slice.
Do not begin by raising the 500-device bulk limit or adding percentage controls to
the existing synchronous loop; those changes would not establish the required
durable admission and recovery behavior.

## 15. Reference basis

The architecture above is an Extrittio design proposal. Commercial capabilities
are used as benchmarks, not as evidence of capacity or correctness in this repo.

- [AWS IoT Jobs configuration](https://docs.aws.amazon.com/iot/latest/developerguide/jobs-configurations-details.html)
  documents rate-controlled rollout, scheduling, abort, timeout and retry controls.
- [Memfault OTA](https://docs.memfault.com/docs/platform/ota) documents cohort release
  activation and staged rollout behavior.
- [The Update Framework specification](https://theupdateframework.github.io/specification/latest/)
  defines signed metadata roles, trust rotation, version/expiry verification and
  defenses relevant to release authenticity and stale metadata.
- [Current Extrittio OTA protocol](../architecture/ota.md) records the existing
  native/ESP32 prerequisites and migration limitations.
- [Backend architecture plan](backend-crate-architecture-plan.md) and
  [persistence contract inventory](backend-persistence-contract-inventory.md)
  govern extraction boundaries and existing compatibility obligations.

Planning used the Postgres best-practices skill for bounded queue claims, short
transactions, consistent lock ordering, partial/composite indexes and keyset
pagination. These are design choices to verify against Extrittio's measured workload.
