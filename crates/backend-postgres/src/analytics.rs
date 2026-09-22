use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{Array, BigInt, Float8, Integer, Jsonb, Text, Timestamptz};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::analytics::AnalyticsRepository;
use extrittio_backend_core::analytics::full_hour_rollup_window;
use extrittio_backend_core::analytics::{
    AnalyticsBlueprintRevision, AnalyticsBucket, AnalyticsDataSource, AnalyticsDevice,
    AnalyticsQuery, AnalyticsQueryData,
};

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresAnalyticsRepository {
    executor: PostgresExecutor,
}
impl PostgresAnalyticsRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
use crate::error::map_diesel_error;

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct BlueprintRevisionRow {
    #[diesel(sql_type = Text)]
    blueprint_id: String,
    #[diesel(sql_type = Text)]
    blueprint_key: String,
    #[diesel(sql_type = Text)]
    blueprint_name: String,
    #[diesel(sql_type = Text)]
    revision_id: String,
    #[diesel(sql_type = Integer)]
    revision: i32,
    #[diesel(sql_type = Jsonb)]
    document: serde_json::Value,
}

#[derive(QueryableByName)]
struct DeviceRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct BucketRow {
    #[diesel(sql_type = Text)]
    device_id: String,
    #[diesel(sql_type = Text)]
    device_name: String,
    #[diesel(sql_type = Timestamptz)]
    bucket_start: chrono::NaiveDateTime,
    #[diesel(sql_type = BigInt)]
    sample_count: i64,
    #[diesel(sql_type = Float8)]
    average: f64,
    #[diesel(sql_type = Float8)]
    minimum: f64,
    #[diesel(sql_type = Float8)]
    maximum: f64,
    #[diesel(sql_type = Float8)]
    latest: f64,
}

#[async_trait]
impl AnalyticsRepository for PostgresAnalyticsRepository {
    async fn blueprint_catalog(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<AnalyticsBlueprintRevision>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                diesel::sql_query(
                    r#"
                    SELECT
                        b.id AS blueprint_id,
                        b.blueprint_key,
                        b.name AS blueprint_name,
                        r.id AS revision_id,
                        r.revision,
                        r.document
                    FROM device_blueprints b
                    JOIN device_blueprint_revisions r
                      ON r.tenant_id = b.tenant_id
                     AND r.blueprint_id = b.id
                    WHERE b.tenant_id = $1
                    ORDER BY b.id, r.revision DESC
                    "#,
                )
                .bind::<Text, _>(&tenant_id)
                .load::<BlueprintRevisionRow>(connection)
                .map_err(map_diesel_error)
                .map(|rows| {
                    rows.into_iter()
                        .map(|row| AnalyticsBlueprintRevision {
                            blueprint_id: row.blueprint_id,
                            blueprint_key: row.blueprint_key,
                            blueprint_name: row.blueprint_name,
                            revision_id: row.revision_id,
                            revision: row.revision,
                            document: row.document,
                        })
                        .collect()
                })
            })
            .await
    }

    async fn query(
        &self,
        tenant: &TenantId,
        query: AnalyticsQuery,
    ) -> Result<AnalyticsQueryData, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .build_transaction()
                    .read_only()
                    .repeatable_read()
                    .run::<_, AnalyticsReadError, _>(|connection| {
                        let scope_count = diesel::sql_query(
                            r#"
                    SELECT count(*)::bigint AS count
                    FROM devices d
                    WHERE d.tenant_id = $1
                      AND (cardinality($2) = 0 OR d.fleet_id = ANY($2))
                      AND (cardinality($3) = 0 OR d.id = ANY($3))
                    "#,
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Array<Integer>, _>(&query.scope.fleet_ids)
                        .bind::<Array<Text>, _>(&query.scope.device_ids)
                        .get_result::<CountRow>(connection)
                        .map_err(map_diesel_error)?
                        .count;
                        let selected_devices = usize::try_from(scope_count).unwrap_or(usize::MAX);

                        let compatible_count = diesel::sql_query(
                            r#"
                    SELECT count(*)::bigint AS count
                    FROM devices d
                    WHERE d.tenant_id = $1
                      AND (cardinality($2) = 0 OR d.fleet_id = ANY($2))
                      AND (cardinality($3) = 0 OR d.id = ANY($3))
                      AND (
                        EXISTS (
                          SELECT 1 FROM device_contract_assignments a
                          JOIN device_contracts c ON c.tenant_id = a.tenant_id
                            AND c.device_id = a.device_id AND c.id = a.desired_contract_id
                          WHERE a.tenant_id = d.tenant_id AND a.device_id = d.id
                            AND c.blueprint_revision_id = ANY($4)
                        ) OR EXISTS (
                          SELECT 1 FROM device_metric_rollups_hourly h
                          WHERE h.tenant_id = d.tenant_id AND h.device_id = d.id
                            AND h.blueprint_revision_id = ANY($4)
                            AND h.stream_key = $5 AND h.field_path = $6
                        )
                      )
                    "#,
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Array<Integer>, _>(&query.scope.fleet_ids)
                        .bind::<Array<Text>, _>(&query.scope.device_ids)
                        .bind::<Array<Text>, _>(&query.compatible_revision_ids)
                        .bind::<Text, _>(&query.metric.selector.stream_key)
                        .bind::<Text, _>(&query.metric.selector.field_path)
                        .get_result::<CountRow>(connection)
                        .map_err(map_diesel_error)?
                        .count;
                        let compatible_devices =
                            usize::try_from(compatible_count).unwrap_or(usize::MAX);

                        let device_limit =
                            i64::try_from(query.max_devices.saturating_add(1)).unwrap_or(i64::MAX);
                        let device_rows = diesel::sql_query(
                            r#"
                    SELECT d.id, d.name
                    FROM devices d
                    WHERE d.tenant_id = $1
                      AND (cardinality($2) = 0 OR d.fleet_id = ANY($2))
                      AND (cardinality($3) = 0 OR d.id = ANY($3))
                      AND (
                        EXISTS (
                          SELECT 1 FROM device_contract_assignments a
                          JOIN device_contracts c ON c.tenant_id = a.tenant_id
                            AND c.device_id = a.device_id AND c.id = a.desired_contract_id
                          WHERE a.tenant_id = d.tenant_id AND a.device_id = d.id
                            AND c.blueprint_revision_id = ANY($4)
                        ) OR EXISTS (
                          SELECT 1 FROM device_metric_rollups_hourly h
                          WHERE h.tenant_id = d.tenant_id AND h.device_id = d.id
                            AND h.blueprint_revision_id = ANY($4)
                            AND h.stream_key = $5 AND h.field_path = $6
                        )
                      )
                    ORDER BY d.name COLLATE "C", d.id COLLATE "C"
                    LIMIT $7
                    "#,
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Array<Integer>, _>(&query.scope.fleet_ids)
                        .bind::<Array<Text>, _>(&query.scope.device_ids)
                        .bind::<Array<Text>, _>(&query.compatible_revision_ids)
                        .bind::<Text, _>(&query.metric.selector.stream_key)
                        .bind::<Text, _>(&query.metric.selector.field_path)
                        .bind::<BigInt, _>(device_limit)
                        .load::<DeviceRow>(connection)
                        .map_err(map_diesel_error)?;
                        let devices: Vec<_> = device_rows
                            .into_iter()
                            .map(|row| AnalyticsDevice {
                                id: row.id,
                                name: row.name,
                            })
                            .collect();

                        if compatible_devices > query.max_devices || devices.is_empty() {
                            return Ok(AnalyticsQueryData {
                                source: AnalyticsDataSource::BlueprintMetricSamples,
                                selected_devices,
                                compatible_devices,
                                devices,
                                buckets: Vec::new(),
                            });
                        }

                        let device_ids: Vec<_> =
                            devices.iter().map(|device| device.id.clone()).collect();
                        let row_limit =
                            i64::try_from(query.max_rows.saturating_add(1)).unwrap_or(i64::MAX);
                        let rollup_window = (query.bucket_seconds >= 3_600)
                            .then(|| {
                                full_hour_rollup_window(
                                    query.start,
                                    query.end,
                                    chrono::Utc::now().naive_utc(),
                                )
                            })
                            .flatten();
                        let rows = if let Some((rollup_start, rollup_end)) = rollup_window {
                            diesel::sql_query(metric_rollups_query())
                                .bind::<Text, _>(&tenant_id)
                                .bind::<Array<Text>, _>(&device_ids)
                                .bind::<Timestamptz, _>(query.start)
                                .bind::<Timestamptz, _>(query.end)
                                .bind::<BigInt, _>(query.bucket_seconds)
                                .bind::<Text, _>(&query.metric.selector.stream_key)
                                .bind::<Text, _>(&query.metric.selector.field_path)
                                .bind::<Array<Text>, _>(&query.compatible_revision_ids)
                                .bind::<BigInt, _>(row_limit)
                                .bind::<Timestamptz, _>(rollup_start)
                                .bind::<Timestamptz, _>(rollup_end)
                                .load::<BucketRow>(connection)
                                .map_err(map_diesel_error)?
                        } else {
                            diesel::sql_query(metric_samples_query())
                                .bind::<Text, _>(&tenant_id)
                                .bind::<Array<Text>, _>(&device_ids)
                                .bind::<Timestamptz, _>(query.start)
                                .bind::<Timestamptz, _>(query.end)
                                .bind::<BigInt, _>(query.bucket_seconds)
                                .bind::<Text, _>(&query.metric.selector.stream_key)
                                .bind::<Text, _>(&query.metric.selector.field_path)
                                .bind::<Array<Text>, _>(&query.compatible_revision_ids)
                                .bind::<BigInt, _>(row_limit)
                                .load::<BucketRow>(connection)
                                .map_err(map_diesel_error)?
                        };
                        let buckets = rows
                            .into_iter()
                            .map(|row| AnalyticsBucket {
                                device_id: row.device_id,
                                device_name: row.device_name,
                                bucket_start: row.bucket_start,
                                sample_count: row.sample_count,
                                average: row.average,
                                minimum: row.minimum,
                                maximum: row.maximum,
                                latest: row.latest,
                            })
                            .collect();

                        Ok(AnalyticsQueryData {
                            source: if rollup_window.is_some() {
                                AnalyticsDataSource::BlueprintMetricSamplesAndRollups
                            } else {
                                AnalyticsDataSource::BlueprintMetricSamples
                            },
                            selected_devices,
                            compatible_devices,
                            devices,
                            buckets,
                        })
                    })
                    .map_err(|error| match error {
                        AnalyticsReadError::Diesel(error) => map_diesel_error(error),
                        AnalyticsReadError::Persistence(error) => error,
                    })
            })
            .await
    }
}

fn metric_samples_query() -> &'static str {
    r#"
        WITH bucketed AS (
            SELECT
                d.id AS device_id,
                d.name AS device_name,
                to_timestamp(floor(extract(epoch FROM s.occurred_at) / $5) * $5) AS bucket_start,
                coalesce(s.value_double, s.value_int::double precision) AS value,
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
            WHERE s.tenant_id = $1
              AND s.device_id = ANY($2)
              AND s.occurred_at >= $3
              AND s.occurred_at < $4
              AND s.stream_key = $6
              AND s.field_path = $7
              AND s.value_type IN ('float64', 'int64')
              AND r.id = ANY($8)
        ), ranked AS (
            SELECT *, row_number() OVER (
                PARTITION BY device_id, bucket_start
                ORDER BY occurred_at DESC, event_id COLLATE "C" DESC
            ) AS latest_rank
            FROM bucketed
        )
        SELECT
            device_id,
            device_name,
            bucket_start,
            count(*)::bigint AS sample_count,
            avg(value)::double precision AS average,
            min(value)::double precision AS minimum,
            max(value)::double precision AS maximum,
            max(value) FILTER (WHERE latest_rank = 1)::double precision AS latest
        FROM ranked
        GROUP BY device_id, device_name, bucket_start
        ORDER BY bucket_start, device_name COLLATE "C", device_id COLLATE "C"
        LIMIT $9
        "#
}

fn metric_rollups_query() -> &'static str {
    r#"
        WITH observations AS (
            SELECT d.id AS device_id, d.name AS device_name,
                   to_timestamp(floor(extract(epoch FROM s.occurred_at) / $5) * $5) AS bucket_start,
                   1::bigint AS sample_count,
                   coalesce(s.value_double, s.value_int::double precision) AS value_sum,
                   coalesce(s.value_double, s.value_int::double precision) AS value_min,
                   coalesce(s.value_double, s.value_int::double precision) AS value_max,
                   coalesce(s.value_double, s.value_int::double precision) AS latest_value,
                   s.occurred_at AS latest_at, s.event_id AS latest_event_id
            FROM device_metric_samples s
            JOIN devices d ON d.tenant_id = s.tenant_id AND d.id = s.device_id
            JOIN device_events e ON e.tenant_id = s.tenant_id AND e.device_id = s.device_id AND e.id = s.event_id
            JOIN device_contracts c ON c.tenant_id = e.tenant_id AND c.device_id = e.device_id AND c.id = e.contract_id
            WHERE s.tenant_id = $1 AND s.device_id = ANY($2)
              AND s.occurred_at >= $3 AND s.occurred_at < $4
              AND NOT (s.occurred_at >= $10 AND s.occurred_at < $11)
              AND s.stream_key = $6 AND s.field_path = $7
              AND s.value_type IN ('float64', 'int64')
              AND c.blueprint_revision_id = ANY($8)
            UNION ALL
            SELECT d.id, d.name,
                   to_timestamp(floor(extract(epoch FROM r.bucket_start) / $5) * $5),
                   r.sample_count, r.value_sum, r.value_min, r.value_max,
                   r.latest_value, r.latest_at, r.latest_event_id
            FROM device_metric_rollups_hourly r
            JOIN devices d ON d.tenant_id = r.tenant_id AND d.id = r.device_id
            WHERE r.tenant_id = $1 AND r.device_id = ANY($2)
              AND r.bucket_start >= $10 AND r.bucket_start < $11
              AND r.stream_key = $6 AND r.field_path = $7
              AND r.blueprint_revision_id = ANY($8)
        ), ranked AS (
            SELECT *, row_number() OVER (
                PARTITION BY device_id, bucket_start
                ORDER BY latest_at DESC, latest_event_id COLLATE "C" DESC
            ) AS latest_rank
            FROM observations
        )
        SELECT device_id, device_name, bucket_start,
               sum(sample_count)::bigint AS sample_count,
               (sum(value_sum) / sum(sample_count))::double precision AS average,
               min(value_min)::double precision AS minimum,
               max(value_max)::double precision AS maximum,
               max(latest_value) FILTER (WHERE latest_rank = 1)::double precision AS latest
        FROM ranked
        GROUP BY device_id, device_name, bucket_start
        ORDER BY bucket_start, device_name COLLATE "C", device_id COLLATE "C"
        LIMIT $9
    "#
}

#[derive(Debug, thiserror::Error)]
enum AnalyticsReadError {
    #[error(transparent)]
    Diesel(#[from] diesel::result::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}
