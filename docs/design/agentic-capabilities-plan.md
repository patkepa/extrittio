# Agentic Capabilities Architecture and Implementation Plan

- **Status:** Proposed
- **Date:** 2026-09-05
- **Scope:** Tenant-scoped, durable operations agents for the Extrittio control plane
- **Initial product slice:** Read-only device investigation and operational diagnosis
- **Primary packages:** `extrittio-backend-core`, `extrittio-backend-postgres`,
  `extrittio-backend-turso`, `extrittio-backend`, and `apps/frontend`
- **Migration rule:** Agent execution must use the same application, authorization,
  tenancy, validation, and delivery boundaries as human-initiated operations

## 1. Executive decision

Add agents as a vertical domain in the existing modular monolith. Do not begin
with a separately deployed agent service, a generic multi-agent framework, or a
model-facing wrapper around the HTTP API.

The target architecture is:

```text
React console / CLI
        |
        v
agent HTTP API -----> AgentApplication -----> AgentRunRepository
                            |                        |
                            |                        v
                            |                 durable run ledger
                            |                        |
                            v                        v
                      Agent worker <---------- claim / lease
                            |
                            v
                       ModelGateway
                            |
                     proposed tool calls
                            |
                            v
                  policy-enforcing ToolRegistry
                     |                    |
                read-only tool       mutation tool
                     |                    |
                     v              approval required
                Application <--------------+
                  use cases
                     |
          persistence / outbox / Zenoh / object store
```

The principal rule is:

> The model may propose. The application authorizes. A typed tool executes.
> The database records. External side effects use durable delivery.

The model is an untrusted planner, not an authorization boundary and not a
privileged runtime component.

## 2. Why this shape fits Extrittio

Extrittio already has most of the necessary structural pieces:

- tenant and actor context in `extrittio-backend-core`;
- a permission catalog and a curated `Application` facade;
- PostgreSQL and Turso adapters with shared semantic contract testing;
- supervised background workers and cancellation;
- a rule-action outbox with leasing, retries, and dead letters;
- audit and activity records;
- OpenAPI as the committed HTTP contract;
- a React feature-slice organization and generated API types.

Agents should extend those boundaries rather than introduce a parallel stack.
In particular, new agent tools must not add to the temporary direct
`AppState -> RepositorySet` access ledger. Any domain operation needed by a
tool must first be available through a narrow application use case.

Two current limitations affect sequencing:

1. Most operational domains have not yet migrated behind
   `extrittio_backend_core::Application`.
2. command creation and Zenoh publication are not one durable operation. The
   existing architecture decision record lists durable command dispatch as a
   separate unresolved guarantee.

For those reasons, the first slice is read-only and the agent migration moves
required read use cases into core before exposing them as tools.

## 3. Goals

The completed capability must:

1. Investigate devices, telemetry, shadows, alerts, logs, commands, firmware
   deployments, and fleet context using bounded typed tools.
2. Preserve tenant isolation even when model output contains a different
   tenant identifier.
3. Never grant an agent more authority than the initiating actor and the agent
   definition both allow.
4. Persist enough state to resume safely after process failure, provider
   timeout, database failover, or operator cancellation.
5. Require explicit, argument-bound approval for material side effects.
6. Make every model request, tool decision, tool result, approval, and terminal
   outcome observable without storing hidden chain-of-thought.
7. Support both PostgreSQL server and local Turso Edge runtime shapes.
8. Keep model providers replaceable and optional.
9. Bound financial cost, token use, wall time, tool calls, result sizes, and
   concurrent executions.
10. Permit deterministic tests without contacting a model provider.
11. Degrade the agent feature independently; provider failure must not make the
    IoT control plane unready.
12. Make architectural violations fail in `cargo xtask architecture`.

## 4. Non-goals

The initial project does not introduce:

- autonomous firmware rollout or bulk device control;
- arbitrary SQL, shell, filesystem, HTTP, or Zenoh tools;
- agents running directly on device clients;
- a general workflow language or replacement for the deterministic rule engine;
- vector search as a prerequisite;
- agent-to-agent delegation;
- user-authored executable tool code;
- a separate agent microservice;
- cross-tenant or installation-wide reasoning;
- guaranteed correctness of model-generated diagnoses;
- storage or exposure of provider-private reasoning or chain-of-thought;
- an MCP server as the internal agent execution path.

An external MCP facade may be considered later, but it must delegate to the
same application use cases and policy enforcement as the built-in agent.

## 5. Product scope and initial use case

The first agent is an **operations investigator** invoked from a device page or
the global operations console.

Example request:

> Investigate why device `sensor-17` became unhealthy in the last two hours.

The result contains:

- a concise diagnosis;
- confidence and explicit uncertainty;
- a timeline of relevant observations;
- evidence references to exact Extrittio records or query windows;
- suggested next actions;
- no side effects.

The initial tool set is:

| Tool | Required permission | Server-enforced bounds |
| --- | --- | --- |
| `devices.get.v1` | `devices.read` | One device in the run tenant |
| `device_contracts.get_assigned.v1` | `devices.read`, `device_blueprints.read` | Assigned contract only |
| `shadows.get.v1` | `shadows.read` | One device; redacted paths removed |
| `telemetry.query.v1` | `telemetry.read`, `devices.read` | One device, approved metrics, maximum time window and point count |
| `alerts.list.v1` | `alerts.read` | Bounded page and time range |
| `commands.list.v1` | `commands.read` | Bounded page and time range; parameters/results redacted by policy |
| `activity.list.v1` | relevant underlying read permissions | Bounded page and source filters |
| `fleet_peers.compare.v1` | `devices.read`, `fleets.read`, `telemetry.read` | One fleet and a small peer sample |

The exact names are stable identifiers. Tool descriptions may evolve without
renaming, but an incompatible input or output change requires a new version.

## 6. Package ownership and module layout

### 6.1 `extrittio-backend-core`

Core owns business meaning and policy:

```text
crates/backend-core/src/
  agents.rs                         domain values and enums
  application/
    agents.rs                       run, cancel, approve, inspect use cases
  ports.rs                          ModelGateway-adjacent domain-safe contracts if needed
  repositories.rs                  AgentRepository in RepositorySet
```

Recommended public types include:

- `AgentId`, `AgentVersionId`, `AgentRunId`, `AgentEventId`, `ToolCallId`;
- `AgentDefinition`, `AgentDefinitionVersion`;
- `AgentRun`, `AgentRunStatus`, `AgentTrigger`;
- `AgentEvent`, `AgentEventKind`;
- `ToolName`, `ToolVersion`, `ToolRisk`, `ToolProposal`;
- `Approval`, `ApprovalStatus`, `ApprovalDecision`;
- `RunBudget`, `RunUsage`, `RunTerminalReason`;
- `AgentRepository`;
- `AgentApplication`.

Core owns the legal state transitions, authorization order, approval policy,
argument-hash verification, budget checks, and stable application errors.

Core does not know about provider model names, provider SDK request structs,
Axum, SSE, Tokio tasks, Zenoh, Reqwest, process environment, or database row
types.

### 6.2 Storage adapters

Each adapter owns:

```text
crates/backend-postgres/src/agents.rs
crates/backend-turso/src/agents.rs
```

and its engine-specific migrations. Both implementations satisfy the same
contract suite in `extrittio-backend-adapter-tests`.

The repository contract should expose semantic operations rather than generic
CRUD, including:

- create a run and initial event atomically;
- append an event with the next sequence number;
- claim the next eligible run with a lease;
- renew or release a lease;
- record a model turn and usage;
- create a tool proposal exactly once;
- transition a proposal to execution or approval waiting;
- decide an approval exactly once;
- reserve a tool execution idempotency key;
- record a tool result and resume the run atomically;
- request cancellation;
- move a run to a terminal state;
- list runs and events within a tenant;
- expire approvals and recover abandoned leases.

### 6.3 `extrittio-backend` runtime host

The host owns concrete integration and transport code:

```text
crates/backend/src/
  api/
    agents.rs                       Axum DTOs, routes, SSE translation
  agents/
    mod.rs
    worker.rs                       supervised durable orchestration
    provider.rs                     ModelGateway implementations
    prompt.rs                       prompt assembly and data classification
    registry.rs                     typed tool registry
    policy.rs                       host deployment/egress restrictions
    tools/
      devices.rs
      telemetry.rs
      alerts.rs
      shadows.rs
      commands.rs
      activity.rs
```

The provider implementation belongs in the host because it performs network
I/O, reads runtime configuration, translates provider errors, applies request
timeouts, and emits telemetry.

The tool registry belongs in the host because it binds stable tool contracts to
application use cases. It must not receive `RepositorySet` or expose a general
service locator.

### 6.4 Frontend

```text
apps/frontend/src/features/agents/
  api.ts
  hooks.ts
  types.ts
  agent-panel.tsx
  run-list.tsx
  run-timeline.tsx
  evidence-card.tsx
  tool-call-card.tsx
  approval-card.tsx
  usage-summary.tsx
```

The device detail page should provide contextual entry into a new run. A
global route can list prior runs and continue runs awaiting input or approval.
The UI consumes REST for commands and SSE for progress.

## 7. Agent identity and authorization

### 7.1 Actor provenance

Extend `Actor` with an explicit agent variant:

```rust
Actor::Agent {
    run_id: AgentRunId,
    agent_id: AgentId,
    initiated_by: Box<Actor>,
}
```

All tool execution receives a non-optional `TenantContext` containing this
actor. The run stores immutable initiation provenance, but authorization uses a
fresh principal snapshot before each tool execution.

For user-started runs, effective authorization is:

```text
current permissions of initiating user
  INTERSECT permissions allowed by agent definition
  INTERSECT tools enabled by deployment policy
```

If the user is disabled, deleted, moved to another tenant, or loses a required
permission, pending execution fails closed. It does not continue using the
permission snapshot from run creation.

Scheduled or rule-triggered agents use an explicit service principal stored in
configuration or domain state. They do not impersonate the default tenant or a
historical user. Service-principal support is deferred until interactive runs
are proven.

### 7.2 Permissions

Add stable permission keys:

| Permission | Meaning |
| --- | --- |
| `agents.read` | Read runs and non-secret run events |
| `agents.run` | Start and continue permitted agents |
| `agents.manage` | Create, version, enable, and disable definitions |
| `agents.approve` | Participate in approvals; target action permission is also required |

Agent permissions do not replace target-domain permissions. For example,
approving `commands.send.v1` requires both `agents.approve` and
`commands.send` at approval time and again at execution time.

The existing stable permission order is externally observable. New keys must
be appended deliberately, with golden API and adapter tests updated in the same
change.

### 7.3 Tenant invariants

- Tenant IDs never appear in model-selectable tool arguments.
- Repositories require `TenantId` for every tenant-owned operation.
- IDs supplied by tools are resolved under the run tenant.
- Cross-tenant references return the same result as missing references.
- Evidence handles are tenant-scoped opaque IDs, not unrestricted database IDs.
- Provider metadata must not include tenant secrets or authentication tokens.

## 8. Tool contract and execution policy

Every tool is a statically registered Rust implementation with:

```rust
struct ToolDescriptor {
    name: ToolName,
    version: ToolVersion,
    description: String,
    input_schema: serde_json::Value,
    output_schema: serde_json::Value,
    required_permissions: Vec<Permission>,
    risk: ToolRisk,
    limits: ToolLimits,
}
```

Suggested risks are:

- `ReadOnly`: cannot mutate durable or external state;
- `ReversibleMutation`: bounded operation with a defined compensating action;
- `IrreversibleMutation`: destructive, external, or fleet-wide action;
- `Prohibited`: never executable by an agent in the current deployment.

The execution sequence is fixed:

1. Parse a provider tool proposal into the registered input type.
2. Reject unknown tool names and unsupported versions.
3. Validate JSON Schema and Rust domain validation.
4. Apply deployment and per-tool limits.
5. Rehydrate the initiating principal and derive `TenantContext`.
6. Verify agent allowlist and all target permissions.
7. Canonicalize arguments and calculate `SHA-256(tool + version + args)`.
8. For a read, reserve the idempotency key and execute.
9. For a mutation, create an approval request or reject it according to policy.
10. Invoke a narrow `Application` use case; never invoke a repository directly.
11. Redact and size-limit the result.
12. Persist the result and audit event before scheduling the next model turn.

Tool results should be structured data, not prose. The model is responsible for
the user-facing explanation. Large time series are aggregated by the tool; raw
unbounded telemetry is never placed into model context.

## 9. Approval protocol

An approval is bound to:

- tenant ID;
- run ID;
- tool-call ID;
- tool name and version;
- canonical argument hash;
- human-readable impact summary generated from deterministic server code;
- required permission set;
- requester provenance;
- approver provenance;
- expiration time;
- single-use status.

The model-generated explanation may be displayed, but the approval title,
target identity, and impact fields come from trusted application code.

Approval rules for the first mutation phase:

- one authorized human approval for single-device reversible mutations;
- a different human from the initiating service principal for autonomous runs;
- two-person or administrator approval for bulk, firmware, credential,
  certificate, deletion, or security-sensitive operations;
- no approval path at all for prohibited tools.

Changing any argument invalidates the approval. Expired approvals cannot be
reopened; the model must propose a new call. Approval endpoints use
compare-and-set semantics so concurrent approve/reject requests yield one
winner and a stable conflict outcome.

## 10. Durable run lifecycle

### 10.1 States

```text
queued
  -> running
  -> waiting_for_user
  -> queued
  -> running
  -> waiting_for_approval
  -> queued
  -> running
  -> completed

queued | running | waiting_for_user | waiting_for_approval
  -> cancellation_requested
  -> cancelled

queued | running
  -> failed | budget_exhausted
```

Only core defines legal transitions. Persistence adapters implement atomic
compare-and-set operations against the expected current state.

### 10.2 Leases and recovery

- Workers claim queued runs with a unique worker ID and lease deadline.
- Only the lease owner may append worker-generated events or renew the lease.
- Long provider requests renew the lease in a separate heartbeat path.
- An expired `running` lease returns to `queued` unless its current operation
  has an ambiguous side effect.
- Ambiguous external mutations transition to `reconciliation_required`, not
  automatic retry.
- Read-only calls may be retried using the same idempotency key.
- Cancellation is cooperative during model I/O and mandatory before the next
  model or tool step.

### 10.3 Idempotency

Use `run_id + provider_turn + tool_call_id` as the stable execution identity.
The repository reserves it before invoking a tool. A duplicate proposal returns
the recorded result when safe, rather than executing again.

Mutation tools additionally pass that identity into the target application
use case and its durable outbox. An agent-side ledger alone is insufficient to
make a non-idempotent external operation safe.

## 11. Persistence model

The following is a semantic schema; adapters may use engine-appropriate types
while preserving the contract.

### 11.1 `agent_definitions`

```text
id                    text primary key
tenant_id             text not null
name                  text not null
description           text not null
enabled               boolean not null
active_version_id     text null
created_by_actor      structured provenance
created_at            timestamp not null
updated_at            timestamp not null
unique (tenant_id, name)
unique (tenant_id, id)
```

### 11.2 `agent_definition_versions`

```text
id                    text primary key
tenant_id             text not null
agent_id              text not null
version               integer not null
instructions          text not null
tool_allowlist        json not null
tool_policy           json not null
default_budget        json not null
context_policy        json not null
provider_profile      text not null
created_by_actor      structured provenance
created_at            timestamp not null
unique (tenant_id, agent_id, version)
```

Published versions are immutable. A run references the exact version used.
Provider credentials are never stored in this table.

### 11.3 `agent_runs`

```text
id                    text primary key
tenant_id             text not null
agent_id              text not null
agent_version_id      text not null
status                text not null
trigger_kind          text not null
initiating_actor      structured provenance not null
subject_type          text null
subject_id            text null
permission_version    bigint/null, for diagnostic provenance only
budget                json not null
usage                 json not null
next_event_sequence   bigint not null
lease_owner           text null
lease_expires_at      timestamp null
cancel_requested_at   timestamp null
terminal_reason       text null
created_at            timestamp not null
updated_at            timestamp not null
completed_at          timestamp null
unique (tenant_id, id)
```

The stored permission version explains what existed when the run started; it
does not authorize later execution.

### 11.4 `agent_events`

```text
id                    text primary key
tenant_id             text not null
run_id                text not null
sequence              bigint not null
kind                  text not null
visibility            text not null
payload               json not null
created_at            timestamp not null
unique (tenant_id, run_id, sequence)
```

Events are append-only. Corrections append a replacement or redaction marker.
Payloads have an explicit schema version.

### 11.5 `agent_tool_calls`

```text
id                    text primary key
tenant_id             text not null
run_id                text not null
provider_turn         integer not null
provider_call_id      text null
tool_name             text not null
tool_version          integer not null
arguments             json not null
arguments_hash        text not null
risk                   text not null
status                text not null
result                 json null
error_code             text null
idempotency_key       text not null
started_at             timestamp null
completed_at           timestamp null
unique (tenant_id, idempotency_key)
```

### 11.6 `agent_approvals`

```text
id                    text primary key
tenant_id             text not null
run_id                text not null
tool_call_id          text not null
arguments_hash        text not null
status                text not null
requested_at          timestamp not null
expires_at            timestamp not null
decided_at             timestamp null
decided_by_actor      structured provenance null
decision_reason       text null
unique (tenant_id, tool_call_id)
```

### 11.7 Retention

- Run metadata and semantic audit records follow the installation audit policy.
- Large tool results and provider transcripts have a shorter configurable
  retention period.
- Redaction runs append evidence of redaction and replace sensitive payload
  fields without rewriting unrelated audit history.
- PostgreSQL cleanup uses a supervised retention worker.
- Turso cleanup must be incremental and followed by the existing maintenance
  checkpoint policy.

## 12. Model provider boundary

Define a narrow provider-neutral host trait rather than allowing provider SDK
types into application code:

```rust
#[async_trait]
trait ModelGateway: Send + Sync {
    async fn generate_turn(
        &self,
        request: ModelTurnRequest,
        cancellation: CancellationToken,
    ) -> Result<ModelTurn, ModelGatewayError>;
}
```

`ModelTurnRequest` contains only the common capabilities Extrittio needs:

- versioned system instructions;
- prior visible events or a compacted summary;
- registered tool descriptors;
- provider-independent output constraints;
- token/output limit;
- correlation metadata with no secrets.

`ModelTurn` is normalized into:

- assistant message;
- zero or more tool proposals;
- request/response usage;
- provider request ID;
- finish reason;
- optional provider safety refusal.

Do not attempt to normalize every provider feature. Provider-specific options
belong in a named `provider_profile` resolved by the host. Unsupported required
capabilities cause startup validation or agent-version publication failure.

Provider retries are limited to transient, pre-response failures. A retry uses
the same client request ID when supported. Rate-limit responses honor bounded
server hints. Invalid tool output consumes a repair attempt and is included in
the run budget.

## 13. Context, evidence, and prompt-injection defense

Device telemetry, logs, shadow values, blueprint text, command output, and
external webhook content are untrusted data. They must be clearly delimited as
data in model input and can never modify the system policy or tool catalog.

Context policy:

- begin with the user's request and a small trusted subject summary;
- retrieve operational data only through tools;
- aggregate time series before sending;
- truncate and label large strings;
- remove configured secret JSON paths and credential-shaped values;
- attach stable evidence handles to every tool result;
- require the final answer to reference evidence handles for operational facts;
- reject tool arguments derived from data if they fail normal server-side
  validation;
- do not enable outbound web retrieval in the initial release.

Evidence handles resolve through an authenticated endpoint to a safe projection
of the original record or exact query definition. They are not provider-visible
URLs and expire according to run retention.

Conversation compaction is deterministic where possible. Store a compacted
summary as a visible event, retain the source event range it covers, and never
allow compaction to erase pending approvals or tool results needed for
idempotency.

## 14. Data egress and secret management

Agents are disabled by default until an administrator configures a provider or
local model endpoint and acknowledges the data-egress policy.

Configuration should include:

```text
EXTRITTIO_AGENTS_ENABLED
EXTRITTIO_AGENT_PROVIDER
EXTRITTIO_AGENT_PROVIDER_ENDPOINT
EXTRITTIO_AGENT_PROVIDER_API_KEY
EXTRITTIO_AGENT_MODEL
EXTRITTIO_AGENT_MAX_CONCURRENT_RUNS
EXTRITTIO_AGENT_DEFAULT_MAX_STEPS
EXTRITTIO_AGENT_DEFAULT_MAX_INPUT_TOKENS
EXTRITTIO_AGENT_DEFAULT_MAX_OUTPUT_TOKENS
EXTRITTIO_AGENT_DEFAULT_TIMEOUT_SECS
EXTRITTIO_AGENT_RESULT_RETENTION_DAYS
```

Exact names remain provisional until implementation.

Provider API keys are host secrets. They do not enter persistence, API
responses, audit metadata, tracing fields, panic messages, or `Debug` output.
Prefer a secret wrapper with redacted `Debug` and zeroization on drop.

Deployment profiles:

- **Production server:** provider integration is an optional Cargo/runtime
  capability and supports configured outbound TLS endpoints.
- **Development:** an explicitly selected fake or local provider is supported.
- **Edge:** agents remain disabled unless a local endpoint or explicit cloud
  egress configuration is present. Agent unavailability does not affect device
  ingestion, rules, HTTP health, or database readiness.

## 15. HTTP API and streaming

All endpoints are under `/api/v1` and included in committed OpenAPI.

### 15.1 Definitions

```text
GET    /api/v1/agents
POST   /api/v1/agents
GET    /api/v1/agents/{agent_id}
POST   /api/v1/agents/{agent_id}/versions
POST   /api/v1/agents/{agent_id}/activate
POST   /api/v1/agents/{agent_id}/disable
```

Definition management is not needed for the first UI slice; the initial
operations investigator may be seeded as a system definition. The persistence
model should nevertheless be versioned from the start.

### 15.2 Runs

```text
POST   /api/v1/agent-runs
GET    /api/v1/agent-runs
GET    /api/v1/agent-runs/{run_id}
GET    /api/v1/agent-runs/{run_id}/events
GET    /api/v1/agent-runs/{run_id}/stream
POST   /api/v1/agent-runs/{run_id}/messages
POST   /api/v1/agent-runs/{run_id}/cancel
POST   /api/v1/agent-runs/{run_id}/tool-calls/{tool_call_id}/approval
```

Creating a run returns `202 Accepted` with its durable representation. It does
not keep an HTTP request open for model completion.

SSE is used for progress because execution is server-to-client event streaming
with ordinary REST commands in the other direction. The stream:

- authenticates and authorizes before reading any event;
- uses event sequence as SSE `id`;
- supports `Last-Event-ID` resume;
- replays from persistence before subscribing to live notifications;
- emits heartbeat comments without adding durable domain events;
- closes after a terminal event;
- never becomes the source of truth.

Polling `GET .../events?after_sequence=N` remains a complete fallback.

### 15.3 API errors

Add stable transport mappings for:

- agent not found;
- agent disabled;
- provider unavailable;
- invalid run transition;
- run or tenant budget exceeded;
- unknown or disabled tool;
- tool input invalid;
- approval required;
- approval expired;
- approval argument mismatch;
- approval already decided;
- run cancellation conflict;
- provider output invalid.

Do not expose raw provider response bodies or database errors.

## 16. Frontend experience

The initial UX has three entry points:

1. **Device context panel:** starts a run with the device as trusted subject.
2. **Operations / Agent runs page:** lists active, waiting, and completed runs.
3. **Approval inbox:** visible only to users with `agents.approve` and relevant
   target permissions.

The run timeline differentiates:

- user message;
- agent update;
- data lookup;
- evidence;
- approval request;
- approval decision;
- error or retry;
- final answer.

The UI must never imply that a proposed action has executed. Proposed,
awaiting approval, executing, succeeded, failed, and reconciliation-required
states use distinct text and icons, not color alone.

Accessibility requirements include keyboard-complete approval controls, focus
movement on newly inserted approval cards, live-region announcements for
terminal status, reduced-motion support, and readable structured tool results.

## 17. Observability and operations

### 17.1 Tracing

Create spans for:

- `agent.run`;
- `agent.model_turn`;
- `agent.tool_proposal`;
- `agent.tool_execution`;
- `agent.approval_wait`;
- `agent.compaction`.

Common fields include tenant-safe identifiers, run ID, agent/version ID, tool
name/version, provider profile, attempt, latency, result category, and token
usage. Tool arguments, model messages, telemetry values, and secrets are not
tracing fields.

### 17.2 Metrics

At minimum:

- active and queued runs;
- runs by terminal reason;
- run and model-turn latency;
- provider errors and throttling;
- tool calls by name and outcome;
- approval wait time and decision counts;
- input/output tokens and estimated cost where configured;
- budget exhaustion;
- lease recovery and duplicate suppression;
- redaction and context truncation counts;
- oldest queued run age.

Cardinality is bounded: do not use tenant ID, run ID, device ID, or provider
request ID as metric labels.

### 17.3 Operational controls

Administrators can:

- globally disable new runs;
- disable a definition without terminating history;
- cancel active runs;
- inspect provider health without revealing credentials;
- view queue and failure summaries;
- retry only explicitly retryable failed runs;
- drain workers for deployment;
- set per-installation and per-tenant concurrency/budget caps.

A kill switch prevents new model turns and tool executions while leaving the
rest of Extrittio operational.

## 18. Safety and threat model

The implementation must explicitly cover:

| Threat | Required control |
| --- | --- |
| Cross-tenant data access | Tenant from `TenantContext`; tenant-scoped repositories; indistinguishable not-found |
| Prompt injection in device data | Treat retrieved content as data; static system policy and tool registry |
| Model fabricates a tool | Static name/version lookup; reject unknown tools |
| Model broadens query | Typed arguments plus server-side time, page, point, and field limits |
| Privilege retained after role change | Rehydrate actor permissions immediately before each execution |
| Approval bait-and-switch | Canonical argument hash bound to approval |
| Duplicate physical action | End-to-end idempotency and durable target-domain outbox |
| SSRF or data exfiltration | No generic HTTP tool; explicit allowlisted integrations only |
| Secret leakage | Input/output redaction, secret wrappers, no secret logging or persistence |
| Denial of service or cost explosion | Run, step, token, result-size, concurrency, and wall-time budgets |
| Provider outage | Independent worker degradation and bounded backoff |
| Worker crash | Durable events, leases, idempotency, and reconciliation state |
| Misleading UI | Explicit proposal/execution states and evidence-backed answers |
| Audit gaps | Semantic audit at the application/tool boundary, not only HTTP middleware |

Before mutation tools ship, complete a focused abuse review covering malicious
telemetry/log content, compromised user sessions, concurrent approval races,
provider replay, stale permissions, and partial external delivery.

## 19. Relationship to rules and automation

Rules remain the deterministic, low-latency signal and action engine. Do not
invoke a model for every telemetry event.

The intended later flow is:

```text
telemetry/event
  -> deterministic rule evaluates
  -> durable "investigation requested" event
  -> agent run is created under an explicit service principal
  -> agent investigates
  -> agent proposes remediation
  -> human approval or bounded policy decides execution
```

This keeps ingestion latency, cost, and availability independent of the model
provider. Rule actions may trigger an agent run only after interactive runs,
budgets, service-principal authorization, and deduplication are proven.

## 20. Testing strategy

### 20.1 Core unit tests

Use fake repositories, clock, identifier source, and provider/tool results to
test:

- every legal and illegal run transition;
- permission-before-validation ordering where it avoids information leakage;
- effective-permission intersection;
- disabled/deleted initiator handling;
- approval hash matching, expiry, single use, and concurrent decisions;
- cancellation at each resumable state;
- step, token, cost, time, and tool-call budgets;
- unknown tools and tool versions;
- stable error vocabulary;
- immutable definition versions;
- agent actor provenance.

### 20.2 Shared adapter contract

Run the same suite against PostgreSQL and Turso:

- exact tenant isolation for every operation;
- atomic run creation plus initial event;
- monotonic event sequences under concurrency;
- one winner for run claims and approval decisions;
- lease renew, expiry, and recovery;
- idempotent tool-call reservation;
- argument hash and status constraints;
- terminal-state immutability;
- cancellation races;
- deterministic ordering and pagination;
- migration from the previous schema;
- cleanup/retention behavior;
- Turso backup and restore compatibility.

### 20.3 Host tests

- provider request/response normalization with a scripted fake server;
- timeout, cancellation, rate limiting, malformed output, and retry behavior;
- no credential material in errors, tracing, or `Debug` output;
- tool schema generation and typed deserialization;
- tool registry calls `Application`, not repositories;
- redaction and result-size enforcement;
- SSE replay, resume, live handoff, authorization, and terminal close;
- worker restart during model, read-tool, and approval phases;
- readiness remains healthy when an optional provider fails.

### 20.4 Security tests

- model-supplied tenant and permission fields are ignored or rejected;
- cross-tenant IDs never disclose existence;
- malicious prompt text in logs/telemetry cannot register or authorize tools;
- stale approval and changed-argument execution fail closed;
- user deactivation and permission removal stop an active run;
- prohibited secrets are absent from provider fixtures and persisted events;
- generic URL, SQL, Zenoh topic, and shell-shaped proposals have no execution
  path.

### 20.5 Evaluation suite

Create versioned, deterministic incident fixtures from synthetic device data.
Score outcomes rather than exact prose:

- correct evidence selection;
- diagnosis category;
- unsupported-claim rate;
- appropriate uncertainty;
- tool efficiency;
- refusal when evidence is insufficient;
- absence of prohibited action proposals;
- bounded token and latency use.

Provider/model changes run the same suite before activation. Store evaluation
results separately from production agent history.

## 21. Work packages

### A0 — Decisions, threat model, and baseline

**Deliverables**

- Accept or amend this design proposal.
- Record ADRs for identity delegation, approval policy, transcript retention,
  data egress, provider boundary, and command idempotency.
- Capture baseline backend verification and known failures.
- Define the synthetic evaluation fixture format.

**Exit criteria**

- Product owner approves the read-only first slice.
- Security owner approves provider data classes and redaction policy.
- No mutation tool is in scope.

### A1 — Prerequisite application slices

**Deliverables**

- Finish the active authentication-epoch security closure.
- Move the initial tool targets behind core application use cases: device read,
  assigned contract read, shadow read, bounded telemetry query, alert list,
  command list, and activity list.
- Move their repository ports and both adapter implementations according to the
  existing backend refactor plan.
- Add application DTOs that contain only model-safe projections where useful.

**Exit criteria**

- Each operation is reachable through `Application` with `TenantContext`.
- No agent code needs host `RepositorySet` access.
- Shared adapter contracts pass for both engines.
- Architecture allowance counts only decrease.

### A2 — Agent core domain

**Deliverables**

- Domain types, state machine, budgets, approvals, and repository contract.
- `AgentApplication` attached to the curated `Application` facade.
- `agents.*` permission keys and role behavior.
- Fake repository and exhaustive core tests.

**Exit criteria**

- Core compiles without forbidden runtime dependencies.
- Illegal transitions and stale authorization fail closed.
- Stable errors and permission ordering are documented and tested.

### A3 — PostgreSQL and Turso persistence

**Deliverables**

- Engine-owned migrations and repositories.
- Shared adapter contract suite.
- Lease, sequence, idempotency, and approval concurrency implementation.
- Retention queries and indexes.

**Exit criteria**

- Both adapters pass identical semantic tests.
- Previous-snapshot migrations and Turso backup/restore pass.
- Query plans/index evidence exists for queue claims and event pagination.

### A4 — Provider boundary and durable worker

**Deliverables**

- `ModelGateway`, scripted fake, and first configured provider adapter.
- Prompt assembly, redaction, error classification, backoff, and cancellation.
- Agent worker in `WorkerSupervisor`.
- Per-installation concurrency and budget enforcement.
- Provider-disabled and provider-unhealthy operational behavior.

**Exit criteria**

- Runs resume after injected crashes at every step boundary.
- Duplicate read calls are suppressed.
- Provider failure does not fail core readiness.
- No provider type leaks into core or adapters.

### A5 — Read-only tool registry

**Deliverables**

- Static versioned descriptors for the initial tool set.
- Typed validation and server-side limits.
- Evidence handles and safe projections.
- Semantic audit records for proposals and executions.

**Exit criteria**

- Tools invoke only `Application` use cases.
- Cross-tenant, oversized, and unknown-tool tests pass.
- Every factual fixture answer can resolve its cited evidence.

### A6 — HTTP, OpenAPI, and frontend

**Deliverables**

- Run creation, list, detail, events, cancel, and SSE endpoints.
- Committed OpenAPI and regenerated frontend types.
- Device context panel, run timeline, evidence display, and run history.
- Permission-aware navigation and disabled-state messaging.

**Exit criteria**

- SSE resume has no gap between persisted replay and live events.
- Full keyboard and screen-reader path is verified.
- Refreshing the browser reconstructs the entire visible run from persistence.
- Backend and frontend verification scopes pass.

### A7 — Evaluation and read-only release

**Deliverables**

- Synthetic incident corpus and scoring harness.
- Operational dashboards and alerts.
- Data-egress disclosure and administrator setup documentation.
- Feature flag rollout and kill-switch runbook.

**Exit criteria**

- Quality, unsupported-claim, latency, and cost thresholds are approved.
- Security abuse cases pass.
- Read-only feature is usable on PostgreSQL and Turso profiles where configured.

### A8 — Durable mutation foundation

This package starts only after A7 is stable.

**Deliverables**

- Move mutation targets behind core application use cases.
- Add durable command dispatch rather than database-create plus direct Zenoh
  publish.
- Add end-to-end idempotency through target-domain outboxes.
- Implement approval UI and compare-and-set endpoints.
- Add the first single-device reversible mutation tool.

**Exit criteria**

- Crash/retry tests prove at-most-once externally observable intent or safe
  reconciliation.
- Approval argument binding and live permission rechecks pass.
- Kill switch blocks execution without losing queued approvals.

### A9 — Event-triggered investigations

**Deliverables**

- Explicit agent service principals and least-privilege policy.
- Rule/outbox event to run creation mapping.
- Deduplication, cooldowns, tenant quotas, and incident correlation.
- Human review of proposed remediation.

**Exit criteria**

- No model call occurs on the device-ingress critical path.
- Trigger storms remain within configured queue and cost bounds.
- Disabling the provider or agent feature does not impede deterministic rules.

## 22. Pull request slicing

Keep each pull request buildable and reversible. A recommended sequence is:

1. ADRs and architecture checks only.
2. One prerequisite read-domain migration per pull request.
3. Agent core types and pure state-machine tests.
4. Repository contract plus PostgreSQL adapter.
5. Turso adapter and shared contract activation.
6. Fake provider and worker skeleton.
7. One read-only tool at a time.
8. REST run endpoints.
9. SSE and frontend timeline.
10. Evaluation harness and documentation.
11. Feature-flagged read-only release.

Do not combine provider integration, database migrations, the complete tool
catalog, and frontend work in one change.

## 23. Architecture enforcement changes

Extend `cargo xtask architecture` so that:

- provider crates are forbidden in core and storage adapters;
- `crates/backend/src/agents/tools/**` cannot reference `.persistence`,
  `RepositorySet`, Diesel, Turso, or Zenoh;
- HTTP agent routes cannot reference repositories;
- only the agent worker may invoke `ModelGateway`;
- only the registry may bind tool descriptors to application use cases;
- no agent code references `DEFAULT_TENANT_ID`;
- no agent code introduces broad environment reads outside host configuration;
- new workspace dependencies respect the established package graph.

Avoid adding temporary exceptions for new agent code. Prerequisite domains move
behind their target boundary before they become tools.

## 24. Verification commands

During implementation, the minimum relevant checks are:

```bash
cargo xtask architecture
cargo test -p extrittio-backend-core
cargo test -p extrittio-backend-adapter-tests
cargo test -p extrittio-backend --all-features
cargo xtask verify backend
cargo xtask verify frontend
```

Provider integration tests use a local scripted server and must not require
network access or real credentials in CI.

## 25. Rollout and compatibility

- Ship schema and disabled runtime code first.
- Keep `EXTRITTIO_AGENTS_ENABLED=false` as the initial default.
- Enable for development tenants with a fake/local provider.
- Enable read-only production runs for a small tenant allowlist.
- Review cost, provider errors, evidence quality, and unsupported claims.
- Expand read-only availability.
- Treat mutation tools as a separate release with separate approval.

Disabling or rolling back the runtime leaves agent tables intact. Older binaries
ignore those tables. Published agent-definition versions and completed events
remain immutable compatibility records.

## 26. Open decisions

The following must be decided in A0 and recorded as ADRs:

1. Which provider capability is implemented first and whether production
   supports more than one provider initially.
2. Which data fields are permitted to leave an installation by default.
3. Whether prompts/tool results require application-layer encryption at rest in
   addition to database/storage controls.
4. Exact transcript and evidence retention periods.
5. Per-tenant quota ownership and whether estimated monetary cost is enforced
   or only token budgets are enforced initially.
6. Whether the operations investigator is a seeded immutable system definition
   or an ordinary versioned tenant definition with protected defaults.
7. The first reversible mutation eligible for A8.
8. Whether approval requires a distinct human for user-initiated runs.
9. Whether Edge supports only local model endpoints or also explicit cloud
   provider egress.
10. The quality and unsupported-claim thresholds required for production.

None of these decisions blocks A1 prerequisite domain extraction. Provider and
data-egress decisions block A4; approval and command-idempotency decisions block
A8.

## 27. Definition of done

The agent capability is considered architecturally complete when:

- model providers are optional host adapters;
- the core state machine and authorization compile independently of runtime
  libraries;
- PostgreSQL and Turso satisfy the same agent repository contract;
- every tool is typed, versioned, bounded, tenant-scoped, and routed through
  `Application`;
- runs survive restart and have deterministic cancellation and retry behavior;
- read-only answers carry resolvable evidence;
- mutation tools, if enabled, require argument-bound approval and durable
  idempotent delivery;
- agent-originated actions have complete actor provenance and semantic audit;
- provider failure cannot interrupt ingestion, deterministic rules, HTTP core
  APIs, or device operations;
- architecture, security, adapter, API, frontend, and evaluation gates pass;
- deployment, data-egress, retention, incident, and kill-switch documentation
  is published.

