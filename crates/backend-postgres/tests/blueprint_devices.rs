use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::{BigInt, Float8, Jsonb, Text};
use extrittio_backend_core::TenantId;
use extrittio_backend_core::analytics::{
    AnalyticsMetric, AnalyticsMetricSelector, AnalyticsQuery, AnalyticsRepository, AnalyticsScope,
};
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
    PostgresAnalyticsRepository, PostgresCiIngestRepository, PostgresDeviceRepository,
    PostgresEventRepository, run_pending_migrations,
};
use serde_json::json;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct Rollup {
    #[diesel(sql_type = BigInt)]
    sample_count: i64,
    #[diesel(sql_type = Float8)]
    value_sum: f64,
    #[diesel(sql_type = Float8)]
    value_min: f64,
    #[diesel(sql_type = Float8)]
    value_max: f64,
    #[diesel(sql_type = Float8)]
    latest_value: f64,
    #[diesel(sql_type = diesel::sql_types::Timestamptz)]
    latest_at: chrono::DateTime<Utc>,
    #[diesel(sql_type = Text)]
    latest_event_id: String,
}

static BASELINE_READY: OnceLock<()> = OnceLock::new();

fn migrate_once(pool: &Pool<ConnectionManager<PgConnection>>) {
    BASELINE_READY.get_or_init(|| {
        run_pending_migrations(&mut pool.get().unwrap()).unwrap();
    });
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
        migrate_once(&pool);
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
    let rollup = diesel::sql_query(
        "SELECT sample_count, value_sum, value_min, value_max, latest_value, latest_at, latest_event_id
         FROM device_metric_rollups_hourly
         WHERE tenant_id = $1 AND device_id = $2 AND blueprint_revision_id = $3
           AND stream_key = 'position' AND field_path = '/latitude'",
    )
    .bind::<Text, _>(&tenant_a)
    .bind::<Text, _>(&device_a)
    .bind::<Text, _>(&revision_a)
    .get_result::<Rollup>(&mut pool.get().unwrap())
    .unwrap();
    assert_eq!(rollup.sample_count, 1);
    assert_eq!(rollup.latest_at, observed_at);
    assert_eq!(rollup.latest_event_id, format!("accepted-{suffix}"));
    assert_eq!(
        (
            rollup.value_sum,
            rollup.value_min,
            rollup.value_max,
            rollup.latest_value
        ),
        (0.0, 0.0, 0.0, 0.0)
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
    let bucket_start =
        chrono::DateTime::from_timestamp(observed_at.timestamp().div_euclid(60) * 60, 0)
            .unwrap()
            .naive_utc();
    let analytics = PostgresAnalyticsRepository::from_pool(pool.clone());
    let analytics_query = AnalyticsQuery {
        scope: AnalyticsScope {
            device_ids: vec![device_a.clone()],
            ..Default::default()
        },
        metric: AnalyticsMetric {
            selector: AnalyticsMetricSelector {
                blueprint_id: format!("{tenant_a}-blueprint"),
                stream_key: "position".into(),
                field_path: "/latitude".into(),
            },
            blueprint_key: "test-blueprint".into(),
            blueprint_name: "Test Blueprint".into(),
            label: "Latitude".into(),
            unit: None,
            value_type: "float64".into(),
            aggregates: vec!["average".into()],
            precision: None,
        },
        compatible_revision_ids: vec![revision_a.clone()],
        start: bucket_start,
        end: bucket_start + Duration::minutes(1),
        bucket_seconds: 60,
        max_devices: 10,
        max_rows: 10,
    };
    let analytics_data = analytics.query(&a, analytics_query.clone()).await.unwrap();
    assert_eq!(
        (
            analytics_data.selected_devices,
            analytics_data.compatible_devices
        ),
        (1, 1)
    );
    assert_eq!(analytics_data.buckets.len(), 1);
    assert_eq!(analytics_data.buckets[0].sample_count, 1);
    assert_eq!(analytics_data.buckets[0].average, 0.0);
    assert_eq!(analytics_data.buckets[0].bucket_start, bucket_start);
    let foreign_data = analytics.query(&b, analytics_query).await.unwrap();
    assert_eq!(
        (
            foreign_data.selected_devices,
            foreign_data.compatible_devices
        ),
        (0, 0)
    );
    assert!(foreign_data.buckets.is_empty());
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
    assert!(
        event_repository
            .latest_location(
                &a,
                &device_a,
                DeviceLocationQuery {
                    contract_id: format!("{device_a}-contract"),
                    stream_key: "position".into(),
                    latitude_path: "/latitude".into(),
                    longitude_path: "/longitude".into(),
                    since: observed_at,
                    now: observed_at + Duration::seconds(5),
                }
            )
            .await
            .unwrap()
            .is_none()
    );
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
                observed_at + Duration::seconds(5)
            )
            .await
            .unwrap()
            .is_empty()
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
            "device_metric_rollups_hourly",
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

#[tokio::test]
async fn postgres_rollups_order_late_events_and_ties() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let pool = Pool::builder()
        .max_size(3)
        .build(ConnectionManager::<PgConnection>::new(url))
        .unwrap();
    migrate_once(&pool);
    let suffix = Uuid::new_v4().simple().to_string();
    let tenant_id = format!("rollup-test-{suffix}");
    let revision_id = format!("revision-{suffix}");
    let device_id = format!("device-{suffix}");
    {
        let mut connection = pool.get().unwrap();
        seed_blueprint(&mut connection, &tenant_id, &revision_id);
    }
    let tenant = TenantId::new(tenant_id.clone()).unwrap();
    PostgresDeviceRepository::from_pool(pool.clone())
        .create(&tenant, new_device(&device_id, &revision_id), None)
        .await
        .unwrap();
    let events = PostgresEventRepository::from_pool(pool.clone());
    let hour = Utc::now().timestamp().div_euclid(3600) * 3600 - 3600;
    let timestamp = |offset| chrono::DateTime::from_timestamp(hour + offset, 0).unwrap();
    for (id, offset, value) in [
        ("a", 30, MetricValue::Float64(2.0)),
        ("late", 10, MetricValue::Int64(4)),
        ("z", 30, MetricValue::Float64(3.0)),
        ("text", 40, MetricValue::String("ok".into())),
    ] {
        let mut item = event(
            &tenant,
            &device_id,
            &format!("{device_id}-contract"),
            &format!("{id}-{suffix}"),
        );
        item.occurred_at = timestamp(offset);
        item.metrics = vec![DeviceMetricSample {
            stream_key: "readings".into(),
            field_path: "/value".into(),
            value,
        }];
        assert!(events.record(&tenant, item.clone()).await.unwrap().recorded);
        assert!(!events.record(&tenant, item).await.unwrap().recorded);
    }
    let rollup = diesel::sql_query(
        "SELECT sample_count, value_sum, value_min, value_max, latest_value, latest_at, latest_event_id
         FROM device_metric_rollups_hourly
         WHERE tenant_id = $1 AND device_id = $2 AND blueprint_revision_id = $3
           AND stream_key = 'readings' AND field_path = '/value'",
    )
    .bind::<Text, _>(&tenant_id)
    .bind::<Text, _>(&device_id)
    .bind::<Text, _>(&revision_id)
    .get_result::<Rollup>(&mut pool.get().unwrap())
    .unwrap();
    assert_eq!(rollup.sample_count, 3);
    assert_eq!(rollup.value_sum, 9.0);
    assert_eq!((rollup.value_min, rollup.value_max), (2.0, 4.0));
    assert_eq!(rollup.latest_value, 3.0);
    assert_eq!(rollup.latest_at, timestamp(30));
    assert_eq!(rollup.latest_event_id, format!("z-{suffix}"));
    let analytics = PostgresAnalyticsRepository::from_pool(pool.clone());
    let query = AnalyticsQuery {
        scope: AnalyticsScope {
            device_ids: vec![device_id.clone()],
            ..Default::default()
        },
        metric: AnalyticsMetric {
            selector: AnalyticsMetricSelector {
                blueprint_id: format!("{tenant_id}-blueprint"),
                stream_key: "readings".into(),
                field_path: "/value".into(),
            },
            blueprint_key: "test-blueprint".into(),
            blueprint_name: "Test Blueprint".into(),
            label: "Value".into(),
            unit: None,
            value_type: "float64".into(),
            aggregates: vec!["average".into()],
            precision: None,
        },
        compatible_revision_ids: vec![revision_id],
        start: timestamp(0).naive_utc(),
        end: timestamp(3_600).naive_utc(),
        bucket_seconds: 3_600,
        max_devices: 10,
        max_rows: 10,
    };
    let before = analytics.query(&tenant, query.clone()).await.unwrap();
    assert_eq!(
        before.source,
        extrittio_backend_core::analytics::AnalyticsDataSource::BlueprintMetricSamplesAndRollups
    );
    assert_eq!(before.buckets.len(), 1);
    assert_eq!(before.buckets[0].sample_count, 3);
    assert_eq!(before.buckets[0].average, 3.0);
    assert_eq!(before.buckets[0].latest, 3.0);
    diesel::sql_query("DELETE FROM device_events WHERE tenant_id = $1 AND device_id = $2")
        .bind::<Text, _>(&tenant_id)
        .bind::<Text, _>(&device_id)
        .execute(&mut pool.get().unwrap())
        .unwrap();
    let after = analytics.query(&tenant, query.clone()).await.unwrap();
    assert_eq!(after.buckets, before.buckets);
    {
        let mut connection = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO device_blueprint_revisions
             (id, tenant_id, blueprint_id, revision, document, document_hash, compatibility)
             VALUES ($1, $2, $3, 2, '{}', repeat('d', 64), '{}')",
        )
        .bind::<Text, _>(format!("new-revision-{suffix}"))
        .bind::<Text, _>(&tenant_id)
        .bind::<Text, _>(format!("{tenant_id}-blueprint"))
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO device_contracts
             (id, tenant_id, device_id, blueprint_revision_id, document, contract_hash)
             VALUES ($1, $2, $3, $4, '{}', repeat('e', 64))",
        )
        .bind::<Text, _>(format!("new-contract-{suffix}"))
        .bind::<Text, _>(&tenant_id)
        .bind::<Text, _>(&device_id)
        .bind::<Text, _>(format!("new-revision-{suffix}"))
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "UPDATE device_contract_assignments SET desired_contract_id = $1
             WHERE tenant_id = $2 AND device_id = $3",
        )
        .bind::<Text, _>(format!("new-contract-{suffix}"))
        .bind::<Text, _>(&tenant_id)
        .bind::<Text, _>(&device_id)
        .execute(&mut connection)
        .unwrap();
    }
    let historical = analytics.query(&tenant, query).await.unwrap();
    assert_eq!(historical.compatible_devices, 1);
    assert_eq!(historical.buckets, before.buckets);
}

#[tokio::test]
async fn postgres_location_batch_filters_missing_future_and_reassigned_devices() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let pool = Pool::builder()
        .max_size(3)
        .build(ConnectionManager::<PgConnection>::new(url))
        .unwrap();
    let suffix = Uuid::new_v4().simple().to_string();
    let tenant_id = format!("location-test-{suffix}");
    let revision_id = format!("revision-{suffix}");
    {
        let mut connection = pool.get().unwrap();
        migrate_once(&pool);
        seed_blueprint(&mut connection, &tenant_id, &revision_id);
    }
    let tenant = TenantId::new(tenant_id.clone()).unwrap();
    let devices = PostgresDeviceRepository::from_pool(pool.clone());
    let events = PostgresEventRepository::from_pool(pool.clone());
    let live = format!("live-{suffix}");
    let reassigned = format!("reassigned-{suffix}");
    let future = format!("future-{suffix}");
    let empty = format!("empty-{suffix}");
    for id in [&live, &reassigned, &future, &empty] {
        devices
            .create(&tenant, new_device(id, &revision_id), None)
            .await
            .unwrap();
    }
    for id in [&live, &reassigned] {
        events
            .record(
                &tenant,
                event(
                    &tenant,
                    id,
                    &format!("{id}-contract"),
                    &format!("event-{id}"),
                ),
            )
            .await
            .unwrap();
    }
    let mut future_event = event(
        &tenant,
        &future,
        &format!("{future}-contract"),
        &format!("event-{future}"),
    );
    future_event.occurred_at += Duration::minutes(1);
    events.record(&tenant, future_event).await.unwrap();
    let now = Utc::now() + Duration::seconds(1);
    let mut requested = vec![
        live.clone(),
        reassigned.clone(),
        future.clone(),
        empty.clone(),
    ];
    requested.extend((0..200).map(|index| format!("missing-{index}-{suffix}")));
    let found = events
        .latest_locations(&tenant, requested.clone(), now)
        .await
        .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|item| item.device_id.as_str())
            .collect::<Vec<_>>(),
        vec![live.as_str(), reassigned.as_str()]
    );
    assert!(
        events
            .latest_locations(&TenantId::new("default").unwrap(), requested.clone(), now)
            .await
            .unwrap()
            .is_empty()
    );
    {
        let mut connection = pool.get().unwrap();
        diesel::sql_query(
            "INSERT INTO device_contracts
             (id, tenant_id, device_id, blueprint_revision_id, document, contract_hash)
             VALUES ($1, $2, $3, $4, '{}', repeat('c', 64))",
        )
        .bind::<Text, _>(format!("replacement-{suffix}"))
        .bind::<Text, _>(&tenant_id)
        .bind::<Text, _>(&reassigned)
        .bind::<Text, _>(&revision_id)
        .execute(&mut connection)
        .unwrap();
        diesel::sql_query(
            "UPDATE device_contract_assignments SET desired_contract_id = $1
             WHERE tenant_id = $2 AND device_id = $3",
        )
        .bind::<Text, _>(format!("replacement-{suffix}"))
        .bind::<Text, _>(&tenant_id)
        .bind::<Text, _>(&reassigned)
        .execute(&mut connection)
        .unwrap();
    }
    let found = events
        .latest_locations(&tenant, requested, now)
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].device_id, live);
}
