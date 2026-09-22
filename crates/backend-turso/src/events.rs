use async_trait::async_trait;
use turso::params;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::events::DeviceEventRepository;
use extrittio_backend_core::events::{
    DeviceLocationQuery, DeviceLocationRecord, DeviceMetricQuery, DeviceMetricRecord, MetricValue,
    RecordDeviceEvent, RecordDeviceEventOutcome,
};

use crate::{TursoConnectionHandles, row};

#[cfg(test)]
mod rollup_tests {
    use super::*;
    use extrittio_backend_core::analytics::{
        AnalyticsMetric, AnalyticsMetricSelector, AnalyticsQuery, AnalyticsRepository,
        AnalyticsScope,
    };
    use extrittio_backend_core::events::DeviceMetricSample;
    use extrittio_backend_core::rule_engine::{cache::RuleCache, types::TelemetryData};
    use extrittio_backend_core::rule_snapshots::{
        DeviceRuleEvaluation, RuleEvaluationInput, RuleEvaluationSnapshot,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn numeric_rollups_handle_late_ties_duplicates_and_non_numeric_samples() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("rollups.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection
            .execute_batch(
                r#"
            INSERT INTO device_blueprints VALUES ('blueprint','default','sensor','Sensor',NULL,0,0);
            INSERT INTO device_blueprint_revisions VALUES ('revision','default','blueprint',1,'{}',
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','{}',0);
            INSERT INTO devices (id,tenant_id,name,status,firmware,created_at,updated_at)
                VALUES ('device','default','Device','online','1',0,0);
            INSERT INTO device_contracts VALUES ('contract','default','device','revision','{}',
                'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',0);
            INSERT INTO device_contract_assignments
                (tenant_id,device_id,desired_contract_id,created_at,updated_at)
                VALUES ('default','device','contract',0,0);
        "#,
            )
            .await
            .unwrap();
        let tenant = TenantId::new("default").unwrap();
        let repository = TursoEventRepository::from_handles(database.shared_handles());
        let make_event = |id: &str, at: i64, value: MetricValue| {
            let occurred_at = chrono::DateTime::from_timestamp_micros(at).unwrap();
            RecordDeviceEvent {
                event_id: id.into(),
                device_id: "device".into(),
                contract_id: "contract".into(),
                route_key: "sample".into(),
                occurred_at,
                received_at: occurred_at,
                payload: serde_json::json!({}),
                metrics: vec![DeviceMetricSample {
                    stream_key: "readings".into(),
                    field_path: "/value".into(),
                    value,
                }],
                rule_evaluation: DeviceRuleEvaluation {
                    snapshot: RuleEvaluationSnapshot::new(Arc::new(RuleCache::default())),
                    tenant: tenant.clone(),
                    device_id: "device".into(),
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
                    observed_at: occurred_at.naive_utc(),
                },
            }
        };
        let hour = 3_600_000_000_i64;
        let first = make_event("a", hour + 30_000_000, MetricValue::Float64(2.0));
        assert!(
            repository
                .record(&tenant, first.clone())
                .await
                .unwrap()
                .recorded
        );
        assert!(!repository.record(&tenant, first).await.unwrap().recorded);
        repository
            .record(
                &tenant,
                make_event("late", hour + 10_000_000, MetricValue::Int64(4)),
            )
            .await
            .unwrap();
        repository
            .record(
                &tenant,
                make_event("z", hour + 30_000_000, MetricValue::Float64(3.0)),
            )
            .await
            .unwrap();
        repository
            .record(
                &tenant,
                make_event("text", hour + 40_000_000, MetricValue::String("ok".into())),
            )
            .await
            .unwrap();
        repository
            .record(
                &tenant,
                make_event("partial", hour * 2 + 10_000_000, MetricValue::Int64(6)),
            )
            .await
            .unwrap();
        let mut rows = connection
            .query(
                "SELECT blueprint_revision_id, bucket_start, sample_count, value_sum,
                    value_min, value_max, latest_value, latest_at, latest_event_id
             FROM device_metric_rollups_hourly
             WHERE tenant_id = 'default' AND device_id = 'device'
               AND stream_key = 'readings' AND field_path = '/value'
               AND bucket_start = ?1",
                params![hour],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(row.get::<String>(0).unwrap(), "revision");
        assert_eq!(row.get::<i64>(1).unwrap(), hour);
        assert_eq!(row.get::<i64>(2).unwrap(), 3);
        assert_eq!(row.get::<f64>(3).unwrap(), 9.0);
        assert_eq!(row.get::<f64>(4).unwrap(), 2.0);
        assert_eq!(row.get::<f64>(5).unwrap(), 4.0);
        assert_eq!(row.get::<f64>(6).unwrap(), 3.0);
        assert_eq!(row.get::<i64>(7).unwrap(), hour + 30_000_000);
        assert_eq!(row.get::<String>(8).unwrap(), "z");
        assert!(rows.next().await.unwrap().is_none());
        drop(rows);
        let analytics = crate::TursoAnalyticsRepository::from_handles(database.shared_handles());
        let query = AnalyticsQuery {
            scope: AnalyticsScope {
                device_ids: vec!["device".into()],
                ..Default::default()
            },
            metric: AnalyticsMetric {
                selector: AnalyticsMetricSelector {
                    blueprint_id: "blueprint".into(),
                    stream_key: "readings".into(),
                    field_path: "/value".into(),
                },
                blueprint_key: "sensor".into(),
                blueprint_name: "Sensor".into(),
                label: "Value".into(),
                unit: None,
                value_type: "float64".into(),
                aggregates: vec!["average".into()],
                precision: None,
            },
            compatible_revision_ids: vec!["revision".into()],
            start: chrono::DateTime::from_timestamp_micros(hour)
                .unwrap()
                .naive_utc(),
            end: chrono::DateTime::from_timestamp_micros(hour * 2 + 20_000_000)
                .unwrap()
                .naive_utc(),
            bucket_seconds: 3_600,
            max_devices: 10,
            max_rows: 10,
        };
        let before = analytics.query(&tenant, query.clone()).await.unwrap();
        assert_eq!(
            before.source,
            extrittio_backend_core::analytics::AnalyticsDataSource::BlueprintMetricSamplesAndRollups
        );
        assert_eq!(before.buckets.len(), 2);
        assert_eq!(before.buckets[0].sample_count, 3);
        assert_eq!(before.buckets[0].average, 3.0);
        assert_eq!(before.buckets[0].latest, 3.0);
        assert_eq!(before.buckets[1].sample_count, 1);
        assert_eq!(before.buckets[1].average, 6.0);
        connection
            .execute(
                "DELETE FROM device_events WHERE tenant_id = 'default' AND occurred_at < ?1",
                params![hour * 2],
            )
            .await
            .unwrap();
        let after = analytics.query(&tenant, query.clone()).await.unwrap();
        assert_eq!(after.buckets, before.buckets);
        connection.execute_batch(r#"
            INSERT INTO device_blueprint_revisions VALUES ('revision-new','default','blueprint',2,'{}',
                'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc','{}',0);
            INSERT INTO device_contracts VALUES ('contract-new','default','device','revision-new','{}',
                'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',0);
            UPDATE device_contract_assignments SET desired_contract_id = 'contract-new'
                WHERE tenant_id = 'default' AND device_id = 'device';
        "#).await.unwrap();
        let historical = analytics.query(&tenant, query).await.unwrap();
        assert_eq!(historical.compatible_devices, 1);
        assert_eq!(historical.buckets, before.buckets);
    }
}

#[cfg(test)]
mod location_tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use std::time::Duration;

    fn time(micros: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_micros(micros).unwrap()
    }

    #[tokio::test]
    async fn location_uses_one_fresh_event_in_the_current_assignment() {
        // Query-level fixture: only contract-native tables, no legacy telemetry.
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("location.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection.execute_batch(
            "CREATE TABLE device_events (tenant_id TEXT, device_id TEXT, contract_id TEXT, id TEXT, occurred_at INTEGER);
             CREATE TABLE device_contract_assignments (tenant_id TEXT, device_id TEXT, desired_contract_id TEXT);
             CREATE TABLE device_metric_samples (tenant_id TEXT, device_id TEXT, event_id TEXT, occurred_at INTEGER, stream_key TEXT, field_path TEXT, value_type TEXT, value_double REAL, value_int INTEGER);
             INSERT INTO device_contract_assignments VALUES ('tenant', 'device', 'contract');"
        ).await.unwrap();

        for (id, tenant, device, contract, stream, at, lat, lon) in [
            (
                "origin", "tenant", "device", "contract", "position", 1000, 0.0, 0,
            ),
            (
                "future", "tenant", "device", "contract", "position", 3000, 1.0, 1,
            ),
            (
                "stale", "tenant", "device", "contract", "position", 100, 1.0, 1,
            ),
            (
                "tenant", "other", "device", "contract", "position", 2000, 1.0, 1,
            ),
            (
                "device", "tenant", "other", "contract", "position", 2000, 1.0, 1,
            ),
            (
                "contract", "tenant", "device", "old", "position", 2000, 1.0, 1,
            ),
            (
                "stream", "tenant", "device", "contract", "other", 2000, 1.0, 1,
            ),
            (
                "latitude", "tenant", "device", "contract", "position", 2000, 91.0, 1,
            ),
            (
                "longitude",
                "tenant",
                "device",
                "contract",
                "position",
                2000,
                1.0,
                181,
            ),
            (
                "partial", "tenant", "device", "contract", "position", 2000, 1.0, 1,
            ),
            (
                "timestamp",
                "tenant",
                "device",
                "contract",
                "position",
                2000,
                1.0,
                1,
            ),
            (
                "split-stream",
                "tenant",
                "device",
                "contract",
                "position",
                2000,
                1.0,
                1,
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO device_events VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![tenant, device, contract, id, at],
                )
                .await
                .unwrap();
            connection.execute("INSERT INTO device_metric_samples VALUES (?1, ?2, ?3, ?4, ?5, '/y', 'float64', ?6, NULL)", params![tenant, device, id, at, stream, lat]).await.unwrap();
            if id != "partial" {
                let lon_at = if id == "timestamp" { at - 1 } else { at };
                let lon_stream = if id == "split-stream" {
                    "other"
                } else {
                    stream
                };
                connection.execute("INSERT INTO device_metric_samples VALUES (?1, ?2, ?3, ?4, ?5, '/x', 'int64', NULL, ?6)", params![tenant, device, id, lon_at, lon_stream, lon]).await.unwrap();
            }
        }
        let repository = TursoEventRepository::from_handles(database.shared_handles());
        connection.execute_batch("CREATE TABLE device_contracts (tenant_id TEXT, device_id TEXT, id TEXT, document TEXT)").await.unwrap();
        let document = serde_json::json!({
            "deviceId":"device", "location": {
                "stream":"position", "latitudePath":"/y", "longitudePath":"/x",
                "coordinateSystem":"wgs84", "unit":"degrees", "maxAgeMs":1
            }
        });
        connection
            .execute(
                "INSERT INTO device_contracts VALUES ('tenant','device','contract',?1)",
                params![document.to_string()],
            )
            .await
            .unwrap();
        let tenant = TenantId::new("tenant").unwrap();
        let query = DeviceLocationQuery {
            contract_id: "contract".into(),
            stream_key: "position".into(),
            latitude_path: "/y".into(),
            longitude_path: "/x".into(),
            since: time(999),
            now: time(1999),
        };
        let location = repository
            .latest_location(&tenant, "device", query.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(location.event_id, "origin");
        assert_eq!(location.contract_id, "contract");
        assert_eq!((location.latitude, location.longitude), (0.0, 0.0));
        assert_eq!(location.occurred_at, time(1000));
        assert_eq!(location.expires_at, time(2000));
        let batch = repository
            .latest_locations(&tenant, vec!["device".into(), "missing".into()], time(1999))
            .await
            .unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].device_id, "device");
        assert_eq!(batch[0].location, location);
        assert!(
            repository
                .latest_locations(&tenant, vec!["device".into()], time(2000))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            repository
                .latest_locations(
                    &TenantId::new("other").unwrap(),
                    vec!["device".into()],
                    time(2000)
                )
                .await
                .unwrap()
                .is_empty()
        );

        let mut boundary = query.clone();
        boundary.since = time(999);
        boundary.now = time(1000);
        assert!(
            repository
                .latest_location(&tenant, "device", boundary.clone())
                .await
                .unwrap()
                .is_some()
        );
        boundary.since = time(1000);
        boundary.now = time(2000);
        assert!(
            repository
                .latest_location(&tenant, "device", boundary)
                .await
                .unwrap()
                .is_none()
        );

        connection.execute("INSERT INTO device_events SELECT tenant_id, device_id, contract_id, 'origin-b', occurred_at FROM device_events WHERE id = 'origin'", ()).await.unwrap();
        connection.execute("INSERT INTO device_metric_samples SELECT tenant_id, device_id, 'origin-b', occurred_at, stream_key, field_path, value_type, value_double, value_int FROM device_metric_samples WHERE event_id = 'origin'", ()).await.unwrap();
        assert_eq!(
            repository
                .latest_location(&tenant, "device", query.clone())
                .await
                .unwrap()
                .unwrap()
                .event_id,
            "origin-b"
        );

        // Reassignment invalidates a previously loaded contract query.
        connection
            .execute(
                "UPDATE device_contract_assignments SET desired_contract_id = 'replacement'",
                (),
            )
            .await
            .unwrap();
        assert!(
            repository
                .latest_locations(&tenant, vec!["device".into()], time(2000))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            repository
                .latest_location(&tenant, "device", query)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn stale_contract_event_is_rejected_before_persistence() {
        use crate::events::TursoEventRepository;
        use extrittio_backend_core::rule_engine::{cache::RuleCache, types::TelemetryData};
        use extrittio_backend_core::rule_snapshots::{
            DeviceRuleEvaluation, RuleEvaluationInput, RuleEvaluationSnapshot,
        };
        use std::sync::Arc;

        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("stale-contract.db"),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        let handles = database.shared_handles();
        handles
            .connect()
            .unwrap()
            .execute_batch(
                "CREATE TABLE device_contract_assignments (
                    tenant_id TEXT, device_id TEXT, desired_contract_id TEXT);
                 CREATE TABLE device_events (id TEXT);
                 INSERT INTO device_contract_assignments
                 VALUES ('tenant', 'device', 'replacement');",
            )
            .await
            .unwrap();
        let tenant = TenantId::new("tenant").unwrap();
        let now = Utc::now();
        let event = RecordDeviceEvent {
            event_id: "stale-event".into(),
            device_id: "device".into(),
            contract_id: "previous".into(),
            route_key: "sample".into(),
            occurred_at: now,
            received_at: now,
            payload: serde_json::json!({}),
            metrics: Vec::new(),
            rule_evaluation: DeviceRuleEvaluation {
                snapshot: RuleEvaluationSnapshot::new(Arc::new(RuleCache::default())),
                tenant: tenant.clone(),
                device_id: "device".into(),
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
        };
        let repository = TursoEventRepository::from_handles(handles.clone());
        assert!(matches!(
            repository.record(&tenant, event).await,
            Err(PersistenceError::NotFound)
        ));
        let mut rows = handles
            .connect()
            .unwrap()
            .query("SELECT id FROM device_events", ())
            .await
            .unwrap();
        assert!(rows.next().await.unwrap().is_none());
    }
}

#[derive(Clone)]
pub struct TursoEventRepository {
    handles: TursoConnectionHandles,
}
impl TursoEventRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
    fn connect(&self) -> Result<turso::Connection, PersistenceError> {
        self.handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))
    }
}

const LOCATION_SQL: &str = r#"SELECT * FROM (SELECT e.contract_id, e.id AS event_id, e.occurred_at,
    e.occurred_at + (?8 - ?7) AS expires_at,
    CASE lat.value_type WHEN 'float64' THEN lat.value_double WHEN 'int64' THEN CAST(lat.value_int AS REAL) END AS latitude,
    CASE lon.value_type WHEN 'float64' THEN lon.value_double WHEN 'int64' THEN CAST(lon.value_int AS REAL) END AS longitude
FROM device_events e
JOIN device_contract_assignments a
  ON a.tenant_id = e.tenant_id AND a.device_id = e.device_id AND a.desired_contract_id = e.contract_id
JOIN device_metric_samples lat
  ON lat.tenant_id = e.tenant_id AND lat.device_id = e.device_id AND lat.event_id = e.id AND lat.occurred_at = e.occurred_at
JOIN device_metric_samples lon
  ON lon.tenant_id = e.tenant_id AND lon.device_id = e.device_id AND lon.event_id = e.id AND lon.occurred_at = e.occurred_at
WHERE e.tenant_id = ?1 AND e.device_id = ?2 AND e.contract_id = ?3
  AND lat.stream_key = ?4 AND lon.stream_key = ?4
  AND lat.field_path = ?5 AND lon.field_path = ?6
  AND e.occurred_at > ?7 AND e.occurred_at <= ?8) AS positions
WHERE latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180
ORDER BY occurred_at DESC, event_id COLLATE BINARY DESC LIMIT 1"#;

#[async_trait]
impl DeviceEventRepository for TursoEventRepository {
    async fn latest_locations(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<extrittio_backend_core::events::LocatedDeviceRecord>, PersistenceError> {
        let connection = self.connect()?;
        if device_ids.is_empty() {
            return Ok(Vec::new());
        }
        // Bound placeholders avoid Turso's incorrect planning of JSON-array
        // membership inside this self-joined/windowed location query.
        let placeholders = (0..device_ids.len())
            .map(|index| format!("?{}", index + 3))
            .collect::<Vec<_>>()
            .join(",");
        let mut bindings = vec![
            turso::Value::Text(tenant.as_str().to_owned()),
            turso::Value::Integer(now.timestamp_micros()),
        ];
        bindings.extend(device_ids.into_iter().map(turso::Value::Text));
        let sql = format!(
            r#"WITH positions AS (
SELECT e.device_id,e.contract_id,e.id AS event_id,e.occurred_at,
e.occurred_at + (json_extract(c.document,'$.location.maxAgeMs') * 1000) AS expires_at,
CASE lat.value_type WHEN 'float64' THEN lat.value_double WHEN 'int64' THEN CAST(lat.value_int AS REAL) END AS latitude,
CASE lon.value_type WHEN 'float64' THEN lon.value_double WHEN 'int64' THEN CAST(lon.value_int AS REAL) END AS longitude
FROM device_events e
JOIN device_contract_assignments a ON a.tenant_id=e.tenant_id AND a.device_id=e.device_id AND a.desired_contract_id=e.contract_id
JOIN device_contracts c ON c.tenant_id=e.tenant_id AND c.device_id=e.device_id AND c.id=e.contract_id
JOIN device_metric_samples lat ON lat.tenant_id=e.tenant_id AND lat.device_id=e.device_id AND lat.event_id=e.id AND lat.occurred_at=e.occurred_at
JOIN device_metric_samples lon ON lon.tenant_id=e.tenant_id AND lon.device_id=e.device_id AND lon.event_id=e.id AND lon.occurred_at=e.occurred_at
WHERE e.tenant_id=?1 AND e.device_id IN ({placeholders}) AND e.occurred_at <= ?2
AND e.occurred_at > ?2 - (json_extract(c.document,'$.location.maxAgeMs') * 1000)
AND json_extract(c.document,'$.deviceId')=e.device_id
AND json_extract(c.document,'$.location.coordinateSystem')='wgs84' AND json_extract(c.document,'$.location.unit')='degrees'
AND lat.stream_key=json_extract(c.document,'$.location.stream') AND lon.stream_key=lat.stream_key
AND lat.field_path=json_extract(c.document,'$.location.latitudePath') AND lon.field_path=json_extract(c.document,'$.location.longitudePath')
), ranked AS (
SELECT *,row_number() OVER (PARTITION BY device_id ORDER BY occurred_at DESC,event_id COLLATE BINARY DESC) AS position_rank
FROM positions WHERE latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180
)
SELECT device_id,contract_id,event_id,occurred_at,expires_at,latitude,longitude FROM ranked WHERE position_rank=1 ORDER BY device_id"#
        );
        let mut rows = connection
            .query(&sql, bindings)
            .await
            .map_err(row::legacy_error)?;
        let mut locations = Vec::new();
        while let Some(r) = rows.next().await.map_err(row::legacy_error)? {
            let device_id: String = r.get(0).map_err(row::legacy_error)?;
            locations.push(extrittio_backend_core::events::LocatedDeviceRecord {
                device_id,
                location: DeviceLocationRecord {
                    contract_id: r.get(1).map_err(row::legacy_error)?,
                    event_id: r.get(2).map_err(row::legacy_error)?,
                    occurred_at: row::datetime(r.get(3).map_err(row::legacy_error)?)?,
                    expires_at: row::datetime(r.get(4).map_err(row::legacy_error)?)?,
                    latitude: r.get(5).map_err(row::legacy_error)?,
                    longitude: r.get(6).map_err(row::legacy_error)?,
                },
            });
        }
        Ok(locations)
    }

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: DeviceLocationQuery,
    ) -> Result<Option<DeviceLocationRecord>, PersistenceError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                LOCATION_SQL,
                params![
                    tenant.as_str(),
                    device_id,
                    query.contract_id,
                    query.stream_key,
                    query.latitude_path,
                    query.longitude_path,
                    query.since.timestamp_micros(),
                    query.now.timestamp_micros(),
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let Some(r) = rows.next().await.map_err(row::legacy_error)? else {
            return Ok(None);
        };
        Ok(Some(DeviceLocationRecord {
            contract_id: r.get(0).map_err(row::legacy_error)?,
            event_id: r.get(1).map_err(row::legacy_error)?,
            occurred_at: row::datetime(r.get(2).map_err(row::legacy_error)?)?,
            expires_at: row::datetime(r.get(3).map_err(row::legacy_error)?)?,
            latitude: r.get(4).map_err(row::legacy_error)?,
            longitude: r.get(5).map_err(row::legacy_error)?,
        }))
    }

    async fn record(
        &self,
        tenant: &TenantId,
        event: RecordDeviceEvent,
    ) -> Result<RecordDeviceEventOutcome, PersistenceError> {
        let mut writer = self.handles.lock_writer().await;
        let transaction = writer.transaction().await.map_err(row::legacy_error)?;
        let mut assignments = transaction
            .query(
                "SELECT desired_contract_id FROM device_contract_assignments
                 WHERE tenant_id = ?1 AND device_id = ?2",
                params![tenant.as_str(), event.device_id.clone()],
            )
            .await
            .map_err(row::legacy_error)?;
        let desired_contract = assignments
            .next()
            .await
            .map_err(row::legacy_error)?
            .map(|record| record.get::<String>(0).map_err(row::legacy_error))
            .transpose()?;
        drop(assignments);
        if desired_contract.as_deref() != Some(event.contract_id.as_str()) {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Err(PersistenceError::NotFound);
        }
        let payload = serde_json::to_string(&event.payload)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO device_events
                    (id, tenant_id, device_id, contract_id, route_key,
                     occurred_at, received_at, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    event.event_id.clone(),
                    tenant.as_str(),
                    event.device_id.clone(),
                    event.contract_id.clone(),
                    event.route_key,
                    event.occurred_at.timestamp_micros(),
                    event.received_at.timestamp_micros(),
                    payload
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        if inserted == 0 {
            transaction.rollback().await.map_err(row::legacy_error)?;
            return Ok(RecordDeviceEventOutcome {
                recorded: false,
                metrics_recorded: 0,
                actions_enqueued: 0,
            });
        }

        transaction
            .execute(
                "UPDATE device_contract_assignments
                 SET active_contract_id = desired_contract_id, status = 'converged',
                     acknowledged_at = ?4, error = NULL, updated_at = ?4
                 WHERE tenant_id = ?1 AND device_id = ?2 AND desired_contract_id = ?3",
                params![
                    tenant.as_str(),
                    event.device_id.clone(),
                    event.contract_id.clone(),
                    event.received_at.timestamp_micros()
                ],
            )
            .await
            .map_err(row::legacy_error)?;

        let metrics_recorded = event.metrics.len();
        for metric in event.metrics {
            let value_type = metric.value.value_type();
            let (value_double, value_int, value_text, value_bool, value_json) = match metric.value {
                MetricValue::Float64(value) => (Some(value), None, None, None, None),
                MetricValue::Int64(value) => (None, Some(value), None, None, None),
                MetricValue::String(value) => (None, None, Some(value), None, None),
                MetricValue::Boolean(value) => (None, None, None, Some(i64::from(value)), None),
                MetricValue::Json(value) => (
                    None,
                    None,
                    None,
                    None,
                    Some(
                        serde_json::to_string(&value)
                            .map_err(|error| PersistenceError::Internal(error.to_string()))?,
                    ),
                ),
            };
            transaction
                .execute(
                    "INSERT INTO device_metric_samples
                        (event_id, tenant_id, device_id, stream_key, field_path, value_type,
                         value_double, value_int, value_text, value_bool, value_json, occurred_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        event.event_id.clone(),
                        tenant.as_str(),
                        event.device_id.clone(),
                        metric.stream_key.clone(),
                        metric.field_path.clone(),
                        value_type,
                        value_double,
                        value_int,
                        value_text,
                        value_bool,
                        value_json,
                        event.occurred_at.timestamp_micros()
                    ],
                )
                .await
                .map_err(row::legacy_error)?;
            if let Some(value) = value_double.or_else(|| value_int.map(|v| v as f64)) {
                let occurred_at = event.occurred_at.timestamp_micros();
                let bucket_start = occurred_at.div_euclid(3_600_000_000) * 3_600_000_000;
                transaction
                    .execute(
                        "INSERT INTO device_metric_rollups_hourly
                            (tenant_id, device_id, blueprint_revision_id, stream_key,
                             field_path, bucket_start, sample_count, value_sum,
                             value_min, value_max, latest_value, latest_at, latest_event_id)
                         SELECT ?1, ?2, c.blueprint_revision_id, ?4, ?5, ?6,
                                1, ?7, ?7, ?7, ?7, ?8, ?9
                         FROM device_contracts c
                         WHERE c.tenant_id = ?1 AND c.device_id = ?2 AND c.id = ?3
                         ON CONFLICT (tenant_id, device_id, blueprint_revision_id,
                                      stream_key, field_path, bucket_start)
                         DO UPDATE SET
                             sample_count = device_metric_rollups_hourly.sample_count + 1,
                             value_sum = device_metric_rollups_hourly.value_sum + excluded.value_sum,
                             value_min = min(device_metric_rollups_hourly.value_min, excluded.value_min),
                             value_max = max(device_metric_rollups_hourly.value_max, excluded.value_max),
                             latest_value = CASE WHEN excluded.latest_at > device_metric_rollups_hourly.latest_at
                                 OR (excluded.latest_at = device_metric_rollups_hourly.latest_at
                                     AND excluded.latest_event_id COLLATE BINARY > device_metric_rollups_hourly.latest_event_id COLLATE BINARY)
                                 THEN excluded.latest_value ELSE device_metric_rollups_hourly.latest_value END,
                             latest_at = max(device_metric_rollups_hourly.latest_at, excluded.latest_at),
                             latest_event_id = CASE WHEN excluded.latest_at > device_metric_rollups_hourly.latest_at
                                 OR (excluded.latest_at = device_metric_rollups_hourly.latest_at
                                     AND excluded.latest_event_id COLLATE BINARY > device_metric_rollups_hourly.latest_event_id COLLATE BINARY)
                                 THEN excluded.latest_event_id ELSE device_metric_rollups_hourly.latest_event_id END",
                        params![
                            tenant.as_str(),
                            event.device_id.clone(),
                            event.contract_id.clone(),
                            metric.stream_key,
                            metric.field_path,
                            bucket_start,
                            value,
                            occurred_at,
                            event.event_id.clone()
                        ],
                    )
                    .await
                    .map_err(row::legacy_error)?;
            }
        }
        let actions = crate::rule_runtime::evaluate_rules_in_transaction(
            &transaction,
            tenant.as_str(),
            &event.device_id,
            Some(&event.rule_evaluation),
        )
        .await?;
        let actions_enqueued =
            crate::outbox::enqueue_actions_in_transaction(&transaction, &actions).await?;
        transaction.commit().await.map_err(row::legacy_error)?;
        Ok(RecordDeviceEventOutcome {
            recorded: true,
            metrics_recorded,
            actions_enqueued,
        })
    }

    async fn list_metrics(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: DeviceMetricQuery,
    ) -> Result<Option<Vec<DeviceMetricRecord>>, PersistenceError> {
        let connection = self.connect()?;
        let mut existence = connection
            .query(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::legacy_error)?;
        let exists = existence
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::legacy_error)?
            != 0;
        drop(existence);
        if !exists {
            return Ok(None);
        }

        let mut rows = connection
            .query(
                "SELECT event_id, device_id, stream_key, field_path, value_type,
                        value_double, value_int, value_text, value_bool, value_json,
                        occurred_at,
                        (SELECT contract_id FROM device_events e WHERE e.tenant_id = device_metric_samples.tenant_id AND e.id = device_metric_samples.event_id) AS contract_id
                 FROM device_metric_samples
                 WHERE tenant_id = ?1 AND device_id = ?2
                   AND (?3 IS NULL OR stream_key = ?3)
                   AND (?4 IS NULL OR field_path = ?4)
                   AND (?5 IS NULL OR occurred_at >= ?5)
                   AND (?6 IS NULL OR occurred_at < ?6)
                 ORDER BY occurred_at DESC, event_id COLLATE BINARY DESC, stream_key COLLATE BINARY, field_path COLLATE BINARY
                 LIMIT ?7",
                params![
                    tenant.as_str(),
                    device_id,
                    query.stream_key,
                    query.field_path,
                    query.since.map(|value| value.and_utc().timestamp_micros()),
                    query.before.map(|value| value.and_utc().timestamp_micros()),
                    query.limit
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut metrics = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            let event_id = record.get::<String>(0).map_err(row::legacy_error)?;
            let value_type = record.get::<String>(4).map_err(row::legacy_error)?;
            let value = match value_type.as_str() {
                "float64" => record
                    .get::<Option<f64>>(5)
                    .map_err(row::legacy_error)?
                    .map(MetricValue::Float64),
                "int64" => record
                    .get::<Option<i64>>(6)
                    .map_err(row::legacy_error)?
                    .map(MetricValue::Int64),
                "string" => record
                    .get::<Option<String>>(7)
                    .map_err(row::legacy_error)?
                    .map(MetricValue::String),
                "boolean" => record
                    .get::<Option<i64>>(8)
                    .map_err(row::legacy_error)?
                    .map(|value| MetricValue::Boolean(value != 0)),
                "json" => record
                    .get::<Option<String>>(9)
                    .map_err(row::legacy_error)?
                    .map(|value| {
                        serde_json::from_str(&value)
                            .map(MetricValue::Json)
                            .map_err(|error| PersistenceError::CorruptData(error.to_string()))
                    })
                    .transpose()?,
                _ => None,
            }
            .ok_or_else(|| {
                PersistenceError::CorruptData(format!(
                    "metric '{event_id}' has invalid value_type '{value_type}' or missing value"
                ))
            })?;
            metrics.push(DeviceMetricRecord {
                contract_id: record.get(11).map_err(row::legacy_error)?,
                event_id,
                device_id: record.get(1).map_err(row::legacy_error)?,
                stream_key: record.get(2).map_err(row::legacy_error)?,
                field_path: record.get(3).map_err(row::legacy_error)?,
                value,
                occurred_at: row::datetime(record.get(10).map_err(row::legacy_error)?)?,
            });
        }
        Ok(Some(metrics))
    }
}
