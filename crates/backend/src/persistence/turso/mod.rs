mod activity;
mod alerts;
mod analytics;
mod api_keys;
mod audit;
mod bootstrap;
mod certificates;
mod commands;
mod configuration;
mod dashboard;
mod database;
mod device_blueprints;
mod device_types;
mod devices;
mod events;
mod firmware;
mod fleets;
mod logs;
mod metrics;
mod outbox;
mod row;
mod rules;
mod shadows;
mod telemetry;

use std::sync::Arc;

use crate::persistence::{DatabaseRuntime, RepositoryPorts, RepositorySet};

pub use database::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabase, TursoDatabaseInfo};

#[derive(Clone)]
pub struct TursoAdapter {
    database: Arc<TursoDatabase>,
}

impl TursoAdapter {
    #[must_use]
    pub fn new(database: Arc<TursoDatabase>) -> Self {
        Self { database }
    }
}

#[must_use]
pub fn create_repositories(database: Arc<TursoDatabase>) -> RepositorySet {
    build_repositories(database)
}

#[must_use]
pub fn create_runtime(database: Arc<TursoDatabase>) -> DatabaseRuntime {
    let repositories = build_repositories(database.clone());
    DatabaseRuntime::turso(repositories, database)
}

fn build_repositories(database: Arc<TursoDatabase>) -> RepositorySet {
    let (zones, rule_zone_snapshots) = crate::database::turso_zones(&database);
    let roles = crate::database::turso_roles(&database);
    let users = crate::database::turso_users(&database);
    let adapter = Arc::new(TursoAdapter::new(database));
    RepositorySet::new(RepositoryPorts {
        activity: adapter.clone(),
        analytics: adapter.clone(),
        api_keys: adapter.clone(),
        alerts: adapter.clone(),
        audit: adapter.clone(),
        bootstrap: adapter.clone(),
        certificates: adapter.clone(),
        commands: adapter.clone(),
        configuration: adapter.clone(),
        dashboard: adapter.clone(),
        device_blueprints: adapter.clone(),
        device_types: adapter.clone(),
        devices: adapter.clone(),
        events: adapter.clone(),
        fleets: adapter.clone(),
        firmware: adapter.clone(),
        logs: adapter.clone(),
        metrics: adapter.clone(),
        outbox: adapter.clone(),
        roles,
        rule_zone_snapshots,
        rules: adapter.clone(),
        shadows: adapter.clone(),
        telemetry: adapter.clone(),
        users,
        zones,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::domains::activity::repository::ActivityRepository;
    use crate::domains::activity::types::ActivityQuery;
    use crate::domains::alerts::port::AlertRepository;
    use crate::domains::alerts::types::NewAlertRecord;
    use crate::domains::analytics::repository::AnalyticsRepository;
    use crate::domains::analytics::types::{
        AnalyticsMetric, AnalyticsMetricSelector, AnalyticsQuery, AnalyticsScope,
    };
    use crate::domains::audit::port::AuditRepository;
    use crate::domains::audit::types::NewAuditEventRecord;
    use crate::domains::commands::port::CommandRepository;
    use crate::domains::commands::types::NewCommandRecord;
    use crate::domains::configuration::repository::DeviceConfigRepository;
    use crate::domains::configuration::types::MergeDeviceConfigOutcome;
    use crate::domains::device_blueprints::repository::DeviceBlueprintRepository;
    use crate::domains::device_blueprints::types::{
        CreateBlueprintRecord, PublishBlueprintOutcome, PublishBlueprintRecord,
        ReplaceBlueprintDraftRecord,
    };
    use crate::domains::device_types::repository::DeviceTypeRepository;
    use crate::domains::device_types::types::CreateDeviceTypeRecord;
    use crate::domains::devices::repository::DeviceRepository;
    use crate::domains::devices::types::{
        CreateDeviceRecord, DeviceListQuery, NewDeviceContractRecord,
    };
    use crate::domains::events::repository::DeviceEventRepository;
    use crate::domains::events::types::{
        DeviceMetricQuery, DeviceMetricSample, MetricValue, RecordDeviceEvent,
    };
    use crate::domains::firmware::port::FirmwareRepository;
    use crate::domains::firmware::types::{NewFirmwareRecord, TriggerOtaOutcome};
    use crate::domains::fleets::repository::FleetRepository;
    use crate::domains::fleets::types::CreateFleetRecord;
    use crate::domains::identity::api_key_repository::ApiKeyRepository;
    use crate::domains::identity::api_key_types::CreateApiKeyRecord;
    use crate::domains::identity::certificate_repository::CertificateRepository;
    use crate::domains::identity::certificate_types::{
        NewCaCertificateRecord, NewDeviceCertificateRecord, ReplaceCertificateOutcome,
    };
    use crate::domains::logs::port::LogRepository;
    use crate::domains::operations::metrics_repository::MetricsRepository;
    use crate::domains::operations::metrics_types::NewAppMetricRecord;
    use crate::domains::operations::outbox_repository::OutboxRepository;
    use crate::domains::rules::port::RuleRepository;
    use crate::domains::rules::types::{
        NewRuleRecord, RuleActionRecord, RuleConditionRecord, RuleFilter,
    };
    use crate::domains::shadows::repository::ShadowRepository;
    use crate::domains::telemetry::port::TelemetryRepository;
    use crate::domains::telemetry::types::{TelemetryQuery, TelemetryWrite};
    use crate::persistence::{BootstrapOwner, BootstrapRepository, BuiltinDeviceType};
    use crate::tenancy::{DEFAULT_TENANT_ID, TenantId};
    use extrittio_backend_core::{CreateUserOutcome, EncodedPasswordHash, NewUser, UserRepository};

    async fn adapter() -> (tempfile::TempDir, TursoAdapter) {
        let directory = tempfile::tempdir().unwrap();
        let database = TursoDatabase::open(
            directory.path(),
            &directory.path().join("extrittio.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        (directory, TursoAdapter::new(database))
    }

    fn blueprint_document(key: &str, name: &str) -> serde_json::Value {
        json!({
            "apiVersion": "extrittio.io/v1alpha1",
            "kind": "DeviceBlueprint",
            "metadata": {"key": key, "name": name},
            "spec": {
                "runtime": {
                    "minimumContractApi": 1,
                    "heartbeat": {"interval": "30s", "offlineAfter": "95s"},
                    "limits": {"maxMessageBytes": 8192, "maxMessagesPerMinute": 120}
                },
                "streams": [{
                    "key": "environment",
                    "route": "readings",
                    "fields": [{
                        "path": "/temperature",
                        "type": "float64",
                        "label": "Temperature",
                        "unit": "Cel",
                        "index": true,
                        "aggregates": ["min", "max", "avg"],
                        "presentation": {"precision": 2}
                    }]
                }]
            }
        })
    }

    #[tokio::test]
    async fn activity_totals_remain_stable_beyond_the_final_page() {
        let (_directory, adapter) = adapter().await;
        let tenant = TenantId::new(DEFAULT_TENANT_ID).unwrap();
        for index in 1..=2 {
            AuditRepository::record(
                &adapter,
                &tenant,
                NewAuditEventRecord {
                    id: format!("audit-{index}"),
                    actor_type: "user".into(),
                    actor_id: Some("owner".into()),
                    action: "device.read".into(),
                    resource_type: "device".into(),
                    resource_id: Some(format!("device-{index}")),
                    outcome: "success".into(),
                    request_id: format!("request-{index}"),
                    metadata: json!({}),
                },
            )
            .await
            .unwrap();
        }

        for (offset, expected_len, expected_total) in [(1, 1, 2), (2, 0, 2), (3, 0, 2)] {
            let page = ActivityRepository::list(
                &adapter,
                &tenant,
                ActivityQuery {
                    source: Some("audit".into()),
                    severity: None,
                    category: None,
                    device_id: None,
                    search: None,
                    since: None,
                    until: None,
                    limit: 1,
                    offset,
                },
            )
            .await
            .unwrap();
            assert_eq!(page.data.len(), expected_len);
            assert_eq!(page.total, expected_total);
        }

        let empty = ActivityRepository::list(
            &adapter,
            &tenant,
            ActivityQuery {
                source: Some("alert".into()),
                severity: None,
                category: None,
                device_id: None,
                search: None,
                since: None,
                until: None,
                limit: 1,
                offset: 2,
            },
        )
        .await
        .unwrap();
        assert!(empty.data.is_empty());
        assert_eq!(empty.total, 0);
    }

    #[tokio::test]
    async fn device_blueprint_draft_and_revision_round_trip() {
        let (_directory, adapter) = adapter().await;
        let tenant = TenantId::new(DEFAULT_TENANT_ID).unwrap();
        let now = Utc::now();
        let (created, draft) = DeviceBlueprintRepository::create(
            &adapter,
            &tenant,
            CreateBlueprintRecord {
                id: "blueprint-1".into(),
                draft_id: "draft-1".into(),
                key: "sensor".into(),
                name: "Sensor".into(),
                description: None,
                document: blueprint_document("sensor", "Sensor"),
                now,
            },
        )
        .await
        .unwrap();
        assert_eq!(created.latest_revision, None);

        let updated_at = now + chrono::Duration::seconds(1);
        let updated = DeviceBlueprintRepository::replace_draft(
            &adapter,
            &tenant,
            &created.id,
            ReplaceBlueprintDraftRecord {
                key: "sensor".into(),
                name: "Updated Sensor".into(),
                description: Some("updated".into()),
                document: blueprint_document("sensor", "Updated Sensor"),
                now: updated_at,
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_ne!(draft.updated_at, updated.updated_at);

        let published = DeviceBlueprintRepository::publish(
            &adapter,
            &tenant,
            &created.id,
            PublishBlueprintRecord {
                revision_id: "revision-1".into(),
                expected_draft_updated_at: updated.updated_at,
                document: updated.document,
                document_hash: "a".repeat(64),
                compatibility: json!({"breaking": false, "changes": []}),
                now: updated_at + chrono::Duration::seconds(1),
            },
        )
        .await
        .unwrap();
        let PublishBlueprintOutcome::Published(revision) = published else {
            panic!("expected published revision");
        };
        assert_eq!(revision.revision, 1);
        assert_eq!(
            DeviceBlueprintRepository::get(&adapter, &tenant, &created.id)
                .await
                .unwrap()
                .unwrap()
                .latest_revision,
            Some(1)
        );

        let device_type = DeviceTypeRepository::create(
            &adapter,
            &tenant,
            CreateDeviceTypeRecord {
                name: "contract-device".into(),
                icon: "cube".into(),
                color_hex: "#112233".into(),
            },
        )
        .await
        .unwrap();
        DeviceRepository::create(
            &adapter,
            &tenant,
            CreateDeviceRecord {
                id: "device-contract-1".into(),
                name: "Contract Device".into(),
                device_type_id: device_type.id,
                fleet_id: None,
                firmware: "v1".into(),
                contract: Some(NewDeviceContractRecord {
                    id: "contract-1".into(),
                    blueprint_revision_id: revision.id.clone(),
                    document: json!({"contractApi": 1, "deviceId": "device-contract-1"}),
                    contract_hash: "b".repeat(64),
                    created_at: updated_at + chrono::Duration::seconds(2),
                }),
            },
            None,
        )
        .await
        .unwrap();
        let assigned = DeviceRepository::assigned_contract(&adapter, &tenant, "device-contract-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(assigned.id, "contract-1");
        assert_eq!(assigned.assignment_status, "pending");
        assert_eq!(assigned.document["contractApi"], 1);
        let ingress = DeviceRepository::ingress_context(
            &adapter,
            &crate::tenancy::DeviceIdentity::new(tenant.as_str(), "device-contract-1").unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(ingress.blueprint_id.as_deref(), Some(created.id.as_str()));

        let firmware = FirmwareRepository::create(
            &adapter,
            &tenant,
            NewFirmwareRecord {
                device_type_id: device_type.id,
                version: "2.0.0".into(),
                url: "firmware/contract-device.bin".into(),
                sha256: Some("c".repeat(64)),
                description: None,
                commit_sha: None,
                branch: None,
                ci_run_url: None,
                build_timestamp: None,
                changelog: None,
                source: None,
                blueprint_revision_id: Some(revision.id.clone()),
                compatibility: json!({"board": "test-board"}),
                update_strategy: Some("binary_replacement".into()),
            },
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(firmware.blueprint_revision_id, Some(revision.id.clone()));
        assert_eq!(firmware.compatibility["board"], "test-board");
        assert!(matches!(
            FirmwareRepository::trigger_ota(
                &adapter,
                &tenant,
                "device-contract-1",
                firmware.id,
                "https://hub.example.test",
            )
            .await
            .unwrap(),
            TriggerOtaOutcome::Ready { .. }
        ));
        let incompatible_firmware = FirmwareRepository::create(
            &adapter,
            &tenant,
            NewFirmwareRecord {
                device_type_id: device_type.id,
                version: "3.0.0".into(),
                url: "firmware/wrong-contract.bin".into(),
                sha256: Some("d".repeat(64)),
                description: None,
                commit_sha: None,
                branch: None,
                ci_run_url: None,
                build_timestamp: None,
                changelog: None,
                source: None,
                blueprint_revision_id: Some("another-revision".into()),
                compatibility: json!({}),
                update_strategy: Some("binary_replacement".into()),
            },
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(matches!(
            FirmwareRepository::trigger_ota(
                &adapter,
                &tenant,
                "device-contract-1",
                incompatible_firmware.id,
                "https://hub.example.test",
            )
            .await
            .unwrap(),
            TriggerOtaOutcome::Incompatible
        ));

        let occurred_at = updated_at + chrono::Duration::seconds(3);
        let event = RecordDeviceEvent {
            event_id: "event-1".into(),
            device_id: "device-contract-1".into(),
            contract_id: "contract-1".into(),
            route_key: "readings".into(),
            occurred_at,
            received_at: occurred_at,
            payload: json!({"temperature": 21.5}),
            metrics: vec![DeviceMetricSample {
                stream_key: "environment".into(),
                field_path: "/temperature".into(),
                value: MetricValue::Float64(21.5),
            }],
            pending_actions: Vec::new(),
        };
        let outcome = DeviceEventRepository::record(&adapter, &tenant, event.clone())
            .await
            .unwrap();
        assert!(outcome.recorded);
        assert_eq!(outcome.metrics_recorded, 1);
        assert!(
            !DeviceEventRepository::record(&adapter, &tenant, event)
                .await
                .unwrap()
                .recorded
        );
        let metrics = DeviceEventRepository::list_metrics(
            &adapter,
            &tenant,
            "device-contract-1",
            DeviceMetricQuery {
                stream_key: Some("environment".into()),
                field_path: Some("/temperature".into()),
                since: None,
                before: None,
                limit: 10,
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].event_id, "event-1");
        assert_eq!(metrics[0].value, MetricValue::Float64(21.5));
        assert!(
            DeviceEventRepository::list_metrics(
                &adapter,
                &TenantId::new("another-tenant").unwrap(),
                "device-contract-1",
                DeviceMetricQuery {
                    stream_key: None,
                    field_path: None,
                    since: None,
                    before: None,
                    limit: 10,
                },
            )
            .await
            .unwrap()
            .is_none()
        );
        let acknowledged =
            DeviceRepository::assigned_contract(&adapter, &tenant, "device-contract-1")
                .await
                .unwrap()
                .unwrap();
        assert_eq!(acknowledged.assignment_status, "converged");
        assert_eq!(acknowledged.id, "contract-1");
    }

    #[tokio::test]
    async fn analytics_catalogs_and_queries_blueprint_metrics() {
        let (_directory, adapter) = adapter().await;
        let tenant = TenantId::new(DEFAULT_TENANT_ID).unwrap();
        let published_at = Utc::now();
        let (blueprint, draft) = DeviceBlueprintRepository::create(
            &adapter,
            &tenant,
            CreateBlueprintRecord {
                id: "analytics-blueprint".into(),
                draft_id: "analytics-draft".into(),
                key: "cold-room".into(),
                name: "Cold room sensor".into(),
                description: None,
                document: blueprint_document("cold-room", "Cold room sensor"),
                now: published_at,
            },
        )
        .await
        .unwrap();
        let published = DeviceBlueprintRepository::publish(
            &adapter,
            &tenant,
            &blueprint.id,
            PublishBlueprintRecord {
                revision_id: "analytics-revision".into(),
                expected_draft_updated_at: draft.updated_at,
                document: draft.document,
                document_hash: "a".repeat(64),
                compatibility: json!({"breaking": false, "changes": []}),
                now: published_at + chrono::Duration::seconds(1),
            },
        )
        .await
        .unwrap();
        let PublishBlueprintOutcome::Published(revision) = published else {
            panic!("expected published analytics blueprint");
        };
        let device_type = DeviceTypeRepository::create(
            &adapter,
            &tenant,
            CreateDeviceTypeRecord {
                name: "temperature sensor".into(),
                icon: "heatmap".into(),
                color_hex: "#ff7a00".into(),
            },
        )
        .await
        .unwrap();
        DeviceRepository::create(
            &adapter,
            &tenant,
            CreateDeviceRecord {
                id: "analytics-device".into(),
                name: "Cold room".into(),
                device_type_id: device_type.id,
                fleet_id: None,
                firmware: "v1".into(),
                contract: Some(NewDeviceContractRecord {
                    id: "analytics-contract".into(),
                    blueprint_revision_id: revision.id,
                    document: json!({"contractApi": 1, "deviceId": "analytics-device"}),
                    contract_hash: "b".repeat(64),
                    created_at: published_at + chrono::Duration::seconds(2),
                }),
            },
            None,
        )
        .await
        .unwrap();

        let now = Utc::now()
            .date_naive()
            .and_hms_opt(12, 10, 0)
            .expect("test timestamp is valid");
        for (index, temperature) in [21.5, 23.5].into_iter().enumerate() {
            let occurred_at = now + chrono::Duration::minutes(i64::try_from(index).unwrap());
            DeviceEventRepository::record(
                &adapter,
                &tenant,
                RecordDeviceEvent {
                    event_id: format!("analytics-event-{index}"),
                    device_id: "analytics-device".into(),
                    contract_id: "analytics-contract".into(),
                    route_key: "readings".into(),
                    occurred_at: occurred_at.and_utc(),
                    received_at: occurred_at.and_utc(),
                    payload: json!({"temperature": temperature}),
                    metrics: vec![DeviceMetricSample {
                        stream_key: "environment".into(),
                        field_path: "/temperature".into(),
                        value: MetricValue::Float64(temperature),
                    }],
                    pending_actions: Vec::new(),
                },
            )
            .await
            .unwrap();
        }

        let catalog = AnalyticsRepository::blueprint_catalog(&adapter, &tenant)
            .await
            .unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].blueprint_id, blueprint.id);

        let metric = AnalyticsMetric {
            selector: AnalyticsMetricSelector {
                blueprint_id: blueprint.id,
                stream_key: "environment".into(),
                field_path: "/temperature".into(),
            },
            blueprint_key: "cold-room".into(),
            blueprint_name: "Cold room sensor".into(),
            label: "Temperature".into(),
            unit: Some("Cel".into()),
            value_type: "float64".into(),
            aggregates: vec!["minimum".into(), "maximum".into(), "average".into()],
            precision: Some(2),
        };
        let analytics_query = AnalyticsQuery {
            scope: AnalyticsScope {
                device_type_ids: vec![device_type.id],
                fleet_ids: Vec::new(),
                device_ids: Vec::new(),
            },
            metric,
            start: now - chrono::Duration::hours(1),
            end: now + chrono::Duration::hours(2),
            bucket_seconds: 3_600,
            max_devices: 50,
            max_rows: 100,
        };
        let result = AnalyticsRepository::query(&adapter, &tenant, analytics_query)
            .await
            .unwrap();
        assert_eq!(result.selected_devices, 1);
        assert_eq!(result.compatible_devices, 1);
        assert_eq!(
            result
                .buckets
                .iter()
                .map(|bucket| bucket.sample_count)
                .sum::<i64>(),
            2
        );
        assert!((result.buckets[0].average - 22.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn identity_and_catalog_foundation_round_trip() {
        let (_directory, adapter) = adapter().await;
        let tenant = TenantId::new(DEFAULT_TENANT_ID).unwrap();
        adapter
            .seed_owner_if_empty(
                &tenant,
                BootstrapOwner {
                    username: "owner".into(),
                    password_hash: "owner-hash".into(),
                },
            )
            .await
            .unwrap();
        let users = crate::database::turso_users(&adapter.database);
        let created_user = users
            .create(
                &tenant,
                NewUser {
                    username: "viewer".into(),
                    password_hash: EncodedPasswordHash::new("viewer-hash"),
                    role_ids: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(created_user, CreateUserOutcome::Created(_)));
        adapter
            .seed_builtin_device_types(
                &tenant,
                vec![BuiltinDeviceType {
                    name: "default".into(),
                    icon: "cube".into(),
                    color_hex: "#8ABBFF".into(),
                }],
            )
            .await
            .unwrap();
        let device_type = DeviceTypeRepository::create(
            &adapter,
            &tenant,
            CreateDeviceTypeRecord {
                name: "sensor".into(),
                icon: "heatmap".into(),
                color_hex: "#112233".into(),
            },
        )
        .await
        .unwrap();
        let fleet =
            FleetRepository::create(&adapter, &tenant, CreateFleetRecord { name: "lab".into() })
                .await
                .unwrap();
        let key = ApiKeyRepository::create(
            &adapter,
            &tenant,
            CreateApiKeyRecord {
                name: "ci".into(),
                key_hash: "hash".into(),
                key_prefix: "prefix".into(),
                device_type_id: Some(device_type.id),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            ApiKeyRepository::list(&adapter, &tenant).await.unwrap()[0]
                .key
                .id,
            key.id
        );

        DeviceRepository::create(
            &adapter,
            &tenant,
            CreateDeviceRecord {
                id: "device-1".into(),
                name: "Device".into(),
                device_type_id: device_type.id,
                fleet_id: Some(fleet.id),
                firmware: "v1".into(),
                contract: None,
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            DeviceRepository::list(
                &adapter,
                &tenant,
                DeviceListQuery {
                    status: None,
                    search: Some("device".into()),
                    fleet_id: Some(fleet.id),
                    limit: 10,
                    offset: 0,
                },
            )
            .await
            .unwrap()
            .total,
            1
        );
        let patch = json!({"rate": 10});
        let merged = adapter
            .merge_for_device(
                &tenant,
                "device-1",
                patch.as_object().unwrap().clone(),
                Utc::now(),
            )
            .await
            .unwrap();
        assert!(matches!(merged, MergeDeviceConfigOutcome::Updated(_)));
        let shadow = adapter
            .update_desired(
                &tenant,
                "device-1",
                patch.as_object().unwrap().clone(),
                Utc::now(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(shadow.version, 2);

        let identity = crate::tenancy::DeviceIdentity::new(tenant.as_str(), "device-1").unwrap();
        assert!(
            LogRepository::record(&adapter, &identity, "INFO".into(), "ready".into())
                .await
                .unwrap()
        );
        assert!(
            CommandRepository::create(
                &adapter,
                &tenant,
                "device-1",
                NewCommandRecord {
                    id: "command-1".into(),
                    command: "reboot".into(),
                    params: "{}".into(),
                },
            )
            .await
            .unwrap()
            .is_some()
        );
        let activity = ActivityRepository::list(
            &adapter,
            &tenant,
            ActivityQuery {
                source: None,
                severity: None,
                category: None,
                device_id: Some("device-1".into()),
                search: None,
                since: None,
                until: None,
                limit: 10,
                offset: 0,
            },
        )
        .await
        .unwrap();
        assert_eq!(activity.total, 2);
        assert!(activity.data.iter().any(|event| event.source == "device"));
        assert!(activity.data.iter().any(|event| event.source == "command"));
        let observed_at = Utc::now().naive_utc();
        let telemetry = TelemetryRepository::record(
            &adapter,
            &identity,
            TelemetryWrite {
                expected_device_type_id: device_type.id,
                expected_fleet_id: Some(fleet.id),
                payload: vec![1, 2, 3],
                temperature: Some(21.5),
                humidity: Some(45.0),
                battery_level: Some(90.0),
                custom_json: Some(json!({"kind": "sample"})),
                latitude: Some(52.0),
                longitude: Some(21.0),
                speed: None,
                altitude: None,
                heading: None,
                pending_actions: Vec::new(),
                observed_at,
            },
        )
        .await
        .unwrap();
        assert!(telemetry.recorded);
        let samples = TelemetryRepository::list(
            &adapter,
            &tenant,
            "device-1",
            TelemetryQuery {
                since: None,
                before: None,
                limit: 10,
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].temperature, Some(21.5));
        let maintenance = TelemetryRepository::maintain(
            &adapter,
            observed_at - chrono::Duration::hours(1),
            observed_at + chrono::Duration::hours(1),
            observed_at - chrono::Duration::days(1),
        )
        .await
        .unwrap();
        assert_eq!(maintenance.rollups_upserted, 1);
        let alert = AlertRepository::create(
            &adapter,
            &tenant,
            NewAlertRecord {
                id: "alert-1".into(),
                rule_id: None,
                device_id: "device-1".into(),
                severity: "warning".into(),
                message: "threshold".into(),
                triggered_value: Some("21.5".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(alert.status, "active");
        let rule = RuleRepository::create(
            &adapter,
            &tenant,
            NewRuleRecord {
                id: "rule-1".into(),
                name: "temperature".into(),
                description: Some("hot".into()),
                trigger_type: "telemetry".into(),
                target_type: "global".into(),
                target_id: None,
                cooldown_seconds: 60,
                conditions: vec![RuleConditionRecord {
                    id: "condition-1".into(),
                    field: "temperature".into(),
                    operator: "gt".into(),
                    value: "30".into(),
                    condition_group: 0,
                    zone_id: None,
                }],
                actions: vec![RuleActionRecord {
                    id: "action-1".into(),
                    action_type: "alert".into(),
                    config: json!({"severity":"warning"}),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(rule.conditions.len(), 1);
        assert_eq!(
            RuleRepository::list(
                &adapter,
                &tenant,
                RuleFilter {
                    enabled: Some(true),
                    trigger_type: None,
                    target_type: None,
                },
            )
            .await
            .unwrap()
            .len(),
            1
        );
        assert_eq!(
            RuleRepository::build_cache(&adapter)
                .await
                .unwrap()
                .global_rules
                .len(),
            1
        );

        let firmware = FirmwareRepository::create(
            &adapter,
            &tenant,
            NewFirmwareRecord {
                device_type_id: device_type.id,
                version: "1.0.0".into(),
                url: "firmware/sensor.bin".into(),
                sha256: Some("abcd".into()),
                description: None,
                commit_sha: None,
                branch: None,
                ci_run_url: None,
                build_timestamp: None,
                changelog: None,
                source: None,
                blueprint_revision_id: None,
                compatibility: serde_json::json!({}),
                update_strategy: None,
            },
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            FirmwareRepository::next_version(&adapter, &tenant, device_type.id)
                .await
                .unwrap(),
            "1.0.1"
        );
        let ota = FirmwareRepository::trigger_ota(
            &adapter,
            &tenant,
            "device-1",
            firmware.id,
            "http://localhost:8080",
        )
        .await
        .unwrap();
        assert!(matches!(ota, TriggerOtaOutcome::Ready { .. }));
        assert_eq!(
            FirmwareRepository::list_all_deployments(&adapter, &tenant, None, 10, 0)
                .await
                .unwrap()
                .total,
            1
        );
        MetricsRepository::insert_app(
            &adapter,
            NewAppMetricRecord {
                request_count: 1,
                error_count: 0,
                avg_latency_ms: 2.0,
                p95_latency_ms: 3.0,
                db_pool_active: 1,
                db_pool_idle: 0,
                zenoh_messages_in: 4,
                zenoh_messages_out: 5,
            },
        )
        .await
        .unwrap();
        assert!(
            MetricsRepository::current(&adapter)
                .await
                .unwrap()
                .app
                .is_some()
        );
        let recurring_action = crate::rule_engine::types::PendingAction::SendCommand {
            tenant_id: tenant.as_str().into(),
            device_id: "device-1".into(),
            command: "reboot".into(),
            params: json!({}),
        };
        let outbox_connection = adapter.database.connect().unwrap();
        assert_eq!(
            super::devices::enqueue(&outbox_connection, &[recurring_action.clone()])
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            super::devices::enqueue(&outbox_connection, &[recurring_action.clone()])
                .await
                .unwrap(),
            0,
            "retrying an active occurrence must remain idempotent"
        );
        let claimed =
            OutboxRepository::claim_batch(&adapter, "worker-1", 10, Duration::from_secs(30))
                .await
                .unwrap();
        assert_eq!(claimed.len(), 1);
        let first_occurrence_id = claimed[0].id.clone();
        assert!(
            OutboxRepository::mark_succeeded(&adapter, &claimed[0].id, "worker-1")
                .await
                .unwrap()
        );
        assert_eq!(
            super::devices::enqueue(&outbox_connection, &[recurring_action])
                .await
                .unwrap(),
            1,
            "a completed occurrence must not suppress a later recurrence"
        );
        let recurrence =
            OutboxRepository::claim_batch(&adapter, "worker-2", 10, Duration::from_secs(30))
                .await
                .unwrap();
        assert_eq!(recurrence.len(), 1);
        assert_ne!(recurrence[0].id, first_occurrence_id);
        assert!(
            OutboxRepository::mark_succeeded(&adapter, &recurrence[0].id, "worker-2")
                .await
                .unwrap()
        );

        let ca = adapter
            .insert_ca_if_absent(NewCaCertificateRecord {
                private_key_pem: "key".into(),
                certificate_pem: "cert".into(),
            })
            .await
            .unwrap();
        assert_eq!(ca.certificate_pem, "cert");
        let replaced = adapter
            .replace_device_certificate(
                &tenant,
                NewDeviceCertificateRecord {
                    device_id: "device-1".into(),
                    private_key_pem: "device-key".into(),
                    certificate_pem: "device-cert".into(),
                    fingerprint: "fingerprint".into(),
                    expires_at: Utc::now() + chrono::Duration::days(1),
                },
            )
            .await
            .unwrap();
        assert!(matches!(replaced, ReplaceCertificateOutcome::Replaced(_)));
    }
}
