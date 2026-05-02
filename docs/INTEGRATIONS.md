# External Integration Opportunities

Extrittio should stay focused as the IoT control plane: device identity, fleet state,
telemetry ingestion, shadows, commands, rules, OTA coordination, and the operator UI.
External integrations should sit around that core so deployments can plug into
existing infrastructure without turning the backend into every possible broker,
analytics database, identity provider, and workflow engine.

This document lists services and open-source projects that are good candidates for
optional integrations. It is not a committed roadmap.

## Highest-Value Integrations

### MQTT Brokers and Gateways

Use an MQTT broker or gateway so existing devices can connect without adopting Zenoh directly.

Good candidates:

- [EMQX](https://docs.emqx.com/en/) for a production MQTT broker with bridges, rule processing, and industrial IoT ecosystem support.
- [Mosquitto](https://mosquitto.org/) for a small, familiar MQTT broker.
- [NATS MQTT](https://docs.nats.io/running-a-nats-service/configuration/mqtt) for teams already using NATS or JetStream.

Likely Extrittio integration shape:

- Translate MQTT topics into Extrittio device telemetry, heartbeat, command response, shadow report, and log topics.
- Publish Extrittio commands and shadow deltas back to MQTT topics.
- Preserve device identity and auth mapping at the gateway boundary.

### Observability

Expose backend, frontend, and device-pipeline health through standard observability tooling.

Good candidates:

- [OpenTelemetry Collector](https://opentelemetry.io/docs/collector/components/) for traces, metrics, and logs pipelines.
- [Prometheus](https://prometheus.io/) for metrics scraping and alerting.
- [Grafana](https://grafana.com/docs/grafana/latest/datasources/) for dashboards and exploration.
- [Loki](https://grafana.com/oss/loki/) for log aggregation.
- [Tempo](https://grafana.com/oss/tempo/) or [Jaeger](https://www.jaegertracing.io/) for traces.

Likely Extrittio integration shape:

- Add OpenTelemetry tracing around REST handlers, database calls, Zenoh handlers, rule evaluation, and background tasks.
- Export Prometheus-compatible metrics for API latency, ingestion rate, device status transitions, rule execution, command success, OTA progress, and database pool health.
- Route structured application logs to Loki or any OpenTelemetry log backend.

### Identity and SSO

Support external identity providers for production deployments instead of relying only on local users.

Good candidates:

- [Keycloak](https://www.keycloak.org/securing-apps/oidc-layers) for self-hosted OpenID Connect and SAML.
- Auth0, Okta, or Microsoft Entra ID for managed enterprise SSO.

Likely Extrittio integration shape:

- Add OIDC login for the frontend.
- Verify external JWTs in the backend.
- Map external groups or claims to Extrittio roles and permissions.
- Keep local auth useful for development and small self-hosted installs.

### Time-Series and Analytics Storage

Keep PostgreSQL as the system-of-record for relational state, but optionally export high-volume telemetry to systems designed for long historical scans and aggregates.

Good candidates:

- [ClickHouse](https://clickhouse.com/resources/engineering/what-is-time-series-database) for large-scale analytical telemetry queries.
- [TimescaleDB](https://www.tigerdata.com/timescaledb) when teams want time-series features while staying close to PostgreSQL.
- [InfluxDB](https://www.influxdata.com/) or [QuestDB](https://questdb.io/) for dedicated time-series workloads.

Likely Extrittio integration shape:

- Stream normalized telemetry events into the external store.
- Keep recent or operational telemetry in PostgreSQL if needed by the product UI.
- Query external stores for long-range charts, rollups, exports, and analytics dashboards.

### Object Storage

Use object storage for binary and archival data rather than storing large artifacts in PostgreSQL.

Good candidates:

- Amazon S3, Google Cloud Storage, Azure Blob Storage, or another cloud object store.
- [MinIO](https://miniodocs.cc/reference/s3-api-compatibility) for self-hosted S3-compatible storage.

Likely Extrittio integration shape:

- Store firmware artifacts, OTA bundles, log archives, crash dumps, exports, and large device uploads.
- Keep metadata, rollout state, access control, and audit records in PostgreSQL.
- Generate short-lived signed URLs for upload and download.

### Automation and Workflow Tools

Let users send Extrittio events into existing operational workflows.

Good candidates:

- [n8n](https://n8n.io/), [Node-RED](https://nodered.org/), Zapier, Make, generic webhooks.
- Slack, Discord, PagerDuty, Opsgenie, email, and incident-management systems.

Likely Extrittio integration shape:

- Add rule actions for webhook delivery and notification targets.
- Include event types for telemetry thresholds, device online/offline transitions, alert lifecycle changes, command failures, and OTA failures.
- Track delivery attempts and failures so automation is observable.

### Streaming and Data Pipelines

Export events to broader data platforms for downstream processing.

Good candidates:

- [Apache Kafka](https://kafka.apache.org/25/kafka-connect/overview/) or Redpanda.
- NATS JetStream.
- Apache Pulsar.

Likely Extrittio integration shape:

- Publish telemetry, device status, command, alert, and audit events to configured streams.
- Use stable event schemas and versioning.
- Treat streaming export as an optional sink, not a replacement for Extrittio's operational database.

### OTA Ecosystem

Extrittio can own OTA coordination, but some deployments already have update tooling that should be reused.

Good candidates:

- [Mender](https://docs.mender.io/) for device update management and fleet rollout workflows.
- Eclipse hawkBit for software update rollout management.
- RAUC or SWUpdate for embedded Linux update mechanics.

Likely Extrittio integration shape:

- Import firmware release and deployment state from external OTA systems.
- Trigger deployments from Extrittio rules or UI actions.
- Keep device inventory and fleet targeting aligned between systems.

### Industrial and Edge Protocol Bridges

Industrial deployments often need protocol translation near the device or gateway.

Good candidates:

- Node-RED for low-code edge flows.
- EMQX Neuron or EdgeX Foundry for industrial protocol ingestion.
- OPC UA, Modbus, CAN, BLE, serial, and custom gateway agents.

Likely Extrittio integration shape:

- Treat protocol gateways as Extrittio devices or fleet gateways.
- Normalize protocol-specific payloads into Extrittio telemetry and shadow updates.
- Preserve source metadata so operators can trace readings back to the original bus, register, node, or endpoint.

### Cloud IoT Interop

Cloud bridges are useful for migration, hybrid deployments, and customers that already depend on cloud data services.

Good candidates:

- AWS IoT Core.
- Azure IoT Hub or Azure Event Hubs.
- Google Pub/Sub.

Likely Extrittio integration shape:

- Bridge selected telemetry, device status, and command events.
- Import or mirror device registry metadata where appropriate.
- Keep cloud interop optional so self-hosted deployments remain clean.

## Suggested Build Order

1. MQTT bridge: unlocks compatibility with the largest set of existing IoT devices.
2. OpenTelemetry, Prometheus, and Grafana: makes the platform operable in production.
3. OIDC and Keycloak-compatible SSO: makes enterprise and multi-user deployments credible.
4. S3 or MinIO artifact storage: keeps firmware and large files out of PostgreSQL.
5. ClickHouse or Kafka export: supports high-volume telemetry analytics and downstream pipelines.
