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
mod fleets;
mod logs;
mod roles;
mod row;
mod shadows;
mod telemetry;
mod users;

use std::sync::Arc;

pub use database::TursoDatabase;

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::domains::commands::port::CommandRepository;
    use crate::domains::commands::types::NewCommandRecord;
    use crate::domains::configuration::repository::DeviceConfigRepository;
    use crate::domains::configuration::types::MergeDeviceConfigOutcome;
    use crate::domains::device_types::repository::DeviceTypeRepository;
    use crate::domains::device_types::types::CreateDeviceTypeRecord;
    use crate::domains::devices::repository::DeviceRepository;
    use crate::domains::devices::types::{CreateDeviceRecord, DeviceListQuery};
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
