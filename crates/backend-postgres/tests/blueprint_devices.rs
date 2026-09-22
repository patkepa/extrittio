use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::{BigInt, Jsonb, Text};
use extrittio_backend_core::TenantId;
use extrittio_backend_core::certificates::NewDeviceCertificateRecord;
use extrittio_backend_core::devices::{
    CreateDeviceRecord, DeviceFilter, DeviceListQuery, DeviceRepository, NewDeviceContractRecord,
    UpdateDeviceRecord,
};
use extrittio_backend_core::events::{
    DeviceEventRepository, DeviceLocationQuery, DeviceMetricQuery, DeviceMetricSample, MetricValue,
    RecordDeviceEvent,
};
use extrittio_backend_core::rule_engine::{cache::RuleCache, types::TelemetryData};
use extrittio_backend_core::rule_snapshots::{
    DeviceRuleEvaluation, RuleEvaluationInput, RuleEvaluationSnapshot,
};
use extrittio_backend_core::{CiIngestOutcome, CiIngestParams, CiIngestRepository};
use extrittio_backend_postgres::{
    PostgresCiIngestRepository, PostgresDeviceRepository, PostgresEventRepository,
    run_pending_migrations,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn count(connection: &mut PgConnection, table: &str, tenant: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) AS count FROM {table} WHERE tenant_id = $1");
    diesel::sql_query(sql)
        .bind::<Text, _>(tenant)
        .get_result::<Count>(connection)
        .unwrap()
        .count
}

fn seed_blueprint(connection: &mut PgConnection, tenant: &str, revision: &str) {
    diesel::sql_query("INSERT INTO organizations (id, name) VALUES ($1, $1)")
        .bind::<Text, _>(tenant)
        .execute(connection)
        .unwrap();
    diesel::sql_query(
        "INSERT INTO device_blueprints (id, tenant_id, blueprint_key, name)
         VALUES ($1, $2, 'test-blueprint', 'Test Blueprint')",
    )
    .bind::<Text, _>(format!("{tenant}-blueprint"))
    .bind::<Text, _>(tenant)
    .execute(connection)
    .unwrap();
    diesel::sql_query(
        "INSERT INTO device_blueprint_revisions
         (id, tenant_id, blueprint_id, revision, document, document_hash, compatibility)
         VALUES ($1, $2, $3, 1, $4, repeat('a', 64), '{}')",
    )
    .bind::<Text, _>(revision)
    .bind::<Text, _>(tenant)
    .bind::<Text, _>(format!("{tenant}-blueprint"))
    .bind::<Jsonb, _>(
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../blueprints/environment-sensor.create-request.json"
        ))
        .unwrap(),
    )
    .execute(connection)
    .unwrap();
}

fn new_device(id: &str, revision: &str) -> CreateDeviceRecord {
    CreateDeviceRecord {
        id: id.into(),
        name: id.into(),
        fleet_id: None,
        firmware: "1.0".into(),
        contract: NewDeviceContractRecord {
            id: format!("{id}-contract"),
            blueprint_revision_id: revision.into(),
            document: json!({
                "deviceId": id,
                "location": {
                    "coordinateSystem": "wgs84",
                    "unit": "degrees",
                    "stream": "position",
                    "latitudePath": "/latitude",
                    "longitudePath": "/longitude",
                    "maxAgeMs": 5000
                }
            }),
            initial_configuration: Some(json!({"reporting": true})),
            contract_hash: "b".repeat(64),
            created_at: Utc::now(),
        },
    }
}

fn event(
    tenant: &TenantId,
    device_id: &str,
    contract_id: &str,
    event_id: &str,
) -> RecordDeviceEvent {
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
    RecordDeviceEvent {
        event_id: event_id.into(),
        device_id: device_id.into(),
        contract_id: contract_id.into(),
        route_key: "sample".into(),
        occurred_at: now,
        received_at: now,
        payload: json!({"value": 1}),
        metrics: vec![
            DeviceMetricSample {
                stream_key: "position".into(),
                field_path: "/latitude".into(),
                value: MetricValue::Float64(0.0),
            },
            DeviceMetricSample {
                stream_key: "position".into(),
                field_path: "/longitude".into(),
                value: MetricValue::Float64(0.0),
            },
        ],
        rule_evaluation: DeviceRuleEvaluation {
            snapshot: RuleEvaluationSnapshot::new(Arc::new(RuleCache::default())),
            tenant: tenant.clone(),
            device_id: device_id.into(),
            fleet_id: None,
            blueprint_id: None,
            input: RuleEvaluationInput::Telemetry {
                data: TelemetryData {
                    latitude: None,
                    longitude: None,
                    metrics: Default::default(),
                },
                geofence: false,
            },
            observed_at: now.naive_utc(),
        },
    }
}

#[tokio::test]
async fn postgres_blueprint_device_and_ci_contracts_when_configured() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL blueprint device contract: DATABASE_URL is not set");
        return;
    };
    let pool = Pool::builder()
        .max_size(3)
        .build(ConnectionManager::<PgConnection>::new(url))
        .expect("connect to disposable PostgreSQL database");
    let suffix = Uuid::new_v4().simple().to_string();
    let tenant_a = format!("device-test-a-{suffix}");
    let tenant_b = format!("device-test-b-{suffix}");
    let revision_a = format!("revision-a-{suffix}");
    let revision_b = format!("revision-b-{suffix}");
    let device_a = format!("device-a-{suffix}");
    let device_b = format!("device-b-{suffix}");
    {
        let mut connection = pool.get().unwrap();
        run_pending_migrations(&mut connection).unwrap();
        seed_blueprint(&mut connection, &tenant_a, &revision_a);
        seed_blueprint(&mut connection, &tenant_b, &revision_b);
    }
    let repository = PostgresDeviceRepository::from_pool(pool.clone());
    let a = TenantId::new(tenant_a.clone()).unwrap();
    let b = TenantId::new(tenant_b.clone()).unwrap();
    let certificate = NewDeviceCertificateRecord {
        device_id: device_a.clone(),
        private_key_pem: "key".into(),
        certificate_pem: "certificate".into(),
        fingerprint: format!("fingerprint-{suffix}"),
        expires_at: Utc::now() + Duration::days(1),
    };
    let created = repository
        .create(&a, new_device(&device_a, &revision_a), Some(certificate))
        .await
        .unwrap();
    assert_eq!(created.blueprint.revision_id, revision_a);
    assert_eq!(created.device.id, device_a);
    assert_eq!(repository.get(&a, &device_a).await.unwrap(), Some(created));
    assert!(repository.get(&b, &device_a).await.unwrap().is_none());
    assert_eq!(
        repository
            .assigned_contract(&a, &device_a)
            .await
            .unwrap()
            .unwrap()
            .assignment_status,
        "pending"
    );
    assert!(
        repository
            .create(
                &a,
                new_device(&format!("invalid-{suffix}"), &revision_b),
                None
            )
            .await
            .is_err()
    );
    assert_eq!(count(&mut pool.get().unwrap(), "devices", &tenant_a), 1);
    assert_eq!(
        count(&mut pool.get().unwrap(), "device_contracts", &tenant_a),
        1
    );
    assert_eq!(
        repository
            .list(
                &a,
                DeviceListQuery {
                    status: None,
                    search: Some("device-a".into()),
                    fleet_id: None,
                    limit: 10,
                    offset: 0,
                }
            )
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        repository
            .resolve_ids(
                &a,
                DeviceFilter {
                    status: None,
                    search: None,
                    fleet_id: None,
                }
            )
            .await
            .unwrap(),
        vec![device_a.clone()]
    );
    let updated = repository
        .update(
            &a,
            &device_a,
            UpdateDeviceRecord {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.device.name, "Renamed");
    assert!(
        repository
            .update(
                &b,
                &device_a,
                UpdateDeviceRecord {
                    name: Some("Foreign".into()),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .is_none()
    );
    repository
        .create(&b, new_device(&device_b, &revision_b), None)
        .await
        .unwrap();
    let event_repository = PostgresEventRepository::from_pool(pool.clone());
    let accepted = event(
        &a,
        &device_a,
        &format!("{device_a}-contract"),
        &format!("accepted-{suffix}"),
    );
    let observed_at = accepted.occurred_at;
    assert!(
        event_repository
            .record(&a, accepted.clone())
            .await
            .unwrap()
            .recorded
    );
    assert!(
        !event_repository
            .record(&a, accepted)
            .await
            .unwrap()
            .recorded
    );
    let metrics = event_repository
        .list_metrics(
            &a,
            &device_a,
            DeviceMetricQuery {
                stream_key: Some("position".into()),
                field_path: None,
                since: None,
                before: None,
                limit: 10,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(metrics.len(), 2);
    assert!(
        metrics
            .iter()
            .all(|metric| metric.contract_id == format!("{device_a}-contract"))
    );
    assert_eq!(
        event_repository
            .list_metrics(
                &b,
                &device_a,
                DeviceMetricQuery {
                    stream_key: None,
                    field_path: None,
                    since: None,
                    before: None,
                    limit: 10,
                }
            )
            .await
            .unwrap(),
        None
    );
    let location = event_repository
        .latest_location(
            &a,
            &device_a,
            DeviceLocationQuery {
                contract_id: format!("{device_a}-contract"),
                stream_key: "position".into(),
                latitude_path: "/latitude".into(),
                longitude_path: "/longitude".into(),
                since: observed_at - Duration::seconds(4),
                now: observed_at + Duration::seconds(1),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!((location.latitude, location.longitude), (0.0, 0.0));
    assert_eq!(location.expires_at, observed_at + Duration::seconds(5));
    let batch = event_repository
        .latest_locations(
            &a,
            vec![device_a.clone(), "missing".into()],
            observed_at + Duration::seconds(1),
        )
        .await
        .unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(
        batch[0].location.expires_at,
        observed_at + Duration::seconds(5)
    );
    assert!(
        event_repository
            .latest_locations(
                &a,
                vec![device_a.clone()],
                observed_at + Duration::seconds(6)
            )
            .await
            .unwrap()
            .is_empty()
    );
    {
        let mut connection = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO device_events
             (id, tenant_id, device_id, contract_id, route_key, occurred_at, payload)
             VALUES ($1, $2, $3, $4, 'sample', now(), '{}')",
        )
        .bind::<Text, _>(format!("event-{suffix}"))
        .bind::<Text, _>(&tenant_a)
        .bind::<Text, _>(&device_a)
        .bind::<Text, _>(format!("{device_a}-contract"))
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO device_metric_samples
             (event_id, tenant_id, device_id, stream_key, field_path, value_type, value_double, occurred_at)
             VALUES ($1, $2, $3, 'sample', '/value', 'float64', 1.0, now())",
        )
        .bind::<Text, _>(format!("event-{suffix}"))
        .bind::<Text, _>(&tenant_a)
        .bind::<Text, _>(&device_a)
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO device_contracts
             (id, tenant_id, device_id, blueprint_revision_id, document, contract_hash)
             VALUES ($1, $2, $3, $4, '{}', repeat('c', 64))",
        )
        .bind::<Text, _>(format!("replacement-{suffix}"))
        .bind::<Text, _>(&tenant_a)
        .bind::<Text, _>(&device_a)
        .bind::<Text, _>(&revision_a)
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "UPDATE device_contract_assignments SET desired_contract_id = $1
             WHERE tenant_id = $2 AND device_id = $3",
        )
        .bind::<Text, _>(format!("replacement-{suffix}"))
        .bind::<Text, _>(&tenant_a)
        .bind::<Text, _>(&device_a)
        .execute(&mut connection)
        .unwrap();
    }
    assert!(
        event_repository
            .record(
                &a,
                event(
                    &a,
                    &device_a,
                    &format!("{device_a}-contract"),
                    &format!("stale-{suffix}"),
                )
            )
            .await
            .is_err()
    );
    assert_eq!(
        count(&mut pool.get().unwrap(), "device_events", &tenant_a),
        2
    );
    assert!(!repository.delete(&b, &device_a).await.unwrap());
    assert_eq!(
        repository
            .bulk_delete(&b, vec![device_a.clone()])
            .await
            .unwrap(),
        0
    );
    assert!(repository.delete(&a, &device_a).await.unwrap());
    assert!(!repository.delete(&a, &device_a).await.unwrap());
    {
        let mut connection = pool.get().unwrap();
        for table in [
            "devices",
            "device_contracts",
            "device_contract_assignments",
            "device_events",
            "device_metric_samples",
            "device_configs",
            "device_certificates",
            "device_shadows",
        ] {
            assert_eq!(count(&mut connection, table, &tenant_a), 0, "{table}");
        }
    }
    assert_eq!(
        repository
            .bulk_delete(&b, vec![device_b.clone(), device_b.clone()])
            .await
            .unwrap(),
        1
    );
    assert_eq!(repository.bulk_delete(&b, vec![device_b]).await.unwrap(), 0);
    let key_hash = format!("ci-key-{suffix}");
    {
        let mut connection = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO api_keys (tenant_id, name, key_hash, key_prefix, blueprint_id)
             VALUES ($1, 'CI', $2, 'test', $3)",
        )
        .bind::<Text, _>(&tenant_a)
        .bind::<Text, _>(&key_hash)
        .bind::<Text, _>(format!("{tenant_a}-blueprint"))
        .execute(&mut connection)
        .unwrap();
    }
    let ci = PostgresCiIngestRepository::from_pool(pool.clone());
    let request = |revision: &str, version: &str| CiIngestParams {
        blueprint_revision_id: revision.into(),
        version: version.into(),
        artifact_url: "https://example.test/firmware.bin".into(),
        sha256: Some("a".repeat(64)),
        commit_sha: None,
        branch: None,
        ci_run_url: None,
        build_timestamp: None,
        description: None,
        changelog: None,
    };
    assert!(matches!(
        ci.ingest_ci("unknown", request(&revision_a, "1"))
            .await
            .unwrap(),
        CiIngestOutcome::Unauthorized
    ));
    assert!(matches!(
        ci.ingest_ci(&key_hash, request(&revision_b, "1"))
            .await
            .unwrap(),
        CiIngestOutcome::BlueprintRevisionNotFound
    ));
    let alternate_revision = format!("alternate-revision-{suffix}");
    {
        let mut connection = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO device_blueprints (id, tenant_id, blueprint_key, name)
             VALUES ($1, $2, 'alternate', 'Alternate')",
        )
        .bind::<Text, _>(format!("alternate-{suffix}"))
        .bind::<Text, _>(&tenant_a)
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO device_blueprint_revisions
             (id, tenant_id, blueprint_id, revision, document, document_hash, compatibility)
             VALUES ($1, $2, $3, 1, $4, repeat('d', 64), '{}')",
        )
        .bind::<Text, _>(&alternate_revision)
        .bind::<Text, _>(&tenant_a)
        .bind::<Text, _>(format!("alternate-{suffix}"))
        .bind::<Jsonb, _>(
            serde_json::from_str::<serde_json::Value>(include_str!(
                "../../../blueprints/environment-sensor.create-request.json"
            ))
            .unwrap(),
        )
        .execute(&mut connection)
        .unwrap();
    }
    assert!(matches!(
        ci.ingest_ci(&key_hash, request(&alternate_revision, "1"))
            .await
            .unwrap(),
        CiIngestOutcome::Forbidden { .. }
    ));
    assert!(matches!(
        ci.ingest_ci(&key_hash, request(&revision_a, "1"))
            .await
            .unwrap(),
        CiIngestOutcome::Created { .. }
    ));
    assert!(
        ci.ingest_ci(&key_hash, request(&revision_a, "1"))
            .await
            .is_err()
    );
    assert_eq!(
        count(&mut pool.get().unwrap(), "firmware_updates", &tenant_a),
        1
    );
}
