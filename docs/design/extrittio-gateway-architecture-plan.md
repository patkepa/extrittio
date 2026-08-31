# Extrittio Gateway architecture plan

Status: proposed

Date: 2026-08-31

Target application: apps/extrittio-gateway

## 1. Purpose

Extrittio Gateway is a standalone edge connectivity application. It connects
devices that speak non-native protocols to Extrittio without moving device
management, state, analytics, or business rules out of Extrittio.

The gateway is intentionally smaller than EdgeX. It owns protocol connectivity,
translation, local delivery guarantees, and bidirectional command routing. It
does not become a second IoT control plane.

The first important adapters are generic MQTT and an Azure IoT Hub-compatible
MQTT endpoint. The architecture must also support poll-based and session-based
drivers such as Modbus, OPC UA, HTTP, BLE, serial, and future proprietary
protocols.

## 2. Architectural decisions

The initial implementation follows these decisions:

1. Extrittio Gateway is a separate executable named extrittio-gateway.
2. It lives in this repository and shares versioned contract and wire crates
   with Extrittio, but it can be built, released, deployed, restarted, and
   upgraded independently.
3. Extrittio remains authoritative for tenants, devices, blueprints,
   materialized contracts, shadows, commands, and telemetry.
4. A gateway is an authenticated tenant-owned principal. It can represent only
   devices explicitly assigned to it.
5. Gateway traffic uses a dedicated, versioned northbound protocol. It does not
   impersonate hundreds of device certificates on the existing device topics.
6. Delivery is at-least-once where a contract requests it. Stable message IDs
   and backend idempotency make retries safe; the design does not promise
   distributed exactly-once delivery.
7. The gateway keeps a bounded durable SQLite spool so temporary loss of the
   Extrittio connection does not lose accepted messages.
8. Version one is one process with in-process, compile-time-selected drivers.
   Dynamic native library loading is explicitly deferred.
9. A later driver process protocol may allow independently deployed Go, C,
   Python, or proprietary drivers without creating an unstable Rust ABI.
10. Version one assigns a gateway to exactly one tenant. Multi-tenant gateway
    processes are deferred until isolation and operational demand justify them.

## 3. Goals

- Translate protocol-specific device traffic into contract-routed Extrittio
  events, presence, reported state, and command results.
- Translate Extrittio desired-state changes and commands into protocol-specific
  downlinks.
- Support push, brokered, polling, and persistent-session device protocols.
- Reuse published blueprints and materialized device contracts rather than
  introducing a second device-profile format.
- Run close to devices and continue accepting bounded traffic during temporary
  upstream outages.
- Provide explicit authentication, authorization, auditability, health, and
  operational metrics.
- Allow more than one gateway per Extrittio deployment and unambiguous
  assignment of devices to gateways.
- Make protocol conformance testable independently of a production backend.

## 4. Non-goals

- Reimplementing Extrittio persistence, analytics, rules, alerts, firmware
  management, user management, or user-facing dashboards.
- Mirroring the complete EdgeX microservice topology or internal data model.
- Automatically accepting or provisioning unknown devices by default.
- Guaranteeing that every feature of an emulated cloud service exists in the
  first adapter release.
- Running untrusted third-party native libraries inside the gateway process.
- Letting protocol payloads bypass the assigned Extrittio contract.
- Treating the gateway cache as an authoritative source after reconnection.

## 5. Current repository anchors and gaps

The design builds on current behavior rather than replacing it:

- crates/device-contract models Zenoh, MQTT, and HTTP transports, route
  direction, payload encoding, delivery semantics, endpoint bindings, and
  credential references.
- Device creation resolves only Zenoh deployment bindings today in
  crates/backend/src/domains/device_blueprints/blueprint_service.rs.
- Contract event ingestion already validates the assigned contract hash, route,
  schema, payload size, and event ID before persistence.
- Contract event ingestion currently accepts only contract-routed JSON events;
  other declared encodings need transport-neutral decoders before gateway
  drivers can preserve them end to end.
- Commands and shadow deltas are currently published directly on per-device
  Zenoh topics.
- Command dispatch rejects a materialized non-Zenoh transport because no
  transport adapter exists yet.
- The Zenoh certificate ACL binds native device certificates to per-device
  topics. A single gateway certificate therefore cannot safely reuse the
  current native-device namespace without a new authorization model.
- Device creation currently generates the Extrittio device ID. An external
  protocol identity therefore needs an explicit mapping to that ID.

These are integration seams for the gateway. They are not reasons to weaken
existing native-device identity checks.

## 6. System context

The target data path is:

    physical or emulated device
      -> protocol driver
      -> mapping and contract runtime
      -> local durable spool
      -> authenticated gateway northbound link
      -> Extrittio gateway ingress
      -> existing domain services and repositories

The reverse path is:

    Extrittio command or desired state
      -> durable gateway dispatch
      -> authenticated gateway northbound link
      -> local inbox and correlation
      -> protocol driver
      -> physical or emulated device
      -> result or reported state

An existing EdgeX deployment can later be connected through an EdgeX
MessageBus driver. EdgeX is then one possible southbound source, not a required
runtime dependency.

## 7. Bounded contexts and ownership

### 7.1 Extrittio owns

- Tenant and gateway registration.
- Device provisioning from a published blueprint.
- Gateway-to-device assignments.
- The canonical external-to-Extrittio identity binding record.
- Materialized contracts and contract assignment state.
- Authorization decisions.
- Canonical shadow and command state.
- Final event validation and durable business persistence.
- Rules, alerts, analytics, audit history, firmware, and OTA orchestration.

### 7.2 Extrittio Gateway owns

- Driver lifecycle and device sessions.
- Protocol-side authentication and authorization.
- Driver-specific discovery when enabled by policy.
- Poll schedules, subscriptions, reconnects, and protocol timeouts.
- External payload decoding and protocol response encoding.
- Local contract cache and preflight validation.
- Local external-device lookup cache.
- In-flight command and request correlation.
- Bounded durable ingress and egress queues.
- Protocol-specific health and metrics.
- Local secret resolution through opaque secret references.

### 7.3 Drivers own

- Wire parsing and encoding.
- Protocol handshakes and session state.
- Protocol-specific authentication extraction.
- Read, write, subscribe, publish, and polling mechanics.
- Mapping protocol acknowledgements to the gateway delivery API.
- Protocol-specific error classification.

Drivers do not call Extrittio APIs directly and do not write the spool directly.
They communicate with gateway core through typed ports and bounded channels.

## 8. Proposed repository structure

Start with a small number of packages:

    apps/extrittio-gateway
      src/main.rs
      src/args.rs
      src/config.rs
      src/composition.rs

    crates/gateway-protocol
      src/envelope.rs
      src/topics.rs
      src/version.rs
      proto/gateway.proto
      tests/wire_compatibility.rs

    crates/gateway-core
      src/assignment.rs
      src/contract.rs
      src/driver.rs
      src/identity.rs
      src/inbound.rs
      src/outbound.rs
      src/runtime.rs
      src/ports.rs

    crates/gateway-storage
      src/sqlite.rs
      migrations/

    crates/gateway-drivers
      src/mqtt.rs
      src/http.rs
      src/iothub.rs
      src/loopback.rs

Package boundaries have specific purposes:

- extrittio-gateway owns process composition, CLI, configuration, signal
  handling, observability, and feature selection.
- gateway-protocol owns the only shared gateway/backend wire contract. It has no
  storage, network runtime, or driver dependencies.
- gateway-core owns protocol-neutral state machines and ports. It must not
  depend on MQTT, Modbus, OPC UA, HTTP server, SQLite, or Zenoh implementations.
- gateway-storage implements the local spool and cached projections.
- gateway-drivers contains first-party adapters. A driver may be split into its
  own crate when its dependency tree or release lifecycle requires isolation.

Do not create one microservice per driver in version one. One supervised process
is easier to deploy on an edge appliance and still preserves internal
boundaries.

## 9. Northbound gateway protocol

### 9.1 Transport

Zenoh is the initial carrier because Extrittio already operates a Zenoh endpoint
and the link is bidirectional. Gateway traffic uses a separate namespace and
gateway certificate policy:

    extrittio/gateways/{gateway_id}/up/events/{device_id}/{route_key}
    extrittio/gateways/{gateway_id}/up/presence/{device_id}
    extrittio/gateways/{gateway_id}/up/reported/{device_id}
    extrittio/gateways/{gateway_id}/up/command-results/{device_id}
    extrittio/gateways/{gateway_id}/up/acks

    extrittio/gateways/{gateway_id}/down/commands/{device_id}
    extrittio/gateways/{gateway_id}/down/desired/{device_id}
    extrittio/gateways/{gateway_id}/down/contracts/{device_id}
    extrittio/gateways/{gateway_id}/down/assignments
    extrittio/gateways/{gateway_id}/down/acks

The certificate identity determines the gateway and tenant. The backend never
trusts a tenant value supplied in a topic or payload. Device and route values in
the topic are cross-checked against the envelope and active assignment.

The current device ACL is assembled at backend startup from exact certificate
common names. Phase 0 must prove that gateway subjects can be added, rotated,
and revoked with a safe runtime ACL refresh. If the active Zenoh version cannot
provide that property, version one must use a dedicated
application-authenticated streaming carrier instead of granting a broad
gateway ACL or requiring routine backend restarts. The envelope and
acknowledgement protocol remains the same under either carrier.

The protocol is transport-neutral even though Zenoh carries version one. A
future mutually authenticated streaming HTTP or gRPC carrier can reuse the
same envelopes and acknowledgement semantics.

### 9.2 Versioning

gateway-protocol defines a Protobuf envelope with:

- protocol_version;
- message_id;
- gateway_id;
- device_id;
- assignment_revision;
- contract_id and contract_hash where applicable;
- occurred_at and sent_at;
- trace context;
- one typed body.

Unknown envelope versions are rejected. New optional fields are additive.
Removing a field, changing meaning, or changing a field number requires a new
protocol version. Golden wire fixtures protect all released messages.

### 9.3 Uplink bodies

- EventUp: route key, event ID, encoding, payload, optional external
  properties, and source sequence.
- PresenceUp: connected or disconnected state, driver key, external endpoint,
  firmware when available, and reason.
- DeviceHeartbeatUp: source uptime, firmware, health status, and optional
  protocol diagnostics.
- ReportedStatePatchUp: patch, source version when available, and correlation.
- CommandResultUp: command ID, protocol status, normalized outcome, payload,
  and completion timestamp.
- DeviceLogUp: source timestamp, normalized level, bounded message, and bounded
  structured fields.
- DiscoveryObservationUp: driver instance, external identity, observed
  capabilities, and first and last observation times.
- GatewayHeartbeatUp: gateway software version, protocol compatibility range,
  queue summary, and driver summary.
- ContractAckUp: device ID, contract ID and hash, applied or rejected status,
  and bounded error detail.
- DeliveryAckUp: the durable acceptance or terminal rejection of a downlink.

### 9.4 Downlink bodies

- CommandDown: command ID, command name, validated input, deadline, and
  idempotency classification.
- DesiredStatePatchDown: canonical shadow version, patch, and deadline.
- ShadowSnapshotDown: desired and reported snapshots with canonical versions,
  used for startup convergence and protocols that implement device-side reads.
- ContractAssignmentDown: materialized contract, assignment revision, external
  identity binding, driver key, and non-secret driver configuration.
- AssignmentRemovalDown: device ID and assignment revision.
- DeliveryAckDown: durable acceptance or terminal rejection of an uplink.

### 9.5 Acknowledgement boundary

Transport-level delivery is not enough to remove a spooled item. The receiver
emits an application acknowledgement only after one of these outcomes:

- the message and its domain effect are durably committed;
- an identical message was already durably committed;
- the message is permanently rejected with a stable reason.

Transient failures are not acknowledged. The sender retries with the same
message ID. Acknowledgements themselves may be repeated.

## 10. Identity and authorization model

### 10.1 Gateway identity

Extrittio introduces a tenant-owned gateway record containing:

- immutable gateway ID;
- display name;
- status and last-seen time;
- certificate fingerprint and expiry;
- protocol compatibility range;
- software version;
- optional location and labels;
- created, rotated, disabled, and revoked timestamps.

Registration returns a one-time mTLS certificate bundle, following the existing
one-time device private-key pattern. Rotation overlaps old and new credentials
for a bounded period. Disabling a gateway immediately prevents new sessions and
downlink delivery.

### 10.2 Device assignment

Each active assignment contains:

- tenant ID;
- gateway ID;
- Extrittio device ID;
- external device identity;
- driver instance key;
- assignment revision;
- enabled state;
- non-secret binding configuration;
- opaque secret references;
- desired and acknowledged contract IDs and hashes;
- last error and convergence timestamps.

Version one allows only one active gateway assignment per device. Reassignment
uses a staged cutover:

1. Extrittio creates a higher assignment revision in pending state.
2. The new gateway validates and stages the contract and binding without
   accepting device traffic.
3. After the new gateway acknowledges readiness, Extrittio atomically activates
   the new revision and revokes the previous one.
4. Downlinks paused during the short activation transaction resume on the new
   gateway.
5. Messages from the old gateway and revision are rejected after activation.

This avoids simultaneous writers while allowing the old gateway to remain
active until the replacement is ready.

External identity uniqueness is enforced within a gateway and driver instance,
not globally. The same Modbus address or MQTT client ID may legitimately exist
behind different gateways.

### 10.3 Unknown devices

Unknown protocol identities are rejected by default and recorded as bounded,
rate-limited discovery observations. An administrator may bind an observation
to a pre-provisioned Extrittio device.

Just-in-time provisioning is a future policy. It must name an explicit
published blueprint revision, limit the accepted identity namespace, and be
auditable. A driver must never choose a blueprint based only on untrusted
payload data.

## 11. Contracts and gateway bindings

The device blueprint remains the semantic description of the device. It
declares routes, schemas, streams, commands, configuration, reported state,
limits, and logical transport requirements.

The deployment binding resolves those requirements for an assigned gateway:

- gateway ID and driver instance;
- physical endpoint or listener;
- external identity;
- protocol dialect;
- poll or reconnect parameters;
- credential references;
- route-specific decoding and encoding parameters.

Secrets are never embedded in an immutable blueprint or materialized contract.
The contract contains an opaque credential reference that only the assigned
gateway can resolve.

MQTT and HTTP already exist as contract protocols. Additional physical
protocols should be added only when their semantics affect compatibility.
Vendor or service dialects such as Azure IoT Hub MQTT should normally be a
driver or binding dialect rather than a new universal transport enum.

The gateway verifies the downloaded materialized contract and hash, as the
shared Rust device runtime already does. Local validation is an early error
filter; Extrittio repeats all authorization, limit, route, hash, and schema
validation before persistence.

### 11.1 Gateway-backed device creation

Gateway assignment and materialized contract creation must not form a circular
workflow. Add a gateway-backed variant to device creation that accepts, in one
request:

- published blueprint revision;
- gateway ID;
- driver instance key;
- external device identity;
- non-secret binding configuration;
- opaque secret references;
- the existing device metadata and configuration overlay.

One transaction creates the device, initial shadow, candidate materialized
contract, and pending gateway assignment. The compiler resolves the candidate
transport using the supplied gateway binding. The device remains pending and
cannot ingest traffic until the gateway acknowledges the contract and
assignment revision.

A gateway-managed device does not require a native per-device Zenoh certificate
unless it also has an explicitly configured native transport. Existing native
device creation and one-time certificate behavior remain unchanged.

Reassignment recompiles a candidate materialized contract when its resolved
endpoint, driver, dialect, or credential reference changes. The staged cutover
in section 10.2 activates the contract and assignment together.

## 12. Driver interface

The protocol-neutral driver interface describes capabilities rather than
forcing all drivers into read/write resource calls.

A driver descriptor reports:

- stable driver key and version;
- supported protocol and dialects;
- push, poll, request-response, discovery, and persistent-session support;
- telemetry, presence, desired state, reported state, command, and binary
  payload capabilities;
- supported authentication mechanisms;
- configuration schema and secret-reference fields.

The runtime-facing operations are conceptually:

    validate_binding(binding) -> validation result
    reconcile(assignments) -> applied or rejected revisions
    start(event_sink, cancellation)
    deliver(downlink) -> accepted, retryable, or terminal result
    health() -> driver and per-binding health
    stop(deadline)

Inbound drivers submit typed observations through an EventSink. The sink
resolves the assignment and contract before accepting data into the spool.
Drivers receive immutable assignment snapshots and never query the gateway
database directly.

Examples of driver shapes:

- Generic MQTT connects to or embeds a configured broker interface and maps
  topic identities to assignments.
- IoT Hub compatibility acts as a TLS MQTT server and implements IoT Hub topic,
  SAS or X.509, twin, and direct-method semantics.
- HTTP exposes bounded authenticated webhook routes.
- Modbus polls registers and executes writes.
- OPC UA subscribes to nodes and invokes methods.

## 13. Core data flows

### 13.1 Startup and convergence

1. The gateway opens and migrates its local database.
2. It loads the last acknowledged assignments and contracts.
3. It starts health endpoints but remains unready for new traffic.
4. It establishes an authenticated northbound session.
5. Extrittio sends an assignment snapshot followed by ordered revisions.
6. Extrittio sends the latest shadow snapshot for each active assignment.
7. The gateway verifies contracts and asks drivers to reconcile.
8. The gateway acknowledges each applied or rejected revision.
9. Drivers accepting traffic become ready independently.

Cached assignments may operate while Extrittio is temporarily unavailable if
their credentials and contracts remain valid. Revocation freshness is therefore
a configured security policy with a conservative maximum offline duration.

### 13.2 Telemetry or event ingress

1. A driver authenticates or identifies the external device.
2. Gateway core resolves the active assignment.
3. The driver decodes the protocol payload into the declared route encoding.
4. Gateway core checks payload size, route direction, contract hash, and schema.
5. It assigns stable message and event IDs and persists the item when required
   by delivery semantics.
6. The northbound publisher sends the event.
7. Extrittio resolves the authenticated gateway and assigned device, repeats
   validation, and calls the existing event application service.
8. Extrittio acknowledges only after durable persistence or idempotent replay.
9. The gateway removes the acknowledged spool record.

The original protocol properties may be attached as bounded metadata, but they
must not determine tenant, gateway, or Extrittio device identity after
authentication.

### 13.3 Desired state

1. Extrittio commits the desired-state change.
2. A durable transport-dispatch record targets the assigned gateway.
3. The gateway stores the downlink before acknowledging receipt.
4. The driver translates it into protocol-specific desired-state behavior.
5. A device report returns through ReportedStatePatchUp.
6. Extrittio reconciles desired and reported state and sends later deltas as
   necessary.

The gateway does not independently merge authoritative desired and reported
state. It may retain the latest versions for protocol responses while offline.

### 13.4 Command

1. Extrittio validates command input against the materialized contract and
   persists the command record.
2. The transport dispatcher sends CommandDown with the existing command ID and
   deadline.
3. The gateway durably accepts it and invokes the assigned driver.
4. The driver maps the command to a direct method, write, RPC, or protocol
   operation.
5. The result returns with the same command ID.
6. Extrittio applies the result idempotently or marks the command timed out.

Redelivery follows the command idempotency declaration. A non-idempotent
command is never automatically invoked again after the driver reports that
execution may have started; it enters an uncertain terminal state for operator
review.

### 13.5 Gateway or device disconnect

Driver session changes emit presence observations. Extrittio owns the canonical
online/offline projection. Loss of the northbound link marks the gateway
unreachable but does not immediately claim every attached device disconnected;
device status follows configured freshness and heartbeat rules.

## 14. Durability, ordering, and backpressure

gateway-storage uses SQLite in WAL mode with explicit migrations. Minimum
tables are:

- metadata and schema version;
- cached assignments;
- cached contracts;
- uplink spool;
- downlink inbox;
- application acknowledgements;
- command execution journal;
- per-stream source sequence or watermark;
- bounded discovery observations.

The spool records payload bytes, type, stable message ID, device, route,
priority, attempt count, available-at time, expiry, and last bounded error.

Delivery rules:

- At-least-once ingress is persisted before the protocol is acknowledged when
  the southbound protocol permits that boundary.
- At-most-once ingress may bypass disk only when the assigned contract
  explicitly allows it.
- Per-device-stream ordering is maintained until an item expires or is
  terminally rejected.
- Presence and reported state have higher delivery priority than bulk
  telemetry; commands and acknowledgements have the highest priority.
- The spool has byte and record limits, not only record limits.
- On capacity exhaustion, the gateway becomes degraded and applies
  protocol-specific backpressure. It does not silently discard durable
  messages.
- A configured policy may coalesce superseded presence or state projections.
  It never coalesces command results or contract events.
- Retry delay uses bounded exponential backoff with jitter.
- Terminal rejections go to a bounded local dead-letter view with operator
  diagnostics and an explicit replay action.

## 15. Extrittio backend changes

The gateway requires a focused backend feature set:

### 15.1 Gateway domain

- Gateway registration, listing, status, rotation, disable, and revoke use
  cases.
- Gateway-backed device creation, assignment, staged reassignment, and removal
  use cases.
- Tenant-scoped persistence for PostgreSQL and Turso.
- Audit events for identity and assignment changes.
- Gateway health and backlog projections.

### 15.2 Gateway ingress

- A distinct Zenoh listener namespace and gateway mTLS policy.
- Envelope decoding and version negotiation.
- Gateway and assignment authorization.
- Topic-to-envelope identity cross-checking.
- Dispatch into existing event, presence, shadow, and command-result
  application services.
- Durable application acknowledgements.

Protocol-neutral event validation and persistence currently embedded in the
Zenoh handler must move behind an application service callable by both native
device and gateway transports. The native handler retains parsing and identity
checks; semantic behavior is shared.

### 15.3 Transport dispatch

Commands and desired-state publication must stop depending directly on a Zenoh
device topic. Introduce a transport dispatch port selected from the active
materialized transport and assignment:

- NativeZenohDispatch keeps current behavior.
- GatewayDispatch writes a durable gateway downlink.

Persistence of the domain change and durable downlink intent must be atomic
where partial success would lose a command or desired-state notification.

### 15.4 Contract compilation

Gateway-backed device creation resolves MQTT, HTTP, and future transports from
the requested gateway binding while atomically creating the pending assignment.
Reassignment resolves from its staged binding. Neither workflow filters all
non-Zenoh transports. A candidate contract cannot become active until the
assigned gateway acknowledges that its driver can apply it.

### 15.5 Public API

The committed OpenAPI contract will eventually include:

- gateway CRUD and certificate lifecycle;
- assignment CRUD and convergence status;
- discovery observations and bind action;
- gateway health and queue summaries;
- bounded dead-letter inspection and replay.

No gateway private key or resolved secret value is returned after its one-time
retrieval.

## 16. Configuration and secrets

The application accepts a configuration file plus environment overrides for
deployment concerns. A representative shape is:

    gateway:
      id: gw-plant-01
      dataDir: /var/lib/extrittio-gateway

    extrittio:
      endpoint: tls/extrittio.example.net:7447
      caCertificate: /etc/extrittio-gateway/ca.pem
      certificate: /etc/extrittio-gateway/gateway.pem
      privateKey: /etc/extrittio-gateway/gateway.key

    runtime:
      maxSpoolBytes: 1073741824
      maxSpoolRecords: 100000
      maximumOfflineDuration: 24h

    drivers:
      - key: factory-mqtt
        type: mqtt
        enabled: true
      - key: legacy-iothub
        type: azure_iot_hub_mqtt
        enabled: true

Driver binding details normally arrive through assignments. Host-level listener
ports and secret-store configuration remain local so a compromised control
plane value cannot unexpectedly expose a new interface.

Secrets use opaque references and a SecretResolver port. Initial resolvers may
read protected files and environment variables. OS keychains, TPM-backed keys,
or an external secret store can be added without changing driver contracts.
Resolved secrets are never serialized into logs, assignment caches, health
responses, or protocol acknowledgements.

## 17. Process lifecycle and CLI

Initial CLI surface:

    extrittio-gateway run --config <path>
    extrittio-gateway check --config <path>
    extrittio-gateway drivers list
    extrittio-gateway database info
    extrittio-gateway database integrity

run owns graceful signal handling. Shutdown order is:

1. stop accepting new southbound sessions or polls;
2. allow bounded in-flight driver work to finish;
3. flush local transactions;
4. stop northbound publishers and subscribers;
5. close storage and observability endpoints.

The process exits nonzero on invalid configuration, failed migration, unusable
identity, or inability to start every required driver. An optional driver
failure makes readiness degraded but does not terminate unrelated drivers.

## 18. Observability

The gateway exposes local health and readiness endpoints, structured logs, and
metrics. Payload bodies and credentials are excluded from logs by default.

Required dimensions are bounded to gateway, driver, protocol, direction,
outcome, and error class. External or Extrittio device IDs must not become
unbounded metric labels.

Minimum metrics:

- active and failed driver instances;
- connected devices by driver;
- messages and bytes accepted, delivered, retried, rejected, and expired;
- spool records, bytes, oldest age, and capacity ratio;
- command latency and outcomes;
- contract convergence counts;
- northbound reconnect count and session state;
- protocol authentication failures and rate-limit rejections.

Readiness requires usable storage, a valid gateway identity, and every required
driver listening or connected. Upstream unavailability produces degraded
readiness only after the configured offline policy is exceeded.

## 19. Security invariants

- A gateway certificate is bound to one gateway record and one tenant.
- A gateway can act only for active device assignments.
- Extrittio never authorizes from a payload-supplied tenant.
- Topic gateway, envelope gateway, certificate gateway, topic device, envelope
  device, and assignment device are cross-checked.
- Backend contract, schema, rate, and size validation remains mandatory even
  when the gateway already validated a message.
- Protocol-side credentials are scoped to the smallest possible driver or
  device set.
- Listener drivers default to TLS and explicit authentication.
- Administrative and health listeners bind to loopback unless configured
  otherwise.
- Discovery is bounded and cannot provision by itself.
- Failed authentication and assignment violations are rate-limited and
  auditable.
- Private keys and resolved secrets are redacted from errors, diagnostics, and
  support bundles.
- Gateway software and protocol versions are checked before assignments are
  delivered.
- Revocation and maximum-offline policies limit how long stale assignments may
  operate without contacting Extrittio.

## 20. Deployment model

Supported deployment shapes:

1. A standalone systemd service on an industrial or Raspberry Pi gateway.
2. A standalone container with persistent storage and explicit device/network
   access.
3. A remote gateway connected to a central Extrittio production deployment.
4. An optional child process supervised by extrittio run in a future Edge
   bundle.

Bundling must not turn the gateway into a library hosted inside the backend.
The process and wire boundary remains intact so protocol crashes, native
dependencies, listener ports, and upgrades remain isolated.

Packaging keeps mutable state outside release directories. Backup covers the
gateway SQLite database and locally managed credentials, while excluding
replayable caches when policy permits. Restore must not clone a gateway
identity onto two simultaneously active machines.

## 21. Testing strategy

### 21.1 Protocol unit and compatibility tests

- Golden Protobuf bytes for every released gateway envelope.
- Topic parser fixtures, including invalid hierarchy and wildcard characters.
- Version negotiation and unknown-field behavior.
- Stable acknowledgement and rejection codes.

### 21.2 Gateway core tests

- Assignment reconciliation and stale revision rejection.
- Contract hash and schema validation.
- Identity mapping isolation across gateways and driver instances.
- Command deadline and idempotency state machines.
- Spool ordering, capacity, retry, expiry, restart, and dead-letter behavior.
- Cancellation and graceful shutdown.

### 21.3 Backend adapter tests

Run the same gateway-domain and ingress fixtures against PostgreSQL and Turso.
Verify tenant isolation, assignment uniqueness, atomic downlink intent,
idempotent ingress, certificate rotation, and revocation.

### 21.4 Driver conformance suite

gateway-core supplies a reusable harness. Every driver demonstrates:

- binding validation;
- clean startup and shutdown;
- bounded event submission;
- retryable versus terminal error classification;
- command correlation;
- secret redaction;
- reconnection;
- health reporting.

### 21.5 End-to-end tests

- Loopback driver through gateway into Extrittio and back.
- Generic MQTT telemetry, desired state, and command response.
- Azure IoT Device SDK telemetry, twin GET and PATCH, desired notification, and
  direct method response against the compatibility driver.
- Backend restart, gateway restart, network partition, duplicate delivery,
  contract reassignment, gateway revocation, and spool exhaustion.
- Multi-gateway tenant-isolation attempts.

Performance tests establish bounded memory and disk behavior before publishing
device-count or throughput claims.

## 22. Delivery phases

### Phase 0: compatibility baseline and decisions

- Record the current native Zenoh event, shadow, command, certificate, and ACL
  behavior with fixtures.
- Finalize gateway identity, assignment, acknowledgement, and offline-policy
  decisions.
- Prove safe runtime authorization refresh for gateway certificate issue,
  rotation, disable, and revoke; otherwise select the authenticated streaming
  carrier before implementation.
- Specify protocol error and rejection codes.

Exit criterion: approved protocol and persistence decision records with no
unresolved security boundary.

### Phase 1: protocol and backend foundation

- Add gateway-protocol with golden fixtures.
- Add gateway identity and assignment domains, migrations, API, and audit.
- Add atomic gateway-backed device creation and staged reassignment.
- Add the dedicated Zenoh namespace and authentication policy.
- Add gateway ingress with a loopback test publisher.
- Introduce the transport dispatch port and durable gateway downlinks.

Exit criterion: a test gateway can be registered, assigned one device, ingest
an idempotent event, receive a command, return its result, and rotate its
certificate on both PostgreSQL and Turso.

### Phase 2: extrittio-gateway core

- Scaffold the app and core/storage packages.
- Implement configuration, identity loading, assignment convergence, contract
  cache, SQLite spool, northbound link, health, metrics, and graceful shutdown.
- Implement the loopback driver and conformance harness.

Exit criterion: restart and network-partition tests preserve accepted durable
messages and do not execute a non-idempotent command twice.

### Phase 3: generic MQTT and HTTP

- Implement generic MQTT broker/client binding modes required by selected
  device use cases.
- Implement authenticated HTTP webhook ingress.
- Add mapping configuration, protocol error responses, and end-to-end tests.

Exit criterion: blueprint-defined JSON telemetry, desired state, and commands
work through both drivers without backend device-family branches.

### Phase 4: Azure IoT Hub-compatible MQTT

- Implement TLS MQTT 3.1.1 listener behavior.
- Implement SAS and selected X.509 device authentication.
- Map IoT Hub telemetry topics and application properties.
- Implement device twin GET, reported PATCH, desired notifications, and direct
  method request/response.
- Define the explicit unsupported surface for AMQP, DPS, jobs, file upload,
  modules, and service-side APIs.
- Run interoperability tests against supported Azure IoT Device SDK versions.

Exit criterion: an existing supported MQTT device application connects using a
gateway hostname and trust certificate, sends telemetry, synchronizes twin
state, and executes direct methods without source changes beyond connection
configuration.

### Phase 5: industrial drivers and driver isolation

- Add drivers according to product demand, beginning with one poll-based
  protocol.
- Validate the capability model against Modbus or OPC UA.
- Specify an out-of-process driver protocol only after two implementations
  demonstrate a need for language or crash isolation.
- Add an EdgeX MessageBus driver if existing EdgeX deployments are a target.

Exit criterion: a poll-based driver and a session-based driver share the same
gateway core without protocol-specific branches in Extrittio.

## 23. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| The gateway grows into a second control plane | Keep authoritative state and user workflows in Extrittio; make gateway storage a cache and delivery journal only |
| Protocol differences leak into backend domains | Normalize at the driver boundary and preserve route/schema validation in contracts |
| One gateway credential can spoof arbitrary devices | Tenant-bound gateway identity plus explicit active assignments and identity cross-checks |
| Network retries duplicate telemetry or commands | Stable IDs, application acknowledgements, idempotent persistence, and command execution journal |
| Offline buffering exhausts disk | Byte and record limits, readiness degradation, backpressure, expiry, and operator-visible dead letters |
| Dynamic plugins destabilize the edge process | Compile first-party drivers in process initially; use a versioned subprocess protocol later |
| Existing native-device security is weakened | Use a separate gateway namespace and ACL; do not broaden native per-device topic permissions |
| Zenoh gateway ACL changes cannot be applied safely at runtime | Prove runtime refresh in Phase 0 or select an application-authenticated streaming carrier before implementation |
| Blueprint and gateway configuration overlap | Blueprints own semantics; deployment bindings own endpoints, dialects, and secret references |
| IoT Hub compatibility becomes unbounded | Publish a precise supported protocol matrix and test each supported SDK behavior |
| Backend and gateway releases drift | Version negotiation, compatibility ranges, golden fixtures, and staged contract assignment |

## 24. Definition of the first production-ready release

The first production-ready release is complete when:

- extrittio-gateway is independently packaged and upgradeable;
- gateway identity, assignment, rotation, disable, and revoke flows are
  tenant-safe and audited;
- PostgreSQL and Turso implement identical gateway semantics;
- the gateway survives restart and bounded upstream outages without losing
  accepted at-least-once messages;
- commands, desired state, reported state, presence, and contract events are
  bidirectional and idempotent;
- generic MQTT and the documented IoT Hub-compatible MQTT subset pass
  interoperability tests;
- unsupported cloud-emulation features fail explicitly rather than appearing
  to succeed;
- queue, driver, connection, and convergence health are observable;
- native Zenoh devices retain their existing wire and authorization behavior;
- threat modeling and performance limits have been recorded and verified.
