use async_trait::async_trait;
use turso::params;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::analytics::AnalyticsRepository;
use extrittio_backend_core::analytics::{
    AnalyticsBlueprintRevision, AnalyticsBucket, AnalyticsDevice, AnalyticsQuery,
    AnalyticsQueryData,
};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoAnalyticsRepository {
    handles: TursoConnectionHandles,
}
impl TursoAnalyticsRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

const SCOPE_FILTER: &str = r#"
    d.tenant_id = ?1
    AND (?2 = '[]' OR d.fleet_id IN (SELECT value FROM json_each(?2)))
    AND (?3 = '[]' OR d.id IN (SELECT value FROM json_each(?3)))
"#;

// Keep the adapter-level contract test beside the scope query it exercises.
#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod blueprint_scope_tests {
    use super::*;
    use extrittio_backend_core::analytics::{
        AnalyticsMetric, AnalyticsMetricSelector, AnalyticsScope,
    };

    #[tokio::test]
    async fn analytics_queries_blueprint_baseline_without_device_types() {
        let directory = tempfile::tempdir().unwrap();
        let database = crate::TursoDatabase::open(
            directory.path(),
            &directory.path().join("analytics.db"),
            std::time::Duration::from_secs(1),
        )
        .await
        .unwrap();
        database.migrate().await.unwrap();
        let connection = database.shared_handles().connect().unwrap();
        connection.execute_batch(r#"
            PRAGMA foreign_keys = ON;
            INSERT INTO device_blueprints VALUES ('blueprint','default','sensor','Sensor',NULL,0,0);
            INSERT INTO device_blueprint_revisions VALUES ('revision','default','blueprint',1,'{}',
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','{}',0);
            INSERT INTO devices (id,tenant_id,name,status,firmware,created_at,updated_at)
                VALUES ('device','default','Device','online','1',0,0),
                       ('unassigned','default','Unassigned','online','1',0,0);
            INSERT INTO device_contracts VALUES ('contract','default','device','revision','{}',
                'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',0);
            INSERT INTO device_contract_assignments
                (tenant_id,device_id,desired_contract_id,created_at,updated_at)
                VALUES ('default','device','contract',0,0);
            INSERT INTO device_events VALUES ('event','default','device','contract','readings',1000000,1000000,'{}');
            INSERT INTO device_metric_samples
                (event_id,tenant_id,device_id,stream_key,field_path,value_type,value_int,occurred_at)
                VALUES ('event','default','device','readings','/arbitrary','int64',42,1000000);
        "#).await.unwrap();
        let repository = TursoAnalyticsRepository::from_handles(database.shared_handles());
        let tenant = TenantId::new("default").unwrap();
        let mut query = AnalyticsQuery {
            scope: AnalyticsScope::default(),
            metric: AnalyticsMetric {
                selector: AnalyticsMetricSelector {
                    blueprint_id: "blueprint".into(),
                    stream_key: "readings".into(),
                    field_path: "/arbitrary".into(),
                },
                blueprint_key: "sensor".into(),
                blueprint_name: "Sensor".into(),
                label: "Arbitrary".into(),
                unit: None,
                value_type: "int64".into(),
                aggregates: vec!["average".into()],
                precision: None,
            },
            start: chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc(),
            end: chrono::DateTime::from_timestamp(60, 0).unwrap().naive_utc(),
            bucket_seconds: 60,
            max_devices: 10,
            max_rows: 10,
        };
        let result = repository.query(&tenant, query.clone()).await.unwrap();
        assert_eq!((result.selected_devices, result.compatible_devices), (2, 1));
        assert_eq!(result.devices[0].id, "device");
        assert_eq!(result.buckets.len(), 1);
        assert_eq!(result.buckets[0].average, 42.0);
        query.scope.device_ids = vec!["unassigned".into()];
        let result = repository.query(&tenant, query.clone()).await.unwrap();
        assert_eq!((result.selected_devices, result.compatible_devices), (1, 0));
        query.scope.device_ids.clear();
        query.scope.fleet_ids = vec![999];
        assert_eq!(
            repository
                .query(&tenant, query.clone())
                .await
                .unwrap()
                .selected_devices,
            0
        );
        query.scope.fleet_ids.clear();
        query.metric.selector.blueprint_id = "another-blueprint".into();
        assert_eq!(
            repository
                .query(&tenant, query.clone())
                .await
                .unwrap()
                .compatible_devices,
            0
        );
        assert_eq!(
            repository
                .query(&TenantId::new("another-tenant").unwrap(), query)
                .await
                .unwrap()
                .selected_devices,
            0
        );
    }
}

#[async_trait]
impl AnalyticsRepository for TursoAnalyticsRepository {
    async fn blueprint_catalog(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<AnalyticsBlueprintRevision>, PersistenceError> {
        let connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let mut rows = connection
            .query(
                r#"
                SELECT b.id, b.blueprint_key, b.name, r.id, r.revision, r.document
                FROM device_blueprints b
                JOIN device_blueprint_revisions r
                  ON r.tenant_id = b.tenant_id
                 AND r.blueprint_id = b.id
                WHERE b.tenant_id = ?1
                  AND r.revision = (
                      SELECT max(latest.revision)
                      FROM device_blueprint_revisions latest
                      WHERE latest.tenant_id = b.tenant_id
                        AND latest.blueprint_id = b.id
                  )
                ORDER BY b.name COLLATE BINARY, b.id COLLATE BINARY
                "#,
                params![tenant.as_str()],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut revisions = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            let document: String = record.get(5).map_err(row::legacy_error)?;
            revisions.push(AnalyticsBlueprintRevision {
                blueprint_id: record.get(0).map_err(row::legacy_error)?,
                blueprint_key: record.get(1).map_err(row::legacy_error)?,
                blueprint_name: record.get(2).map_err(row::legacy_error)?,
                revision_id: record.get(3).map_err(row::legacy_error)?,
                revision: record.get(4).map_err(row::legacy_error)?,
                document: serde_json::from_str(&document).map_err(|error| {
                    PersistenceError::CorruptData(format!(
                        "stored blueprint revision document is invalid: {error}"
                    ))
                })?,
            });
        }
        Ok(revisions)
    }

    async fn query(
        &self,
        tenant: &TenantId,
        query: AnalyticsQuery,
    ) -> Result<AnalyticsQueryData, PersistenceError> {
        let mut raw_connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let connection = raw_connection
            .transaction()
            .await
            .map_err(row::legacy_error)?;
        let fleet_ids = serde_json::to_string(&query.scope.fleet_ids)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        let requested_device_ids = serde_json::to_string(&query.scope.device_ids)
            .map_err(|error| PersistenceError::Internal(error.to_string()))?;

        let count_sql = format!("SELECT count(*) FROM devices d WHERE {SCOPE_FILTER}");
        let mut count_rows = connection
            .query(
                &count_sql,
                params![
                    tenant.as_str(),
                    fleet_ids.clone(),
                    requested_device_ids.clone()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let selected_devices = count_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::legacy_error)
            .and_then(|value| {
                usize::try_from(value).map_err(|_| {
                    PersistenceError::CorruptData("analytics device count is invalid".to_string())
                })
            })?;

        drop(count_rows);
        let compatible_sql = format!(
            r#"
            SELECT count(*)
            FROM devices d
            JOIN device_contract_assignments a
              ON a.tenant_id = d.tenant_id
             AND a.device_id = d.id
            JOIN device_contracts c
              ON c.tenant_id = a.tenant_id
             AND c.device_id = a.device_id
             AND c.id = a.desired_contract_id
            JOIN device_blueprint_revisions r
              ON r.tenant_id = c.tenant_id
             AND r.id = c.blueprint_revision_id
            WHERE {SCOPE_FILTER}
              AND r.blueprint_id = ?4
            "#
        );
        let mut compatible_rows = connection
            .query(
                &compatible_sql,
                params![
                    tenant.as_str(),
                    fleet_ids.clone(),
                    requested_device_ids.clone(),
                    query.metric.selector.blueprint_id.clone()
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let compatible_devices = compatible_rows
            .next()
            .await
            .map_err(row::legacy_error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::legacy_error)
            .and_then(|value| {
                usize::try_from(value).map_err(|_| {
                    PersistenceError::CorruptData(
                        "analytics compatible device count is invalid".to_string(),
                    )
                })
            })?;
        drop(compatible_rows);

        let device_sql = format!(
            r#"
            SELECT d.id, d.name
            FROM devices d
            JOIN device_contract_assignments a
              ON a.tenant_id = d.tenant_id
             AND a.device_id = d.id
            JOIN device_contracts c
              ON c.tenant_id = a.tenant_id
             AND c.device_id = a.device_id
             AND c.id = a.desired_contract_id
            JOIN device_blueprint_revisions r
              ON r.tenant_id = c.tenant_id
             AND r.id = c.blueprint_revision_id
            WHERE {SCOPE_FILTER}
              AND r.blueprint_id = ?4
            ORDER BY d.name COLLATE BINARY, d.id COLLATE BINARY
            LIMIT ?5
            "#
        );
        let device_limit = i64::try_from(query.max_devices.saturating_add(1)).unwrap_or(i64::MAX);
        let mut device_rows = connection
            .query(
                &device_sql,
                params![
                    tenant.as_str(),
                    fleet_ids,
                    requested_device_ids,
                    query.metric.selector.blueprint_id.clone(),
                    device_limit
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut devices = Vec::new();
        while let Some(record) = device_rows.next().await.map_err(row::legacy_error)? {
            devices.push(AnalyticsDevice {
                id: record.get(0).map_err(row::legacy_error)?,
                name: record.get(1).map_err(row::legacy_error)?,
            });
        }

        drop(device_rows);
        if compatible_devices > query.max_devices || devices.is_empty() {
            connection.commit().await.map_err(row::legacy_error)?;
            return Ok(AnalyticsQueryData {
                selected_devices,
                compatible_devices,
                devices,
                buckets: Vec::new(),
            });
        }

        let device_ids = serde_json::to_string(
            &devices
                .iter()
                .map(|device| device.id.as_str())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| PersistenceError::Internal(error.to_string()))?;
        let bucket_micros = query.bucket_seconds.saturating_mul(1_000_000);
        let row_limit = i64::try_from(query.max_rows.saturating_add(1)).unwrap_or(i64::MAX);
        let mut rows = connection
            .query(
                metric_samples_query(),
                params![
                    tenant.as_str(),
                    device_ids,
                    query.start.and_utc().timestamp_micros(),
                    query.end.and_utc().timestamp_micros(),
                    bucket_micros,
                    query.metric.selector.stream_key,
                    query.metric.selector.field_path,
                    query.metric.selector.blueprint_id,
                    row_limit
                ],
            )
            .await
            .map_err(row::legacy_error)?;
        let mut buckets = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            let bucket_micros: i64 = record.get(2).map_err(row::legacy_error)?;
            buckets.push(AnalyticsBucket {
                device_id: record.get(0).map_err(row::legacy_error)?,
                device_name: record.get(1).map_err(row::legacy_error)?,
                bucket_start: row::datetime(bucket_micros)?.naive_utc(),
                sample_count: record.get(3).map_err(row::legacy_error)?,
                average: record.get(4).map_err(row::legacy_error)?,
                minimum: record.get(5).map_err(row::legacy_error)?,
                maximum: record.get(6).map_err(row::legacy_error)?,
                latest: record.get(7).map_err(row::legacy_error)?,
            });
        }

        drop(rows);
        connection.commit().await.map_err(row::legacy_error)?;
        Ok(AnalyticsQueryData {
            selected_devices,
            compatible_devices,
            devices,
            buckets,
        })
    }
}

fn metric_samples_query() -> &'static str {
    r#"
        WITH bucketed AS (
            SELECT
                d.id AS device_id,
                d.name AS device_name,
                s.occurred_at - CASE WHEN s.occurred_at % ?5 < 0 THEN s.occurred_at % ?5 + ?5 ELSE s.occurred_at % ?5 END AS bucket_start,
                coalesce(s.value_double, CAST(s.value_int AS REAL)) AS value,
                s.occurred_at,
                s.event_id
            FROM device_metric_samples s
            JOIN devices d
              ON d.tenant_id = s.tenant_id
             AND d.id = s.device_id
            JOIN device_events e
              ON e.tenant_id = s.tenant_id
             AND e.device_id = s.device_id
             AND e.id = s.event_id
            JOIN device_contracts c
              ON c.tenant_id = e.tenant_id
             AND c.device_id = e.device_id
             AND c.id = e.contract_id
            JOIN device_blueprint_revisions r
              ON r.tenant_id = c.tenant_id
             AND r.id = c.blueprint_revision_id
            WHERE s.tenant_id = ?1
              AND s.device_id IN (SELECT value FROM json_each(?2))
              AND s.occurred_at >= ?3
              AND s.occurred_at < ?4
              AND s.stream_key = ?6
              AND s.field_path = ?7
              AND s.value_type IN ('float64', 'int64')
              AND r.blueprint_id = ?8
        ), ranked AS (
            SELECT *, row_number() OVER (
                PARTITION BY device_id, bucket_start
                ORDER BY occurred_at DESC, event_id COLLATE BINARY DESC
            ) AS latest_rank
            FROM bucketed
        )
        SELECT
            device_id,
            device_name,
            bucket_start,
            count(*) AS sample_count,
            avg(value) AS average,
            min(value) AS minimum,
            max(value) AS maximum,
            max(CASE WHEN latest_rank = 1 THEN value END) AS latest
        FROM ranked
        GROUP BY device_id, device_name, bucket_start
        ORDER BY bucket_start, device_name COLLATE BINARY, device_id COLLATE BINARY
        LIMIT ?9
        "#
}
