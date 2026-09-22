use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Float8, Jsonb, Nullable, Text, Timestamptz};

use extrittio_backend_core::events::DeviceEventRepository;
use extrittio_backend_core::events::{
    DeviceLocationQuery, DeviceLocationRecord, DeviceMetricQuery, DeviceMetricRecord,
    MetricPruneOutcome, MetricRetentionCutoffs, MetricValue, RecordDeviceEvent,
    RecordDeviceEventOutcome,
};

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;

use crate::{PostgresExecutor, PostgresPool};
#[derive(Clone)]
pub struct PostgresEventRepository {
    executor: PostgresExecutor,
}
impl PostgresEventRepository {
    pub fn from_pool(pool: PostgresPool) -> Self {
        Self {
            executor: PostgresExecutor::new(pool),
        }
    }
}
#[derive(Debug, thiserror::Error)]
enum EventTransactionError {
    #[error(transparent)]
    Diesel(#[from] diesel::result::Error),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

use crate::outbox::enqueue_actions_in_transaction as enqueue_pending_actions;

#[derive(diesel::QueryableByName)]
struct ExistsRow {
    #[diesel(sql_type = Bool)]
    exists: bool,
}

#[derive(diesel::QueryableByName)]
struct AssignedContractRow {
    #[diesel(sql_type = Text)]
    desired_contract_id: String,
}

#[derive(diesel::QueryableByName)]
struct RetentionStateRow {
    #[diesel(sql_type = Timestamptz)]
    raw_retained_since: chrono::DateTime<chrono::Utc>,
    #[diesel(sql_type = Timestamptz)]
    rollup_retained_since: chrono::DateTime<chrono::Utc>,
}

#[derive(diesel::QueryableByName)]
struct RollupCutoffRow {
    #[diesel(sql_type = Timestamptz)]
    rollup_retained_since: chrono::DateTime<chrono::Utc>,
}

#[derive(diesel::QueryableByName)]
struct MetricRow {
    #[diesel(sql_type = Text)]
    contract_id: String,
    #[diesel(sql_type = Text)]
    event_id: String,
    #[diesel(sql_type = Text)]
    device_id: String,
    #[diesel(sql_type = Text)]
    stream_key: String,
    #[diesel(sql_type = Text)]
    field_path: String,
    #[diesel(sql_type = Text)]
    value_type: String,
    #[diesel(sql_type = Nullable<Float8>)]
    value_double: Option<f64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    value_int: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    value_text: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    value_bool: Option<bool>,
    #[diesel(sql_type = Nullable<Jsonb>)]
    value_json: Option<serde_json::Value>,
    #[diesel(sql_type = Timestamptz)]
    occurred_at: chrono::DateTime<chrono::Utc>,
}

fn decode_metric(row: MetricRow) -> Result<DeviceMetricRecord, PersistenceError> {
    let value = match row.value_type.as_str() {
        "float64" => row.value_double.map(MetricValue::Float64),
        "int64" => row.value_int.map(MetricValue::Int64),
        "string" => row.value_text.map(MetricValue::String),
        "boolean" => row.value_bool.map(MetricValue::Boolean),
        "json" => row.value_json.map(MetricValue::Json),
        _ => None,
    }
    .ok_or_else(|| {
        PersistenceError::CorruptData(format!(
            "metric '{}' has invalid value_type '{}' or missing value",
            row.event_id, row.value_type
        ))
    })?;
    Ok(DeviceMetricRecord {
        contract_id: row.contract_id,
        event_id: row.event_id,
        device_id: row.device_id,
        stream_key: row.stream_key,
        field_path: row.field_path,
        value,
        occurred_at: row.occurred_at,
    })
}

#[derive(diesel::QueryableByName)]
struct LocationRow {
    #[diesel(sql_type = Text)]
    contract_id: String,
    #[diesel(sql_type = Text)]
    event_id: String,
    #[diesel(sql_type = Float8)]
    latitude: f64,
    #[diesel(sql_type = Float8)]
    longitude: f64,
    #[diesel(sql_type = Timestamptz)]
    occurred_at: chrono::DateTime<chrono::Utc>,
    #[diesel(sql_type = Timestamptz)]
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(diesel::QueryableByName)]
struct LocatedRow {
    #[diesel(sql_type = Text)]
    device_id: String,
    #[diesel(embed)]
    location: LocationRow,
}

#[async_trait]
impl DeviceEventRepository for PostgresEventRepository {
    async fn prune_metrics(
        &self,
        cutoffs: MetricRetentionCutoffs,
    ) -> Result<MetricPruneOutcome, PersistenceError> {
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, diesel::result::Error, _>(|connection| {
                        let state = diesel::sql_query(
                            "UPDATE device_metric_retention_state
                             SET raw_retained_since = greatest(raw_retained_since, $1),
                                 rollup_retained_since = greatest(rollup_retained_since, $2)
                             WHERE id = 1
                             RETURNING raw_retained_since, rollup_retained_since",
                        )
                        .bind::<Timestamptz, _>(cutoffs.raw_retained_since)
                        .bind::<Timestamptz, _>(cutoffs.rollup_retained_since)
                        .get_result::<RetentionStateRow>(connection)?;
                        let events_deleted =
                            diesel::sql_query("DELETE FROM device_events WHERE occurred_at < $1")
                                .bind::<Timestamptz, _>(state.raw_retained_since)
                                .execute(connection)?;
                        let rollups_deleted = diesel::sql_query(
                            "DELETE FROM device_metric_rollups_hourly WHERE bucket_start < $1",
                        )
                        .bind::<Timestamptz, _>(state.rollup_retained_since)
                        .execute(connection)?;
                        let receipts_deleted = diesel::sql_query(
                            "DELETE FROM device_event_receipts WHERE occurred_at < $1",
                        )
                        .bind::<Timestamptz, _>(state.rollup_retained_since)
                        .execute(connection)?;
                        Ok(MetricPruneOutcome {
                            events_deleted: events_deleted as u64,
                            rollups_deleted: rollups_deleted as u64,
                            receipts_deleted: receipts_deleted as u64,
                        })
                    })
                    .map_err(crate::error::map_diesel_error)
            })
            .await
    }

    async fn latest_locations(
        &self,
        tenant: &TenantId,
        device_ids: Vec<String>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<extrittio_backend_core::events::LocatedDeviceRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor.run(move |connection| {
            diesel::sql_query(r#"WITH positions AS (
SELECT e.device_id,e.contract_id,e.id AS event_id,e.occurred_at,
e.occurred_at + ((c.document #>> '{location,maxAgeMs}')::double precision * INTERVAL '1 millisecond') AS expires_at,
CASE lat.value_type WHEN 'float64' THEN lat.value_double WHEN 'int64' THEN CAST(lat.value_int AS double precision) END AS latitude,
CASE lon.value_type WHEN 'float64' THEN lon.value_double WHEN 'int64' THEN CAST(lon.value_int AS double precision) END AS longitude
FROM device_events e
JOIN device_contract_assignments a ON a.tenant_id=e.tenant_id AND a.device_id=e.device_id AND a.desired_contract_id=e.contract_id
JOIN device_contracts c ON c.tenant_id=e.tenant_id AND c.device_id=e.device_id AND c.id=e.contract_id
JOIN device_metric_samples lat ON lat.tenant_id=e.tenant_id AND lat.device_id=e.device_id AND lat.event_id=e.id AND lat.occurred_at=e.occurred_at
JOIN device_metric_samples lon ON lon.tenant_id=e.tenant_id AND lon.device_id=e.device_id AND lon.event_id=e.id AND lon.occurred_at=e.occurred_at
WHERE e.tenant_id=$1 AND e.device_id=ANY($2) AND e.occurred_at <= $3
AND e.occurred_at > $3 - ((c.document #>> '{location,maxAgeMs}')::double precision * INTERVAL '1 millisecond')
AND c.document #>> '{deviceId}'=e.device_id
AND c.document #>> '{location,coordinateSystem}'='wgs84' AND c.document #>> '{location,unit}'='degrees'
AND lat.stream_key=c.document #>> '{location,stream}' AND lon.stream_key=lat.stream_key
AND lat.field_path=c.document #>> '{location,latitudePath}' AND lon.field_path=c.document #>> '{location,longitudePath}'
), ranked AS (
SELECT *,row_number() OVER (PARTITION BY device_id ORDER BY occurred_at DESC,event_id COLLATE "C" DESC) AS position_rank
FROM positions WHERE latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180
)
SELECT device_id,contract_id,event_id,occurred_at,expires_at,latitude,longitude FROM ranked WHERE position_rank=1 ORDER BY device_id"#)
                .bind::<Text,_>(tenant_id)
                .bind::<diesel::sql_types::Array<Text>,_>(device_ids)
                .bind::<Timestamptz,_>(now)
                .load::<LocatedRow>(connection)
                .map(|rows|rows.into_iter().map(|r|extrittio_backend_core::events::LocatedDeviceRecord {
                    device_id:r.device_id,location:DeviceLocationRecord {
                        contract_id:r.location.contract_id,event_id:r.location.event_id,
                        latitude:r.location.latitude,longitude:r.location.longitude,occurred_at:r.location.occurred_at,
                        expires_at:r.location.expires_at,
                    }
                }).collect()).map_err(crate::error::map_diesel_error)
        }).await
    }

    async fn latest_location(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: DeviceLocationQuery,
    ) -> Result<Option<DeviceLocationRecord>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor.run(move |connection| {
            diesel::sql_query(r#"SELECT * FROM (SELECT e.contract_id, e.id AS event_id, e.occurred_at,
    e.occurred_at + ($8 - $7) AS expires_at,
    CASE lat.value_type WHEN 'float64' THEN lat.value_double WHEN 'int64' THEN CAST(lat.value_int AS double precision) END AS latitude,
    CASE lon.value_type WHEN 'float64' THEN lon.value_double WHEN 'int64' THEN CAST(lon.value_int AS double precision) END AS longitude
FROM device_events e
JOIN device_contract_assignments a
  ON a.tenant_id = e.tenant_id AND a.device_id = e.device_id AND a.desired_contract_id = e.contract_id
JOIN device_metric_samples lat
  ON lat.tenant_id = e.tenant_id AND lat.device_id = e.device_id AND lat.event_id = e.id AND lat.occurred_at = e.occurred_at
JOIN device_metric_samples lon
  ON lon.tenant_id = e.tenant_id AND lon.device_id = e.device_id AND lon.event_id = e.id AND lon.occurred_at = e.occurred_at
WHERE e.tenant_id = $1 AND e.device_id = $2 AND e.contract_id = $3
  AND lat.stream_key = $4 AND lon.stream_key = $4
  AND lat.field_path = $5 AND lon.field_path = $6
  AND e.occurred_at > $7 AND e.occurred_at <= $8) AS positions
WHERE latitude BETWEEN -90 AND 90 AND longitude BETWEEN -180 AND 180
ORDER BY occurred_at DESC, event_id COLLATE "C" DESC LIMIT 1"#)
                .bind::<Text, _>(tenant_id)
                .bind::<Text, _>(device_id)
                .bind::<Text, _>(query.contract_id)
                .bind::<Text, _>(query.stream_key)
                .bind::<Text, _>(query.latitude_path)
                .bind::<Text, _>(query.longitude_path)
                .bind::<Timestamptz, _>(query.since)
                .bind::<Timestamptz, _>(query.now)
                .get_result::<LocationRow>(connection).optional()
                .map(|result| result.map(|r| DeviceLocationRecord {
                    contract_id: r.contract_id, event_id: r.event_id,
                    latitude: r.latitude, longitude: r.longitude, occurred_at: r.occurred_at,
                    expires_at: r.expires_at,
                }))
                .map_err(crate::error::map_diesel_error)
        }).await
    }

    async fn record(
        &self,
        tenant: &TenantId,
        event: RecordDeviceEvent,
    ) -> Result<RecordDeviceEventOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, EventTransactionError, _>(|connection| {
                        let retention = diesel::sql_query(
                            "SELECT rollup_retained_since FROM device_metric_retention_state
                             WHERE id = 1 FOR SHARE",
                        )
                        .get_result::<RollupCutoffRow>(connection)?;
                        if event.occurred_at < retention.rollup_retained_since {
                            return Err(PersistenceError::HistoryExpired.into());
                        }
                        use crate::schema::devices;
                        devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&event.device_id))
                            .for_update()
                            .select(devices::id)
                            .first::<String>(connection)?;
                        let desired_contract = diesel::sql_query(
                            "SELECT desired_contract_id FROM device_contract_assignments
                             WHERE tenant_id = $1 AND device_id = $2 FOR UPDATE",
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&event.device_id)
                        .get_result::<AssignedContractRow>(connection)
                        .optional()?;
                        if desired_contract
                            .as_ref()
                            .map(|row| row.desired_contract_id.as_str())
                            != Some(event.contract_id.as_str())
                        {
                            return Err(PersistenceError::NotFound.into());
                        }

                        let inserted = diesel::sql_query(
                            "INSERT INTO device_event_receipts
                                (id, tenant_id, device_id, occurred_at, received_at)
                             VALUES ($1, $2, $3, $4, $5)
                             ON CONFLICT (id) DO NOTHING",
                        )
                        .bind::<Text, _>(&event.event_id)
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&event.device_id)
                        .bind::<Timestamptz, _>(event.occurred_at)
                        .bind::<Timestamptz, _>(event.received_at)
                        .execute(connection)?;
                        if inserted == 0 {
                            return Ok(RecordDeviceEventOutcome {
                                recorded: false,
                                metrics_recorded: 0,
                                actions_enqueued: 0,
                            });
                        }

                        diesel::sql_query(
                            "INSERT INTO device_events
                                (id, tenant_id, device_id, contract_id, route_key,
                                 occurred_at, received_at, payload)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        )
                        .bind::<Text, _>(&event.event_id)
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&event.device_id)
                        .bind::<Text, _>(&event.contract_id)
                        .bind::<Text, _>(&event.route_key)
                        .bind::<Timestamptz, _>(event.occurred_at)
                        .bind::<Timestamptz, _>(event.received_at)
                        .bind::<Jsonb, _>(&event.payload)
                        .execute(connection)?;

                        diesel::sql_query(
                            "UPDATE device_contract_assignments
                             SET active_contract_id = desired_contract_id,
                                 status = 'converged', acknowledged_at = $4,
                                 error = NULL, updated_at = $4
                             WHERE tenant_id = $1 AND device_id = $2
                               AND desired_contract_id = $3",
                        )
                        .bind::<Text, _>(&tenant_id)
                        .bind::<Text, _>(&event.device_id)
                        .bind::<Text, _>(&event.contract_id)
                        .bind::<Timestamptz, _>(event.received_at)
                        .execute(connection)?;

                        let metrics_recorded = event.metrics.len();
                        for metric in event.metrics {
                            let value_type = metric.value.value_type();
                            let (value_double, value_int, value_text, value_bool, value_json) =
                                match metric.value {
                                    MetricValue::Float64(value) => {
                                        (Some(value), None, None, None, None)
                                    }
                                    MetricValue::Int64(value) => {
                                        (None, Some(value), None, None, None)
                                    }
                                    MetricValue::String(value) => {
                                        (None, None, Some(value), None, None)
                                    }
                                    MetricValue::Boolean(value) => {
                                        (None, None, None, Some(value), None)
                                    }
                                    MetricValue::Json(value) => {
                                        (None, None, None, None, Some(value))
                                    }
                                };
                            diesel::sql_query(
                                "INSERT INTO device_metric_samples
                                    (event_id, tenant_id, device_id, stream_key, field_path,
                                     value_type, value_double, value_int, value_text, value_bool,
                                     value_json, occurred_at)
                                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                            )
                            .bind::<Text, _>(&event.event_id)
                            .bind::<Text, _>(&tenant_id)
                            .bind::<Text, _>(&event.device_id)
                            .bind::<Text, _>(&metric.stream_key)
                            .bind::<Text, _>(&metric.field_path)
                            .bind::<Text, _>(value_type)
                            .bind::<Nullable<Float8>, _>(value_double)
                            .bind::<Nullable<BigInt>, _>(value_int)
                            .bind::<Nullable<Text>, _>(value_text)
                            .bind::<Nullable<Bool>, _>(value_bool)
                            .bind::<Nullable<Jsonb>, _>(value_json)
                            .bind::<Timestamptz, _>(event.occurred_at)
                            .execute(connection)?;
                            if let Some(value) = value_double.or_else(|| value_int.map(|v| v as f64)) {
                                diesel::sql_query(
                                    "INSERT INTO device_metric_rollups_hourly
                                        (tenant_id, device_id, blueprint_revision_id, stream_key,
                                         field_path, bucket_start, sample_count, value_sum,
                                         value_min, value_max, latest_value, latest_at, latest_event_id)
                                     SELECT $1, $2, c.blueprint_revision_id, $4, $5,
                                            to_timestamp(floor(extract(epoch FROM $7::timestamptz) / 3600) * 3600),
                                            1, $6, $6, $6, $6, $7, $8
                                     FROM device_contracts c
                                     WHERE c.tenant_id = $1 AND c.device_id = $2 AND c.id = $3
                                     ON CONFLICT (tenant_id, device_id, blueprint_revision_id,
                                                  stream_key, field_path, bucket_start)
                                     DO UPDATE SET
                                         sample_count = device_metric_rollups_hourly.sample_count + 1,
                                         value_sum = device_metric_rollups_hourly.value_sum + EXCLUDED.value_sum,
                                         value_min = least(device_metric_rollups_hourly.value_min, EXCLUDED.value_min),
                                         value_max = greatest(device_metric_rollups_hourly.value_max, EXCLUDED.value_max),
                                         latest_value = CASE WHEN EXCLUDED.latest_at > device_metric_rollups_hourly.latest_at
                                             OR (EXCLUDED.latest_at = device_metric_rollups_hourly.latest_at
                                                 AND EXCLUDED.latest_event_id COLLATE \"C\" > device_metric_rollups_hourly.latest_event_id COLLATE \"C\")
                                             THEN EXCLUDED.latest_value ELSE device_metric_rollups_hourly.latest_value END,
                                         latest_at = greatest(device_metric_rollups_hourly.latest_at, EXCLUDED.latest_at),
                                         latest_event_id = CASE WHEN EXCLUDED.latest_at > device_metric_rollups_hourly.latest_at
                                             OR (EXCLUDED.latest_at = device_metric_rollups_hourly.latest_at
                                                 AND EXCLUDED.latest_event_id COLLATE \"C\" > device_metric_rollups_hourly.latest_event_id COLLATE \"C\")
                                             THEN EXCLUDED.latest_event_id ELSE device_metric_rollups_hourly.latest_event_id END",
                                )
                                .bind::<Text, _>(&tenant_id)
                                .bind::<Text, _>(&event.device_id)
                                .bind::<Text, _>(&event.contract_id)
                                .bind::<Text, _>(&metric.stream_key)
                                .bind::<Text, _>(&metric.field_path)
                                .bind::<Float8, _>(value)
                                .bind::<Timestamptz, _>(event.occurred_at)
                                .bind::<Text, _>(&event.event_id)
                                .execute(connection)?;
                            }
                        }
                        let actions = crate::rule_runtime::evaluate_rules_in_transaction(
                            connection,
                            &tenant_id,
                            &event.device_id,
                            Some(&event.rule_evaluation),
                        )?;
                        let actions_enqueued = enqueue_pending_actions(connection, &actions)?;
                        Ok(RecordDeviceEventOutcome {
                            recorded: true,
                            metrics_recorded,
                            actions_enqueued,
                        })
                    })
                    .map_err(|error| match error {
                        EventTransactionError::Diesel(error) => crate::error::map_diesel_error(error),
                        EventTransactionError::Persistence(error) => error,
                    })
            })
            .await
    }

    async fn list_metrics(
        &self,
        tenant: &TenantId,
        device_id: &str,
        query: DeviceMetricQuery,
    ) -> Result<Option<Vec<DeviceMetricRecord>>, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        let device_id = device_id.to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .build_transaction()
                    .read_only()
                    .repeatable_read()
                    .run::<_, EventTransactionError, _>(|connection| {
                let device_exists = diesel::sql_query(
                    "SELECT EXISTS(
                        SELECT 1 FROM devices WHERE tenant_id = $1 AND id = $2
                     ) AS exists",
                )
                .bind::<Text, _>(&tenant_id)
                .bind::<Text, _>(&device_id)
                .get_result::<ExistsRow>(connection)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?
                .exists;
                if !device_exists {
                    return Ok(None);
                }

                let retention = diesel::sql_query(
                    "SELECT raw_retained_since, rollup_retained_since
                     FROM device_metric_retention_state WHERE id = 1",
                )
                .get_result::<RetentionStateRow>(connection)?;
                let raw_since = retention.raw_retained_since.naive_utc();
                if query.since.is_some_and(|since| since < raw_since)
                    || query.before.is_some_and(|before| before <= raw_since)
                {
                    return Err(PersistenceError::HistoryExpired.into());
                }

                let rows = diesel::sql_query(
                    "SELECT event_id, device_id, stream_key, field_path, value_type,
                            value_double, value_int, value_text, value_bool, value_json,
                            occurred_at,
                            (SELECT contract_id FROM device_events e WHERE e.tenant_id = device_metric_samples.tenant_id AND e.id = device_metric_samples.event_id) AS contract_id
                     FROM device_metric_samples
                     WHERE tenant_id = $1 AND device_id = $2
                       AND ($3::text IS NULL OR stream_key = $3)
                       AND ($4::text IS NULL OR field_path = $4)
                       AND ($5::timestamptz IS NULL OR occurred_at >= $5)
                       AND ($6::timestamptz IS NULL OR occurred_at < $6)
                     ORDER BY occurred_at DESC, event_id COLLATE \"C\" DESC, stream_key COLLATE \"C\", field_path COLLATE \"C\"
                     LIMIT $7",
                )
                .bind::<Text, _>(&tenant_id)
                .bind::<Text, _>(&device_id)
                .bind::<Nullable<Text>, _>(query.stream_key)
                .bind::<Nullable<Text>, _>(query.field_path)
                .bind::<Nullable<Timestamptz>, _>(query.since.map(|value| value.and_utc()))
                .bind::<Nullable<Timestamptz>, _>(query.before.map(|value| value.and_utc()))
                .bind::<BigInt, _>(query.limit)
                .load::<MetricRow>(connection)
                .map_err(|error| PersistenceError::Internal(error.to_string()))?;
                Ok(Some(rows.into_iter()
                    .map(decode_metric)
                    .collect::<Result<Vec<_>, _>>()?))
                    })
                    .map_err(|error| match error {
                        EventTransactionError::Diesel(error) => crate::error::map_diesel_error(error),
                        EventTransactionError::Persistence(error) => error,
                    })
            })
            .await
    }
}
