# Backend boundary implementation audit

Source audit: 2026-09-14. This records R11/R12 implementation closure with runtime
acceptance deferred by the user's test policy. It does not claim verified release
readiness. The chronological [execution record](backend-refactor-execution-plan.md)
contains the individual migration checks and behavior-preservation decisions.

## R11: entry points and workers

| Requirement | Current evidence | Assessment |
| --- | --- | --- |
| Routes invoke applications rather than repositories | `api/mod.rs` registers domain routers. Host domain imports now use core application/record types; the architecture guard recursively rejects RepositorySet, host persistence/database, and concrete adapter imports throughout every domain file. AppState exposes Application but no repository collection. | Implemented; guard covers all domain files, not only earlier allowlisted handlers. |
| Residual multi-operation business coordination moves to core | `application/bulk_devices.rs` owns target resolution and sequential bulk command/OTA outcomes. `application/shadows.rs` coordinates committed reported-shadow changes with firmware report processing. `application/rule_delivery.rs` owns action decoding/execution/result classification. | Implemented; host retains DTO construction, protocol output, and diagnostics. |
| Workers receive narrow capabilities | `app/workers.rs::WorkerApplications` constructs named applications once. `background.rs` receives maintenance/ingress applications, and subscribers receive DeviceMessageApplications. Handlers take individual relevant capabilities. | Implemented; state fields and worker capability fields are private. |
| Host owns runtime scheduling and shutdown | `app/workers.rs` owns worker registration, cancellation, supervision, and maintenance checkpoint. `service.rs::Service` owns boot/serve/drain orchestration. | Implemented; cancellation and failure behavior need future runtime verification. |
| Preserve middleware and public transport contracts | `app/http.rs` retains CORS, metrics, tracing, rate limiting, authentication, audit, and response layers. Routes retain wire DTOs; subscriber handlers retain Protobuf/JSON decoding and publishing. The documented username correction and epoch-presence guard are explicit exceptions. | Source preservation recorded per slice; HTTP/OpenAPI/wire parity remains deferred acceptance. |
| Webhook SDK/security policy stays outbound | `outbound/webhook.rs` validates public HTTPS and resolved targets, pins addresses, disables redirects, keeps ten-second timeouts and delivery headers, and conditionally injects OTLP context. Core depends on WebhookSender. | Implemented; DNS, redirects, failures, and retry behavior remain runtime evidence tasks. |
| Remove direct mutable cache/repository access | Core RepositorySet and host AppState fields are private. The retained rule snapshot accessor serves supervisor/diagnostic needs; it exposes immutable definitions/metrics, not process-local rule correctness state. Runtime rule transitions are adapter-owned. | Implemented; snapshot polling and stale-state failure evidence remains deferred. |

The host's remaining service modules format device values, hash uploaded bytes,
translate application errors, publish post-commit deltas, and report storage cleanup
diagnostics. They contain no business repository implementations. Host operational
routes inspect runtime/environment state intentionally; that is distinct from core
business policy. The core/adapters dependency and source guards reject transport
SDK leakage and adapter-to-host dependencies.

## R12: composition, process shell, and isolation

| Requirement | Current evidence | Assessment |
| --- | --- | --- |
| One selected adapter and shared engine handles | `persistence/factory.rs` selects exactly the requested feature/configuration. Adapter builders create core RepositorySetInput from shared pools/handles; DatabaseComposition consumes the paired ports/runtime once. | Implemented; no fallback engine or second business aggregate. |
| Lifecycle is separate from business ports | `persistence/runtime.rs` contains only descriptor/lifecycle. `database/postgres.rs` and `database/turso.rs` map adapter lifecycle results. No SQL remains in host lifecycle code. | Implemented. |
| Private runtime and transport substates | `state.rs` separates HTTP, messaging, observability, operational state, application, and workers. All AppState fields are private; callers borrow accessors. | Implemented; construction input remains explicit. |
| CLI is a process shell | `apps/extrittio/src/commands/service.rs` translates flags/configuration/output and calls host Service/provisioning/maintenance/OpenThread operations. It no longer imports adapters, ports, or OpenThread runtime. | Implemented. |
| Preserve service sequencing | Service::start boots before worker startup. Service::run awaits HTTP then worker shutdown/checkpoint. CLI observability shutdown follows; server errors retain precedence. Boot errors retain their early-return behavior. | Source-checked; executable failure/shutdown acceptance deferred. |
| Remove bridges and obsolete paths | Host db/repositories SQL modules, domain port/type aliases, migration-bridge features, raw writer bridge, and direct Diesel/Turso dependencies are deleted. Adapter models/schema are private. | Implemented; verifier forbids reintroduced bridge features and direct host/CLI engine dependencies. |
| One executable owner | Backend disables automatic binaries and retains only explicit OpenAPI generation. The CLI owns serve/run and allocator selection. | Implemented; Cargo target metadata is checked by the verifier. |
| Feature isolation | Production/edge dependency closure rules exclude the opposite database engine. Combined, edge, production, and no-adapter CLI builds plus no-adapter OpenAPI compilation have passed in recorded slices. | Compilation/dependency evidence, not deployed artifact/runtime evidence. |
| Resolve old EX register | EX-002–EX-015 no longer require structural allowances. Four exact default-tenant locations remain bounded; three represent intended definition/bootstrap/login behavior. | Implemented except EX-001 rollout retirement, explicitly deferred below. |

EX-001 removal requires one supported token lifetime without compatibility hits
after all issuers write tenant claims. No deployment telemetry was available here.
The mapper is kept epoch-bound and fail-closed; this is preserved rollout behavior,
not a claim that the retirement criterion passed. See [identity closure](backend-identity-closure.md).

## Remaining release proof

R14 remains postponed in full. Public API/device/CLI compatibility, cancellation,
concurrency, migrations, restored backups, clean-checkout builds, and production
artifact/runtime comparisons remain deferred. Formatting, compilation, and source
checks cannot substitute for them. No new feature flow, test runner, database
operation, or deployment was added by this audit.
