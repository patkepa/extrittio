mod alerts;
mod api_keys;
mod audit;
mod bootstrap;
mod certificates;
mod commands;
mod configuration;
mod dashboard;
mod database;
mod device_types;
mod devices;
mod firmware;
mod fleets;
mod logs;
mod metrics;
mod outbox;
mod roles;
mod row;
mod rules;
mod shadows;
mod telemetry;
mod users;
mod zones;

use std::sync::Arc;

use crate::persistence::{BackendDescriptor, Persistence, PersistencePorts};

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
pub fn create_persistence(database: Arc<TursoDatabase>) -> Persistence {
    let path = database.path().to_path_buf();
    let adapter = Arc::new(TursoAdapter::new(database));
    Persistence::new(
        BackendDescriptor::turso(path),
        PersistencePorts {
            api_keys: adapter.clone(),
            alerts: adapter.clone(),
            audit: adapter.clone(),
            bootstrap: adapter.clone(),
            certificates: adapter.clone(),
            commands: adapter.clone(),
            configuration: adapter.clone(),
            dashboard: adapter.clone(),
            device_types: adapter.clone(),
            devices: adapter.clone(),
            fleets: adapter.clone(),
            firmware: adapter.clone(),
            logs: adapter.clone(),
            metrics: adapter.clone(),
            outbox: adapter.clone(),
            roles: adapter.clone(),
            rules: adapter.clone(),
            shadows: adapter.clone(),
            telemetry: adapter.clone(),
            users: adapter.clone(),
            zones: adapter,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::domains::alerts::port::AlertRepository;
    use crate::domains::alerts::types::NewAlertRecord;
    use crate::domains::commands::port::CommandRepository;
    use crate::domains::commands::types::NewCommandRecord;
    use crate::domains::configuration::repository::DeviceConfigRepository;
    use crate::domains::configuration::types::MergeDeviceConfigOutcome;
    use crate::domains::device_types::repository::DeviceTypeRepository;
    use crate::domains::device_types::types::CreateDeviceTypeRecord;
    use crate::domains::devices::repository::DeviceRepository;
    use crate::domains::devices::types::{CreateDeviceRecord, DeviceListQuery};
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
    use crate::domains::identity::user_repository::UserRepository;
    use crate::domains::identity::user_types::{CreateUserOutcome, CreateUserRecord};
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
        let created_user = UserRepository::create(
            &adapter,
            &tenant,
            CreateUserRecord {
                username: "viewer".into(),
                password_hash: "viewer-hash".into(),
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
                declared_connections: None,
                observed_network_hosts: None,
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
        super::devices::enqueue(
            &adapter.database.connect().unwrap(),
            &[crate::rule_engine::types::PendingAction::SendCommand {
                tenant_id: tenant.as_str().into(),
                device_id: "device-1".into(),
                command: "reboot".into(),
                params: json!({}),
            }],
        )
        .await
        .unwrap();
        let claimed =
            OutboxRepository::claim_batch(&adapter, "worker-1", 10, Duration::from_secs(30))
                .await
                .unwrap();
        assert_eq!(claimed.len(), 1);
        assert!(
            OutboxRepository::mark_succeeded(&adapter, &claimed[0].id, "worker-1")
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
