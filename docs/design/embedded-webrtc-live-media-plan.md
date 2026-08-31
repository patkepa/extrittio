# Embedded WebRTC Live Media Implementation Plan

- **Status:** Proposed
- **Target branch:** `feat/webrtc-video`
- **Decision date:** 2026-08-31
- **Primary packages:** `extrittio-backend`, `extrittio-common`,
  `extrittio-device-contract`, the React console, the Rust device runtime, and
  the iOS app
- **Deployment constraint:** one Extrittio process; no TURN, STUN, SFU, media
  gateway, transcoder, Redis, or other separately operated service

## 1. Executive decision

Implement live, receive-only device media as an optional embedded WebRTC relay
inside the Extrittio backend.

```text
device publisher
  -> authenticated offer/answer signaling over the existing Zenoh session
  -> WebRTC audio/video peer connection to Extrittio MediaRuntime
  -> encoded RTP/RTCP fan-out in memory, without decode or transcode
  -> receive-only WebRTC peer connection
  -> React console or iOS embedded player
```

Extrittio terminates both WebRTC connections. Devices and viewers therefore
make outbound connections to one known server instead of trying to reach each
other through arbitrary NATs. The deployment adds UDP port exposure, but it
does not add another process or managed dependency.

The first release deliberately supports one live publisher per declared media
stream, H.264 video, optional Opus audio, a bounded number of viewers, and
ephemeral sessions. It does not record media or persist SDP, ICE candidates,
RTP, RTCP, frames, or viewer session secrets.

## 2. Why this shape fits the repository

The current device path already provides provisioned identity, optional
Zenoh mTLS, per-device topic ACLs, bounded payload handling, and supervised
subscribers. Browser requests already carry tenant-scoped authentication, and
the iOS application already carries an API bearer token. Reusing those paths
avoids a second device credential and a second signaling server.

Media must not enter the existing contract-event pipeline. Contract events are
validated, persisted, indexed, and consumed by analytics and rules. Live media
has different loss, ordering, lifetime, and backpressure semantics. Only the
media capability belongs in the materialized device contract; signaling and
packets stay in the runtime host.

The implementation follows the current modular-monolith boundary:

- `extrittio-device-contract` owns declarative media capability and validation;
- `extrittio-common` owns versioned device signaling messages and topic helpers;
- `extrittio-backend-core` owns the `media.view` permission key only;
- `extrittio-backend` owns WebRTC, UDP sockets, session orchestration, Axum
  translation, Zenoh translation, readiness, metrics, and shutdown;
- database adapters only receive the permission-data migration; they do not
  store media sessions;
- clients own capture or rendering and SDP construction.

Do not add WebRTC to `TransportProtocol`. That enum describes routed device
messages with delivery and ordering semantics. Add media as a sibling of
`streams`, `commands`, and `firmware` in a blueprint.

## 3. Supported operating envelope

The initial release has an explicit support boundary.

### 3.1 Supported

- production server with a stable public IPv4 address and a directly mapped UDP
  range;
- Extrittio Edge and viewers on the same routed LAN;
- browser playback in supported desktop and mobile browsers;
- one publishing connection per device media key;
- up to a configured small viewer count per stream;
- video-only or video-plus-audio feeds;
- a single Extrittio process using either PostgreSQL or Turso.

### 3.2 Not supported initially

- a server hidden behind a TCP-only proxy, CDN, or load balancer;
- a server behind NAT without an explicit UDP port mapping;
- TURN fallback for networks that block outbound UDP;
- horizontal replicas or moving a live session between processes;
- browser-to-device peer-to-peer media;
- talkback, browser publishing, or device control over a data channel;
- transcoding, mixing, overlays, thumbnails, recording, playback, or export;
- simulcast, SVC, adaptive layer selection, or more than one quality rendition;
- arbitrary codecs or H.265-only devices.

If deployment evidence later shows that UDP blocking is material, TURN is a
separate follow-up decision. Do not disguise a best-effort direct path as a
reliable supported path.

## 4. Version-one media contract

Extend `BlueprintSpec` with a defaulted `media: Vec<MediaDefinition>`. Keep the
serialized empty form omitted where possible so blueprints without media
retain their existing canonical representation.

Example authoring document:

```yaml
media:
  - key: main-camera
    label: Main camera
    video:
      codec: h264
      profile: constrained_baseline
      maxWidth: 1280
      maxHeight: 720
      maxFrameRate: 30
      maxBitrateKbps: 2000
    audio:
      codec: opus
      channels: 1
      sampleRateHz: 48000
presentation:
  tabs: [overview, media, telemetry, commands]
```

### 4.1 Model and validation

Add these bounded concepts under `crates/device-contract/src/model.rs`:

- `MediaDefinition { key, label, video, audio }`;
- `VideoMediaDefinition` with the single initial codec/profile combination;
- `AudioMediaDefinition` with the single initial codec;
- `PresentationTab::Media`.

Validation in `validation.rs` must enforce:

- media keys use the existing bounded key syntax and are unique;
- at least one of audio and video exists;
- width and height are `1..=3840` and `1..=2160`;
- frame rate is `1..=60`;
- video bitrate is `64..=10_000` Kbit/s;
- Opus is mono or stereo at 48 kHz in version one;
- `presentation.tabs` may contain `media` only when at least one media source
  exists;
- no unknown codec/profile string reaches the runtime.

Compilation in `compiler.rs` produces a key-addressable media map in the
materialized contract. Compatibility comparison treats removal of a media key,
removal of a track, a codec/profile change, or reduction below a previously
declared maximum as breaking. Increasing a maximum or adding a new key is
non-breaking.

Add model, validation, canonicalization, deterministic hash, and compatibility
tests. Existing no-media fixtures must continue to validate.

## 5. Device signaling protocol

Use the existing authenticated Zenoh session only for signaling. Media packets
never pass through Zenoh.

### 5.1 Topics

Add helpers and parsers in `crates/common/src/topics.rs`:

```text
extrittio/devices/{device_id}/media/signal/up
extrittio/devices/{device_id}/media/signal/down
```

The device may publish only `up` and subscribe only to its own `down`. The
backend does the reverse. Extend the generated Zenoh ACL in
`crates/backend/src/init.rs` and its exact-policy tests. The backend declares a
single wildcard upstream subscriber in
`crates/backend/src/zenoh_handler/subscriber.rs`.

### 5.2 Wire message

Add a versioned Protobuf message rather than placing opaque ad hoc JSON on the
wire:

```proto
message DeviceMediaSignal {
  uint32 protocol_version = 1;
  string device_id = 2;
  string stream_key = 3;
  string session_id = 4;
  string contract_hash = 5;
  int64 sent_at_ms = 6;
  oneof payload {
    MediaOffer offer = 10;
    MediaAnswer answer = 11;
    MediaStop stop = 12;
    MediaError error = 13;
  }
}
```

Version one uses non-trickle ICE. The device waits for ICE gathering to finish
and publishes one complete offer; the backend returns one complete answer. A
new random session ID replaces the previous publisher for the same device and
stream. A repeated identical message is idempotent. Keep a bounded tombstone
set for superseded session IDs so a delayed signal cannot reactivate an older
publisher after replacement.

The handler must:

- enforce the existing Zenoh maximum before decode and a 64 KiB SDP limit after
  decode;
- cross-check topic identity, payload device ID, provisioned identity, assigned
  contract hash, and declared stream key;
- accept only an SDP offer on the upstream path and an SDP answer on the
  downstream path;
- reject unsupported protocol versions and malformed or unexpected SDP types;
- avoid logging SDP or ICE candidates;
- count accepted and rejected signals by bounded reason labels.

Update Rust wire-compatibility fixtures and generated C/nanopb artifacts. Add
topic round-trip and ACL tests. The Rust device runtime gains typed helpers,
but media capture remains an application concern rather than part of the
platform-neutral sensor SDK.

## 6. Embedded `MediaRuntime`

Add an optional `media` Cargo feature to `extrittio-backend` and
`apps/extrittio`. The dependency on `webrtc-rs` is optional and contained in a
host-owned module such as `crates/backend/src/media`. Production and Edge
release features include it; runtime configuration keeps it disabled until an
operator supplies valid media settings.

### 6.1 Runtime state

Construct one `Arc<MediaRuntime>` during boot and place the narrow handle in
`AppStateInput`/`AppState`. Do not expose WebRTC library types to backend core,
HTTP DTOs, device contracts, or persistence adapters.

```text
MediaRuntime
  configuration and UDP allocator
  publisher registry: (tenant, device, stream) -> Publisher
  viewer registry: session ID -> Viewer
  shutdown token
  metrics

Publisher
  device/session/contract identity
  peer connection
  negotiated audio/video metadata
  bounded RTP fan-out senders
  state timestamps

Viewer
  tenant/device/stream/user identity
  peer connection
  outbound tracks and forwarding tasks
  creation/expiry timestamps
```

Use random, unguessable UUIDs for runtime identifiers. Registry lookup always
includes tenant ownership; never authorize from a bare session ID.

### 6.2 Publisher establishment

On an accepted device offer:

1. reserve `(tenant, device, stream)` without holding a lock across `await`;
2. create a peer connection restricted to contract-declared codecs;
3. set the remote offer and create an answer;
4. wait for local ICE gathering with a bounded timeout;
5. publish the answer on the device's downstream Zenoh topic;
6. promote the connection to the active publisher only after connection and
   track validation;
7. atomically close the prior publisher and all of its viewers;
8. remove the reservation on every error path.

The runtime advertises the configured server address and allocates only from
the configured UDP range. It does not contact a public STUN service.

### 6.3 RTP/RTCP forwarding

Read incoming encoded RTP and fan it out to bounded per-viewer tasks. Never
decode a frame. A lagging viewer skips stale packets or is disconnected; it
must not backpressure the device or grow memory without bound.

Each viewer gets stable outbound SSRC/track state and the WebRTC library's
default RTP/RTCP interceptors. Drain sender RTCP continuously. Forward PLI/FIR
keyframe requests to the publisher and use the supported NACK/RTX path where
the negotiated codec permits it. The device integration contract additionally
requires a regular H.264 IDR interval so a new viewer does not wait indefinitely
for a decodable frame.

Reject a publisher whose negotiated codecs, media count, or parameters exceed
its materialized contract. Reject a viewer offer that cannot negotiate the
active publisher's codec. There is no transcoding fallback.

### 6.4 Lifecycle and supervision

- close viewers when their UI disconnects, their session expires, their
  publisher is replaced, or Extrittio shuts down;
- close a publisher on peer failure, explicit device stop, contract change, or
  device deletion;
- use timeouts around ICE gathering, DTLS connection, and close operations;
- register media signaling and media runtime readiness with the existing
  worker/readiness registry;
- make shutdown idempotent and await forwarding tasks within the application's
  shutdown budget;
- keep all state ephemeral across restarts.

## 7. Viewer HTTP API and authorization

Add a vertical `media` HTTP domain and merge its router in
`crates/backend/src/api/mod.rs`.

### 7.1 Endpoints

```text
GET    /api/v1/devices/{device_id}/media
POST   /api/v1/devices/{device_id}/media/{stream_key}/sessions
DELETE /api/v1/media/sessions/{session_id}
POST   /api/v1/devices/{device_id}/media/{stream_key}/tickets
POST   /api/v1/media/ticket-sessions
```

`GET` returns declared streams, publisher availability, negotiated track kinds,
viewer count, and last state change. It never returns addresses or SDP.

The authenticated session-creation request contains the viewer's complete
receive-only SDP offer. The `201 Created` response contains the random session
ID, complete answer, and expiry. `DELETE` is idempotent and returns `204`.

Tickets are only for the native iOS embedded player described below. A ticket
is 256 bits of randomness, single-use, in-memory, scoped to tenant/user/device/
stream, and expires after at most 30 seconds. Redeeming it creates a normal
viewer session; it cannot call any other API. The redemption request carries
the ticket and SDP offer in its body, never in a request target, header, or
server log.

Use these stable error semantics:

- `400` malformed or non-receive-only SDP;
- `403` missing permission or cross-tenant access;
- `404` unknown device, undeclared stream, session, or ticket;
- `409` no active publisher or incompatible codec;
- `429` per-user, per-stream, or global session limit reached;
- `503` media disabled or runtime unavailable.

### 7.2 RBAC and audit

Add `Permission::ReadMedia` with the key `media.view` to the stable permission
catalog and both frontend permission models. It is independent of
`telemetry.read`; viewing a camera is a distinct privacy grant.

Add PostgreSQL and Turso data migrations that grant `media.view` to existing
built-in owner roles using idempotent inserts. Do not grant it to arbitrary
custom roles. New owner bootstrap continues to use `Permission::all()`.

Record bounded audit metadata when a viewer session is created and closed:
actor, tenant, device, stream key, session ID, outcome, and reason. Never record
SDP, candidates, IP addresses, codec parameter blobs, or packet statistics in
the audit payload.

Register DTOs, routes, errors, and the `media` OpenAPI tag, regenerate
`api/openapi.json`, and regenerate `apps/frontend/src/types/openapi.ts`.

## 8. React console

Add a Live Media tab to the existing device detail screen only when:

- the assigned contract declares at least one media entry; and
- the user has `media.view`.

The client flow is:

1. load media status;
2. create an `RTCPeerConnection` with no external ICE servers;
3. add receive-only video and optional audio transceivers;
4. create/set the local offer and wait for ICE gathering completion;
5. create the authenticated viewer session through the API;
6. set the remote answer;
7. attach tracks to one `MediaStream` and assign it to `video.srcObject`;
8. delete the session and close the peer on unmount, tab change, retry, logout,
   or terminal connection failure.

Add `media-tab.tsx`, a focused `use-media-session` hook, API functions, query
keys, styles, and tests. The hook owns the peer connection and must not place it
in TanStack Query state.

UX requirements:

- start video muted to satisfy autoplay policy and require an explicit audio
  enable action;
- use `playsInline` and preserve aspect ratio;
- show distinct offline, connecting, live, reconnecting, unsupported-codec,
  permission, limit, and failed states;
- provide Stop/Retry controls and a visible audio state;
- stop media when the tab unmounts rather than streaming invisibly;
- announce state transitions and make controls keyboard accessible;
- display no stale frame after session teardown;
- avoid automatic infinite reconnect loops; use bounded exponential retry only
  for transient connection failures.

Unit tests use a small `RTCPeerConnection` facade/fake. Add browser integration
coverage with a deterministic test publisher; do not depend on a physical
camera in CI.

## 9. iOS application

The current iOS application intentionally has no third-party runtime
dependencies. Preserve that property for version one by using a narrowly scoped
`WKWebView` player backed by the same tested web WebRTC code instead of adding a
binary libwebrtc distribution.

Add a first-party `/media/embed` frontend route that renders only the player,
accepts a one-time ticket from the URL fragment, immediately clears the
fragment, redeems the ticket, and reports state through a constrained WebKit
message handler. The fragment is not sent in the initial HTTP request or
referrer. Never place the API bearer token in the web view.

The Swift flow is:

1. `MediaRepository` requests a one-time ticket through the existing
   authenticated `APIClient`;
2. `DeviceLiveMediaTab` constructs an ephemeral `WKWebView` with a
   non-persistent website data store;
3. it loads `https://server/media/embed#ticket=...`;
4. a coordinator maps allow-listed player messages to Swift view state;
5. navigation is restricted to the configured Extrittio origin;
6. disappearance destroys the web view and therefore the viewer connection.

Add the Live tab to `GeneralSubTab`, guarded by `media.view` and contract
capability. Do not cache tickets, SDP, frames, or live-player state in SwiftData.
Disable screenshots or screen recording only if a later product requirement
explicitly calls for it; do not claim DRM or content protection in version one.

If native rendering, Picture in Picture, or background audio becomes a product
requirement, evaluate a pinned native WebRTC XCFramework in a separate design
decision. It is not required to ship receive-only live viewing.

## 10. Configuration and deployment

Add validated configuration with conservative defaults:

```text
EXTRITTIO_MEDIA_ENABLED=false
EXTRITTIO_MEDIA_LISTEN_HOST=0.0.0.0
EXTRITTIO_MEDIA_PUBLIC_IP=
EXTRITTIO_MEDIA_UDP_MIN=50000
EXTRITTIO_MEDIA_UDP_MAX=50031
EXTRITTIO_MEDIA_MAX_VIEWERS_PER_STREAM=4
EXTRITTIO_MEDIA_MAX_VIEWERS_TOTAL=32
EXTRITTIO_MEDIA_SESSION_TTL_SECS=900
EXTRITTIO_MEDIA_ICE_GATHER_TIMEOUT_SECS=10
EXTRITTIO_MEDIA_CONNECT_TIMEOUT_SECS=15
```

When media is enabled, require a literal, non-unspecified advertised IP outside
local-development mode. Validate ordered, nonzero ports, cap the range size,
cap viewer limits, and reject unsafe timeouts. Never silently fall back to a
third-party STUN address.

Update:

- `.env.example` and deployment-specific environment examples;
- production and development Compose with a fixed UDP range mapping;
- Docker and Debian/Edge builds so the selected release features include media;
- Docker, Edge, and OpenThread deployment documentation with UDP routing and
  firewall requirements;
- readiness documentation to explain the enabled/disabled states;
- upgrade notes stating that nginx proxies HTTP only and WebRTC UDP bypasses it.

Edge media is LAN-only in version one. Thread-only sleepy/end devices are not
assumed capable of carrying the video bitrate; a camera device must have an
appropriate IP link and hardware encoder.

## 11. Security, privacy, and resource controls

Treat live media as a sensitive capability.

- require HTTPS for production viewer signaling and retain DTLS-SRTP for media;
- require current tenant context and `media.view` on session/ticket creation;
- bind deletion to the creating user or a privileged owner;
- validate `Origin` on browser mutations in addition to existing cookie/CSRF
  protections;
- use bearer-authenticated API calls for iOS and single-use player tickets;
- rate-limit offers before expensive peer-connection allocation;
- limit SDP size, media sections, codecs, header extensions, viewers, UDP
  allocations, task count, bitrate declarations, and session lifetime;
- reserve capacity atomically to prevent concurrent limit bypass;
- redact SDP, ICE candidates, remote addresses, and tickets from logs and error
  bodies;
- zero or drop ticket/session secrets promptly;
- close sessions immediately on logout/auth invalidation where the current
  process can observe it, and always enforce the TTL;
- document that the embedded relay terminates SRTP, so media is encrypted on
  each network leg but is not end-to-end encrypted through Extrittio;
- run dependency license/advisory review and add any justified exception to
  `docs/operations/dependency-exceptions.md`.

## 12. Observability and capacity

Expose bounded host metrics through the existing operational metrics path:

- active/failed publishers;
- active viewers and rejected viewer allocations;
- signaling failures by stable reason;
- ICE/DTLS setup duration;
- inbound/outbound bytes and packets;
- packet loss, jitter, PLI/FIR/NACK counts where available;
- lagged/dropped fan-out packets;
- session duration and close reason;
- UDP allocation exhaustion.

Do not label metrics by tenant, user, device ID, session ID, candidate, or IP.
Those dimensions are unbounded or sensitive. Logs may identify tenant/device/
session for operator correlation, but must never contain negotiation payloads.

Add a sizing note: one publisher consumes one inbound encoded stream and each
viewer consumes another outbound copy. CPU is primarily encryption and packet
handling because version one does not transcode; network egress grows linearly
with viewers.

## 13. Implementation work packages

Each package must leave the repository buildable. Keep the feature disabled
until the end-to-end path and deployment checks are complete.

### P0 — interoperability spike and recorded evidence

- Pin the current stable `webrtc-rs` line behind the optional feature.
- Prove Chrome, Safari, and a Linux/Raspberry Pi publisher can negotiate H.264
  plus optional Opus against a Rust peer.
- Prove packet fan-out to two viewers without decode/transcode.
- Prove the advertised-address/UDP-range strategy in Docker and on an Edge LAN.
- Capture codec SDP, startup latency, packet-loss behavior, keyframe behavior,
  idle resource use, and failure modes in a short checked-in evidence note.
- Exit criterion: no unresolved library/API or H.264-profile blocker. If this
  fails, revise this plan before production code proceeds.

### P1 — contract, permission, and device wire foundations

- Add media blueprint/compiled-contract types, validation, compatibility, and
  `PresentationTab::Media`.
- Add `media.view`, permission migrations, and permission tests.
- Add media topic helpers, Protobuf, fixtures, generated C bindings, and Zenoh
  ACL updates.
- Regenerate and verify all committed protocol artifacts.
- Exit criterion: old contracts and wire fixtures remain compatible; a media
  contract and signaling message round-trip across Rust and C.

### P2 — backend media runtime and publisher ingress

- Add configuration and optional Cargo feature.
- Implement UDP allocation, peer factory, publisher registry, signaling
  subscriber/handler, RTP fan-out, RTCP handling, cleanup, readiness, shutdown,
  and metrics.
- Add deterministic unit tests with fake clock/ID sources around registry
  replacement, limits, expiry, and races.
- Add loopback integration tests for connect, replacement, malformed offers,
  contract mismatch, timeout, and shutdown.
- Exit criterion: a deterministic publisher can remain connected, publish both
  tracks, reconnect, and shut down without leaked tasks or sockets.

### P3 — viewer application/API slice

- Put authorization and tenant decisions behind an application-facing media
  service rather than accessing repositories directly from handlers.
- Add status, session, deletion, and ticket endpoints with OpenAPI.
- Implement bounded viewer forwarding and audit integration.
- Cover tenant isolation, permission denial, limits, idempotent delete, ticket
  single use/expiry, publisher replacement, and auth invalidation.
- Exit criterion: two authenticated viewers receive the deterministic source;
  unauthorized and cross-tenant requests allocate no peer resources.

### P4 — React console and embedded player

- Add contract-aware Live tab, player hook/component, API module, permission,
  responsive styling, and accessible states.
- Add `/media/embed` with strict ticket redemption and origin behavior.
- Add unit tests, browser tests, teardown/leak checks, and autoplay/audio tests.
- Exit criterion: Chrome and Safari show and stop a live feed predictably, and
  closing/switching tabs releases the backend viewer within the timeout.

### P5 — reference device integration

- Extend the Rust client runtime with signaling helpers and a reference Linux/
  Raspberry Pi publisher using a hardware-friendly capture/encoder boundary.
- Keep camera drivers and encoder selection outside the generic SDK.
- Document required H.264 profile, packetization mode, IDR interval, bitrate,
  Opus parameters, reconnect/backoff, and graceful stop behavior.
- Add a synthetic color-bar/tone publisher for CI and operator diagnostics.
- Exit criterion: the reference device reconnects across backend restart and
  contract replacement without manual intervention.

### P6 — iOS viewing

- Add media entities, repository/use case, ticket request, permission mapping,
  Live tab, ephemeral WebKit wrapper, navigation policy, state bridge, and
  lifecycle cleanup.
- Add unit tests for repository/view-model state and UI tests with the
  deterministic publisher.
- Verify foreground/background, rotation, route changes, audio enable, logout,
  server change, and loss/recovery behavior.
- Exit criterion: the app receives the same feed without storing credentials in
  WebKit or leaving a viewer active after dismissal.

### P7 — deployment hardening and release gate

- Wire production/Edge features, Compose UDP exposure, examples, and docs.
- Run dependency, architecture, protocol, OpenAPI, backend, frontend, iOS,
  Docker, and package verification.
- Load-test configured publisher/viewer limits and prove bounded memory under a
  slow viewer and malformed-offer flood.
- Verify upgrade from both existing database adapters and rollback with media
  disabled.
- Complete security review and publish the precise networking support matrix.
- Exit criterion: release checklist and acceptance criteria below pass on a
  production-like host and an Edge appliance.

## 14. Test and verification matrix

### 14.1 Required automated checks

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo xtask architecture
cargo xtask verify protocol
cargo xtask verify backend
npm --prefix apps/frontend test
npm --prefix apps/frontend run build
cargo xtask ios generate
cargo xtask ios build
cargo xtask ios test
docker compose config for development and production manifests
```

Add targeted loopback integration jobs for no-media and media-enabled feature
sets. Hardware/browser interoperability jobs may be scheduled rather than run
on every pull request, but their last successful evidence is a release gate.

### 14.2 Failure cases that require explicit coverage

- missing permission and cross-tenant device/session/ticket access;
- deleted device or changed assigned contract during a live session;
- wrong device ID, contract hash, stream key, protocol version, or SDP type;
- duplicate, stale, oversized, malformed, and replayed device signals;
- no publisher, viewer limit, total limit, UDP exhaustion, and timeout;
- publisher replacement while viewers are active;
- video-only and video-plus-audio negotiation;
- incompatible H.264 profile or missing Opus;
- slow or disconnected viewer without publisher backpressure;
- browser refresh, tab switch, logout, iOS dismissal/background, and process
  shutdown;
- database unavailable after authorization data is loaded, without leaking a
  partially authorized session;
- media disabled in binaries built with and without the optional feature.

## 15. Release acceptance criteria

Version one is complete only when all of the following are true:

1. A declared and provisioned device publishes H.264 video and optional Opus
   audio through authenticated Zenoh signaling.
2. An authorized web user and authorized iOS user can view the same live source
   through Extrittio.
3. A user without `media.view` and a user from another tenant cannot learn
   publisher state or allocate media resources.
4. No media payload, SDP, candidate, ticket, or frame is stored in PostgreSQL,
   Turso, object storage, logs, or the iOS cache.
5. Device and viewer reconnection is bounded and cleans up superseded state.
6. Slow viewers cannot cause unbounded memory growth or publisher latency.
7. The documented maximum viewers work within measured CPU, memory, and network
   limits on the supported production and Edge reference hosts.
8. Media-disabled deployments retain current behavior, ports, readiness, and
   resource use.
9. Existing device protocol fixtures, blueprint contracts, REST behavior, and
   both database migration paths remain compatible.
10. The deployment documentation makes the public-IP/UDP requirement and the
    absence of TURN fallback unmistakable.

## 16. Deferred decisions

The following require separate evidence and approval rather than expanding the
first implementation implicitly:

- TURN for restrictive networks;
- a dedicated or horizontally scalable SFU;
- native iOS libwebrtc and Picture in Picture;
- recordings, snapshots, retention, and object storage;
- talkback and bidirectional audio;
- simulcast/adaptive bitrate;
- H.265, VP8, VP9, or AV1;
- media analytics, inference, or rule triggers;
- end-to-end encryption through the relay.

Keeping these out of version one is what allows the embedded relay to remain a
bounded Extrittio capability instead of becoming an undeclared media platform.
