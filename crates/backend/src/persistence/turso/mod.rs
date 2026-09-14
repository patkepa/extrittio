mod activity;
mod analytics;
mod audit;
mod dashboard;
mod database;
mod firmware;
mod metrics;
mod row;

use std::sync::Arc;

use crate::persistence::{DatabaseRuntime, RepositorySet};

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
    let api_keys = crate::database::turso_api_keys(&database);
    let telemetry = crate::database::turso_telemetry(&database);
    let device_ingress = crate::database::turso_device_ingress(&database);
    let events = crate::database::turso_events(&database);
    let logs = crate::database::turso_logs(&database);
    let commands = crate::database::turso_commands(&database);
    let configuration = crate::database::turso_configuration(&database);
    let shadows = crate::database::turso_shadows(&database);
    let fleets = crate::database::turso_fleets(&database);
    let device_blueprints = crate::database::turso_device_blueprints(&database);
    let alerts = crate::database::turso_alerts(&database);
    let outbox = crate::database::turso_outbox(&database);
    let rules = crate::database::turso_rules(&database);
    let devices = crate::database::turso_devices(&database);
    let device_types = crate::database::turso_device_types(&database);
    let bootstrap = crate::database::turso_bootstrap(&database);
    let certificates = crate::database::turso_certificates(&database);
    let ci_ingest = crate::database::turso_ci_ingest(&database);
    let adapter = Arc::new(TursoAdapter::new(database));
    RepositorySet {
        activity: adapter.clone(),
        analytics: adapter.clone(),
        api_keys,
        ci_ingest,
        alerts,
        audit: adapter.clone(),
        bootstrap,
        certificates,
        commands,
        configuration,
        dashboard: adapter.clone(),
        device_blueprints,
        device_types,
        devices,
        device_ingress,
        events,
        fleets,
        firmware: adapter.clone(),
        logs,
        metrics: adapter.clone(),
        outbox,
        roles,
        rule_zone_snapshots,
        rules,
        shadows,
        telemetry,
        users,
        zones,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::activity::{repository::ActivityRepository, types::ActivityQuery};
    use crate::domains::audit::{port::AuditRepository, types::NewAuditEventRecord};
    use crate::tenancy::{DEFAULT_TENANT_ID as TEST_TENANT_ID, TenantId};
    use serde_json::json;
    use std::time::Duration;

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
    async fn activity_totals_remain_stable_beyond_the_final_page() {
        let (_directory, adapter) = adapter().await;
        let tenant = TenantId::new(TEST_TENANT_ID).unwrap();
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
}
