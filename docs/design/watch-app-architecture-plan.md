# Apple Watch App Architecture and Implementation Plan

- **Status:** Proposed
- **Decision date:** 2026-09-02
- **Scope:** watchOS app, watch complications, iOS pairing bridge, and required backend seams
- **Minimum platform:** watchOS 26 with Xcode 26, matching the current iOS app baseline
- **Product design:** [Apple Watch App Product Design](watch-app-product-design.md)
- **Compatibility rule:** Reuse the current REST contract where it fits; add narrow contracts for watch authentication and contract-defined metric summaries

## 1. Executive decision

Build a hybrid native watchOS companion:

- The watch calls the Extrittio REST API directly over HTTPS when the configured
  server is reachable.
- A paired iPhone can proxy a small allowlist of reads and alert
  acknowledgements when the watch cannot reach a local or private server.
- The iPhone pushes the latest compact snapshot opportunistically through Watch
  Connectivity.
- The watch and its WidgetKit extension share an atomic, bounded snapshot for
  offline UI and complications.
- The backend issues a separate, revocable, least-privilege watch session. The
  iPhone's full user JWT is never copied to the watch.
- Backend data remains canonical. Watch Connectivity and the local snapshot are
  synchronization aids, not a second source of truth.

The transport order is **direct API, live iPhone proxy, cached snapshot**. A
read may fall through that order. A mutation may use only the direct API or a
live iPhone proxy and is never queued for later delivery.

## 2. Current-system evidence

The proposal builds on these checked-in behaviors:

- `apps/mobile-app-ios` already uses Swift 6, SwiftUI, `@Observable`,
  repository/use-case boundaries, `URLSession`, Keychain, and a disposable
  SwiftData cache.
- `APIConfiguration` resolves the user's server address under `/api/v1` and
  enforces HTTPS outside Debug.
- Native login requests a bearer JWT, stores it with
  `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, and validates the session with
  `GET /api/v1/auth/me`.
- User JWTs expire after 24 hours and carry tenant, user, permission version,
  scopes, and authentication epoch.
- Existing endpoints already cover dashboard device counts, alert summary and
  transitions, device inventory, and legacy latest telemetry.
- RBAC distinguishes `devices.read`, `alerts.read`, `alerts.manage`,
  `telemetry.read`, and `commands.send`.
- Device blueprints and materialized contracts are the forward-looking device
  model, while fixed telemetry fields remain a compatibility surface.
- The repository has no APNs device-registration or notification-delivery
  integration today.

The existing iPhone JWT must not be reused as a watch credential. It carries all
of the user's permissions, is not independently revocable by installation, and
would make a signed-out or replaced watch difficult to invalidate cleanly.

## 3. Goals

1. Remain useful with a reachable server, a reachable iPhone, or only cached
   state.
2. Preserve tenant isolation and server-side RBAC on every path.
3. Keep watch payloads small and background work power-aware.
4. Share domain and transport code without importing iOS-only UI, Bluetooth,
   MapKit, or SwiftData into watchOS.
5. Make credential revocation, permission changes, password changes, and device
   replacement fail closed.
6. Keep the Edge/self-hosted topology viable when the server is LAN-only.
7. Add watch verification to the generated Xcode project and repository CI.

## 4. Non-goals

The initial implementation does not:

- recreate every iOS screen on watchOS;
- log in with a password on the watch;
- make a LAN-only Edge server reachable over cellular;
- guarantee a fixed complication or background-refresh interval;
- queue commands or alert mutations for later execution;
- add real-time WebSocket or Zenoh connectivity to the watch;
- expose provisioning, configuration editing, firmware, OTA, or administration;
- make APNs a prerequisite for the first foreground-capable release.

## 5. Platform constraints

Watch Connectivity provides three distinct semantics that must not be blurred:

- `sendMessage` is immediate and requires the counterpart to be reachable;
- `updateApplicationContext` replaces older context and is appropriate for the
  newest snapshot;
- `transferUserInfo` is queued and eventually delivered, but not immediately.

The architecture uses `sendMessage` for confirmed pairing and live proxy
requests, `updateApplicationContext` for the latest read snapshot, and
`transferUserInfo` only for non-secret revocation or invalidation events.
Apple's
[WCSession documentation](https://developer.apple.com/documentation/watchconnectivity/wcsession)
explicitly notes that background transfers may be delayed.

watchOS background refresh is opportunistic, system-budgeted, and may be
throttled. SwiftUI background tasks are preferred on current watchOS. The app
therefore refreshes on every foreground activation, treats background work as a
freshness optimization, and always renders an age. See
[Using background tasks](https://developer.apple.com/documentation/watchkit/using-background-tasks).

## 6. Runtime topology

```text
                              HTTPS
Apple Watch app --------------------------------------> Extrittio REST API
      |                                                        ^
      | Watch Connectivity                                    |
      v                                                        | HTTPS
Extrittio iOS app ---------------------------------------------+
      |
      | latest application context
      v
Apple Watch app <---- App Group snapshot ----> Watch widget extension
```

### 6.1 Read path

1. Return a fresh in-memory value immediately when present.
2. Start a direct conditional REST request.
3. If direct networking fails and `WCSession.isReachable` is true, request one
   allowlisted resource from the iPhone.
4. If both transports fail, return the persisted snapshot with its timestamp.
5. Surface partial results. Failure to load alert summary must not discard
   device counts that loaded successfully.

### 6.2 Mutation path

1. Require a current local permission projection and connectivity.
2. Prefer the direct REST request with the watch access token.
3. Fall back only to an immediate iPhone proxy request.
4. On a timeout or lost response, fetch the canonical resource and reconcile.
5. Never store a pending mutation in Watch Connectivity, the snapshot, or a
   background queue.

The iPhone proxy accepts typed operations such as `loadOverview` and
`acknowledgeAlert(id:)`; it never accepts an arbitrary URL, HTTP verb, header,
or body from the watch.

The proxy uses the iPhone's current user session and preserves its existing
`kSecAttrAccessibleWhenUnlockedThisDeviceOnly` policy. A locked iPhone may be
reachable through Watch Connectivity while its credential is unavailable; in
that case the proxy fails cleanly and the watch falls back to its snapshot. Do
not weaken the full iPhone credential's Keychain accessibility solely to make
proxying more available.

## 7. Target package and dependency boundaries

Introduce a local Swift package so iOS and watchOS share stable, compile-time
checked code instead of linking selected folders into multiple targets.

```text
ExtrittioClientCore                         Foundation only
├── Models                                 User, Device, Alert, summaries
├── Authorization                          PermissionKey and capability checks
├── HTTP                                   endpoint building, API errors, codec
├── Operations                             repository protocols and use cases
├── WatchContracts                         snapshots and versioned bridge messages
├── Formatting                             dates, status, severity, fleet health
└── Routing                                deep links and Handoff route payloads

Extrittio iOS app                          iOS frameworks + ExtrittioClientCore
├── iOS presentation and view models
├── SwiftData cache
├── login Keychain
├── Bluetooth provisioning
└── WatchPairingBridge

Extrittio Watch app                        SwiftUI/WatchKit + ExtrittioClientCore
├── Overview, alerts, devices
├── WatchSessionStore
├── WatchAPIClient
├── PhoneProxyClient
└── WatchSnapshotStore

Extrittio Watch widgets                    WidgetKit + snapshot DTOs only
```

`ExtrittioClientCore` supports iOS 26 and watchOS 26 and imports Foundation
only. It must not import SwiftUI, UIKit, WatchKit, WidgetKit, SwiftData,
Security, MapKit, CoreBluetooth, or `os`.

The current `APIClient` reads Keychain directly. Before extraction, replace that
global dependency with a `CredentialProvider: Sendable` protocol. iOS and
watchOS then provide separate Keychain-backed implementations. The shared HTTP
client owns request validation, JSON coding, typed errors, and unauthorized
callbacks but not credential persistence.

## 8. SwiftUI state and navigation

The watch root owns one `@State` reference to an `@Observable @MainActor`
`WatchAppModel`. Shared services enter the environment at the app root. Feature
models are created by the composition root and passed explicitly to their root
views.

```text
WatchApp
└── WatchAppModel                          pairing and authenticated app state
    ├── OverviewModel                      independent partial section states
    ├── AlertsModel                        list, filter, mutation state
    └── DevicesModel                       list and selected-device state
```

Use value enums rather than Boolean flags:

- `WatchAuthenticationState`: `unpaired`, `pairing`, `ready`, `expired`;
- `WatchRoute`: `alerts`, `alert(id)`, `devices`, `device(id)`, `connection`;
- `LoadState<Value>`: `idle`, `loading`, `value(value, freshness)`, `failed`;
- `Freshness`: `live(date)`, `cached(date)`, `expired(date)`.

A single `NavigationStack` owns `[WatchRoute]`. Views start cancellable work
with `.task` or `.task(id:)`; no service call originates from `body`.

## 9. Watch session authentication

### 9.1 Required backend contract

Add a tenant-owned companion-session aggregate with these routes:

| Route | Authentication | Purpose |
| --- | --- | --- |
| `POST /api/v1/auth/watch-sessions` | Current user JWT/cookie | Create a session and return its secret once |
| `GET /api/v1/auth/watch-sessions` | Current user JWT/cookie | Show the user's paired watches in iOS settings |
| `POST /api/v1/auth/watch-sessions/{id}/token` | Session ID + rotating secret | Exchange for a short-lived access JWT |
| `DELETE /api/v1/auth/watch-sessions/{id}` | Current user JWT/cookie | Revoke a lost or replaced watch |

The creation request contains an opaque watch installation ID and a
user-visible label. The response contains the session ID, a random one-time
secret, allowed scopes, renewal expiry, and server configuration. Secrets are
returned only at creation or rotation and are never logged.

The persistence record contains:

- tenant ID and user ID;
- watch session ID and installation ID;
- hash of the current rotating secret;
- granted scopes;
- bound permission version and authentication epoch;
- creation, last-used, renewal-expiry, absolute-expiry, and revocation times.

### 9.2 Credential policy

- The initial scope allowlist is `devices.read`, `alerts.read`,
  `alerts.manage`, and `telemetry.read`.
- Granted scopes are the intersection of that allowlist and the user's current
  effective permissions.
- A watch access JWT lasts 15 minutes.
- The rotating renewal secret expires after 30 days of inactivity and after 90
  days absolutely. These are server constants and can become configuration only
  if deployments demonstrate a real need.
- Every exchange rechecks user existence, tenant, authentication epoch,
  permission version, revocation, and expiry, then rotates the renewal secret.
- A permission or password change invalidates the old exchange state. The iOS
  app must reauthorize the watch with the new projection.
- Access claims include `client_kind = watch` and `watch_session_id` for rate
  limiting and audit attribution.

Core owns the watch-session entity, authorization, lifecycle, and repository
port. PostgreSQL and Turso implement the same contract. The backend host owns
HTTP DTOs, JWT creation, secret hashing, rate limiting, and request audit.

### 9.3 Provisioning sequence

```text
Watch                    iPhone                         Backend
  | pairing request         |                              |
  |------------------------>| authenticated create        |
  |                         |----------------------------->|
  |                         | session + one-time secret    |
  | credential bundle      |<-----------------------------|
  |<------------------------|                              |
  | store in Keychain       |                              |
  | token exchange --------------------------------------->|
  | short-lived access JWT <-------------------------------|
```

The iPhone shows the server, account, requested capabilities, and watch label
before creating the session. It delivers the one-time credential bundle only
through an immediate `sendMessageData` exchange while both apps are active. If
delivery cannot be confirmed, the iPhone revokes that session and starts a new
pairing attempt; it does not persist or queue the secret. The watch stores the
renewal secret with
`kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, which Apple recommends for
background access and which does not migrate to a different device. See the
[Keychain accessibility documentation](https://developer.apple.com/documentation/security/ksecattraccessibleafterfirstunlockthisdeviceonly).

Logout or server replacement sends an immediate revocation message when
possible, updates application context, and deletes local watch state. Backend
revocation remains authoritative if the counterpart is unreachable.

## 10. REST contract usage

### 10.1 Existing routes used by the first release

| Capability | Route | Required permission |
| --- | --- | --- |
| Device counts | `GET /api/v1/dashboard/stats` | `devices.read` |
| Alert counts | `GET /api/v1/alerts/summary` | `alerts.read` |
| Alert list/detail | `GET /api/v1/alerts`, `GET /api/v1/alerts/{id}` | `alerts.read` |
| Acknowledge | `PUT /api/v1/alerts/{id}/acknowledge` | `alerts.manage` |
| Device list/detail | `GET /api/v1/devices`, `GET /api/v1/devices/{id}` | `devices.read` |
| Legacy latest sample | `GET /api/v1/devices/{id}/telemetry/latest` | `telemetry.read` |

The watch should begin with these domain routes in parallel rather than adding
a watch-specific backend-for-frontend endpoint. Payload measurements during
implementation determine whether an aggregated overview route is justified.

### 10.2 Required metric compatibility route

Add `GET /api/v1/devices/{id}/metrics/latest`. It returns the newest value for
each contract-defined metric selected for summary presentation, bounded to a
small server maximum. It uses the assigned materialized contract, preserves
typed values and units, and requires `telemetry.read`.

This avoids either fan-out through repeated `/metrics` queries or incorrectly
assuming that every blueprint uses the legacy temperature, humidity, and
battery columns.

The response should carry contract revision/hash so clients can invalidate
presentation metadata after reassignment.

### 10.3 HTTP efficiency

- Add `ETag` and honor `If-None-Match` on watch read routes where practical.
- Clamp alert and device page sizes to watch needs.
- Use gzip or Brotli only when already supported by the deployment ingress;
  do not add a watch-only compression format.
- Do not place credentials, tenant identifiers, or sensitive values in URLs.

### 10.4 Continue on iPhone

Watch detail views publish a versioned `NSUserActivity` containing only the
resource kind and opaque resource ID. The iOS target declares and restores that
activity, maps it through `AppNavigationRouter`, and re-fetches the resource
under the current iPhone session. No credential, tenant, cached payload, or
telemetry value enters the activity.

Handoff offers continuity but does not let the watch force-launch the iPhone
app. If Handoff is unavailable, the watch may send the same route as a
non-secret invalidation through Watch Connectivity so the iOS app can offer it
the next time it becomes active. Apple's
[Handoff documentation](https://developer.apple.com/documentation/foundation/implementing-handoff-in-your-app)
confirms that watchOS can originate activities for other devices.

## 11. Watch Connectivity protocol

Every message is a property-list-safe dictionary containing a binary encoded
payload and a small envelope:

```text
schema_version
message_id
kind
created_at
payload
```

Version 1 message kinds are:

- `pairingRequest`
- `credentialBundle`
- `revokeSession`
- `latestSnapshot`
- `readRequest` / `readResponse`
- `acknowledgeAlertRequest` / `acknowledgeAlertResponse`
- `invalidateResource`

Decode unknown versions and kinds as unsupported without crashing. Cap message
and snapshot sizes before decoding. The iPhone maps each request to an explicit
use case, checks its current user capability, and lets the backend authorize
again.

Use `updateApplicationContext` for `latestSnapshot`, because only the newest
snapshot matters. Use `sendMessageData` for the confirmed credential handoff
and `sendMessage` for live proxy pairs. Use `transferUserInfo` only for
non-secret revocation or invalidation delivery when eventual arrival is useful.
Credentials never enter application context, snapshot files, or queued
transfers.

## 12. Snapshot and cache design

`WatchSnapshot` is a versioned, `Codable`, `Sendable` value:

```text
schemaVersion, generatedAt, serverID, userID, permissionVersion
overview?: WatchOverview
alerts?: [WatchAlertSummary]
devices?: [WatchDeviceSummary]
```

Each section is optional and has its own fetched time and error metadata. The
snapshot is bounded to the overview, up to 20 attention alerts, and up to 50
device summaries. Detail payloads use a separate small LRU with at most 20
entries.

Persist JSON with atomic replacement in the watch App Group container. This
data is disposable and non-authoritative, so SwiftData is unnecessary. On
server, user, or permission-version mismatch, delete it before rendering.

Freshness policy:

- under two minutes: no age label in the foreground;
- two minutes to 24 hours: show “Updated … ago”;
- over 24 hours: show an explicit stale treatment and disable all mutations.

The watch writes a redacted `WidgetSnapshot` to the shared container after a
successful refresh and calls `WidgetCenter.reloadTimelines(ofKind:)`. The
widget extension never opens the Keychain or performs arbitrary REST work.

## 13. Background refresh and complications

The app refreshes on foreground activation and schedules a best-effort app
refresh around 15 minutes later when a complication is active. Each successful
background run:

1. renews the access JWT only when necessary;
2. uses conditional requests for dashboard and alert summary;
3. atomically stores the result;
4. reloads relevant WidgetKit timelines;
5. schedules the next preferred refresh.

The system may defer or omit any run. No correctness rule depends on it.
Watch-Connectivity background delivery is handled through SwiftUI's
`.backgroundTask(.watchConnectivity)` path, and URL-session completion through
the matching background task API when a background session is introduced.

## 14. Notification architecture after the first release

APNs support requires a separate backend integration:

- the watch registers an APNs token against its watch session;
- registration records environment, topics, user/tenant, preferences, and
  last-seen time;
- alert creation enqueues a durable notification action;
- a host-owned APNs adapter sends sanitized payloads and records delivery
  outcomes without logging tokens;
- invalid-token responses deactivate the registration;
- the notification contains identifiers and display text, while the app still
  fetches canonical alert state before a mutation.

Direct watch notifications are preferred to relying only on iPhone forwarding,
because direct targeting supports independent use and unambiguous action
handling. Notification delivery is best effort; it does not replace alert
persistence or the rules engine.

## 15. File layout

```text
apps/mobile-app-ios/
├── Packages/ExtrittioClientCore/
│   ├── Package.swift
│   ├── Sources/ExtrittioClientCore/
│   └── Tests/ExtrittioClientCoreTests/
├── Extrittio/                         existing iOS target
│   └── Data/WatchConnectivity/
├── ExtrittioWatch/
│   ├── App/
│   ├── Features/Overview/
│   ├── Features/Alerts/
│   ├── Features/Devices/
│   ├── Infrastructure/Auth/
│   ├── Infrastructure/Connectivity/
│   ├── Infrastructure/Networking/
│   ├── Infrastructure/Storage/
│   └── Resources/
├── ExtrittioWatchWidgets/
├── ExtrittioWatchTests/
└── Shared/                            deep-link vocabulary only
```

XcodeGen remains the project source of truth. `project.yml` adds the watch app,
watch widget, shared local package, test target, App Group, Watch Connectivity,
and required embedding relationships. Generated `.xcodeproj` files remain
ignored.

## 16. Build and verification changes

- Extend `cargo xtask ios build` or add an explicit watch build that targets
  `generic/platform=watchOS Simulator`.
- Add watch unit tests with a named watchOS simulator destination.
- Include watch and package paths in SwiftLint and SwiftFormat.
- Extend the module check to enforce Foundation-only imports in
  `ExtrittioClientCore`.
- Add fixtures that decode current OpenAPI JSON shapes into shared Swift models.
- Keep an iOS build in CI so extraction cannot silently break the phone app.

Required test layers:

| Layer | Required proof |
| --- | --- |
| Core Swift package | DTO decoding, permissions, health classification, freshness, bridge-version decoding |
| Watch unit | state transitions, transport fallback, no offline mutations, route parsing |
| iOS unit | pairing, allowlisted proxy operations, logout/server-change revocation |
| Backend core | scope intersection, expiry, epoch/version invalidation, revocation |
| Adapter contract | tenant isolation, secret rotation atomicity, uniqueness, expiry semantics |
| HTTP integration | session lifecycle, watch claims, rate limit/audit identity, ambiguous acknowledgement reconciliation |
| Widget | every family, stale rendering, redaction, deep links |
| UI | loading, empty, partial, cached, unauthorized, expired, offline, large text, VoiceOver |

## 17. Observability and security

Record low-cardinality metrics for direct/proxy/cache read outcomes, token
exchange outcomes, background refresh age, snapshot decode failures, and alert
acknowledgement outcomes. Logs may include watch session ID, route class, and
status, but never access JWTs, renewal secrets, APNs tokens, raw payloads, or
customer telemetry values.

Rate limiting keys include tenant, user, and watch session. Audit events use the
authenticated tenant and identify the client as watch. The iPhone proxy does
not accept tenant input from the watch and uses its mapped authenticated user.

Credential bundles, snapshots, and bridge messages are size-bounded. Snapshot
files explicitly use complete-until-first-user-authentication file protection
inside the App Group container. Renewal secrets use Keychain and do not migrate
to replacement hardware.

## 18. Work packages

### P0 — Shared contracts and build skeleton

- Extract `ExtrittioClientCore` without changing iOS behavior.
- Inject `CredentialProvider` into the shared HTTP client.
- Define snapshot, bridge envelope, navigation, and permission contracts.
- Add empty watch app, widget, tests, XcodeGen targets, and CI builds.

Exit: the unchanged iOS app and an empty watch shell build from generated
projects; package boundaries fail in CI when violated.

### P1 — Backend watch sessions

- Add core lifecycle and repository port.
- Implement PostgreSQL and Turso migrations/adapters plus shared contract tests.
- Add HTTP routes, secret hashing/rotation, watch JWT claims, rate limits, and
  audit attribution.
- Add iOS create/list/revoke settings and the provisioning bridge.

Exit: a paired watch can obtain a least-privilege access JWT, renew it, and be
revoked; user security changes fail closed.

### P2 — Foreground watch experience

- Implement direct REST, live iPhone proxy, snapshot fallback, and freshness.
- Build Overview, Alerts, Alert detail, Devices, and Device detail.
- Implement acknowledgement reconciliation and iPhone handoff.
- Add the generic latest-metrics route.

Exit: all first-release foreground acceptance criteria pass with production and
Edge topology tests.

### P3 — Complications and background refresh

- Add WidgetKit families and deep links.
- Add redacted App Group snapshot and timeline reloads.
- Implement best-effort background and Watch Connectivity task handlers.
- Measure payload, energy, and stale behavior on physical hardware.

Exit: complications remain honest under throttling, offline use, permission
changes, and session expiry.

### P4 — Direct alert notifications

- Add APNs registration, preferences, durable delivery action, and adapter.
- Add watch notification categories and acknowledgement handling.
- Add invalid-token cleanup, delivery metrics, and deployment documentation.

Exit: canonical alert state remains correct across duplicate, delayed, tapped,
and failed notifications.

## 19. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Self-hosted server is unreachable from watch cellular | Live iPhone proxy and cached snapshot; document that independent access needs routable HTTPS or VPN |
| iPhone is reachable but its locked Keychain is unavailable | Preserve the existing iPhone credential policy and fall back to cached state |
| Background work is throttled | Foreground refresh, timestamp every snapshot, never promise real time |
| Full iPhone permissions leak to watch | Separate scope-intersected session and short-lived access JWT |
| Permission/password change leaves stale access | Bind exchange to permission version and auth epoch; fail closed |
| Watch/iPhone message schemas drift | Versioned bounded envelope with compatibility tests |
| Blueprint devices show misleading legacy metrics | Add contract-aware latest metric route; omit unsupported metrics |
| Ambiguous mutation response causes duplicate work | Re-fetch canonical alert and treat target state as success |
| Widget exposes sensitive details | Store only counts and health state in the widget snapshot |
| Shared package becomes a dumping ground | Foundation-only dependency rule and explicit package folders |

## 20. Definition of done

The architecture is implemented when:

1. iOS, watchOS, widget, shared-package, backend, PostgreSQL, and Turso checks
   pass in CI.
2. The watch never stores the iPhone user JWT or an Extrittio password.
3. Direct, proxy, cached, partial, unauthorized, and expired paths are tested.
4. All watch reads and mutations preserve tenant and RBAC enforcement.
5. The watch remains useful when the iPhone is absent and the server is
   reachable, and remains honestly read-only when neither transport is live.
6. Contract-defined device metrics work without assuming legacy fields.
7. Complications are bounded, redacted, deep-linked, and freshness-aware.
8. Documentation covers setup, revocation, self-hosted reachability, security,
   and operational monitoring.
9. The product acceptance criteria in the companion design document pass on
   small and large supported Apple Watch sizes with VoiceOver and large text.
