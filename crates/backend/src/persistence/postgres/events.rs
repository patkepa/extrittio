use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Float8, Jsonb, Nullable, Text, Timestamptz};

use crate::domains::events::repository::DeviceEventRepository;
use crate::domains::events::types::{
    DeviceMetricQuery, DeviceMetricRecord, MetricValue, RecordDeviceEvent, RecordDeviceEventOutcome,
};
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::outbox::enqueue_pending_actions;

#[derive(diesel::QueryableByName)]
struct ExistsRow {
    #[diesel(sql_type = Bool)]
    exists: bool,
}

#[derive(diesel::QueryableByName)]
struct MetricRow {
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
        event_id: row.event_id,
        device_id: row.device_id,
        stream_key: row.stream_key,
        field_path: row.field_path,
        value,
        occurred_at: row.occurred_at,
    })
}

#[async_trait]
impl DeviceEventRepository for PostgresAdapter {
    async fn record(
        &self,
        tenant: &TenantId,
        event: RecordDeviceEvent,
    ) -> Result<RecordDeviceEventOutcome, PersistenceError> {
        let tenant_id = tenant.as_str().to_owned();
        self.executor
            .run(move |connection| {
                connection
                    .transaction::<_, AppError, _>(|connection| {
                        use crate::db::schema::devices;
                        devices::table
                            .filter(devices::tenant_id.eq(&tenant_id))
                            .filter(devices::id.eq(&event.device_id))
                            .for_update()
                            .select(devices::id)
                            .first::<String>(connection)?;

                        let inserted = diesel::sql_query(
                            "INSERT INTO device_events
                                (id, tenant_id, device_id, contract_id, route_key,
                                 occurred_at, received_at, payload)
                             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                             ON CONFLICT (id) DO NOTHING",
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
                        if inserted == 0 {
                            return Ok(RecordDeviceEventOutcome {
                                recorded: false,
                                metrics_recorded: 0,
                                actions_enqueued: 0,
                            });
                        }

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
                            .bind::<Text, _>(metric.stream_key)
                            .bind::<Text, _>(metric.field_path)
                            .bind::<Text, _>(value_type)
                            .bind::<Nullable<Float8>, _>(value_double)
                            .bind::<Nullable<BigInt>, _>(value_int)
                            .bind::<Nullable<Text>, _>(value_text)
                            .bind::<Nullable<Bool>, _>(value_bool)
                            .bind::<Nullable<Jsonb>, _>(value_json)
                            .bind::<Timestamptz, _>(event.occurred_at)
                            .execute(connection)?;
                        }
                        let actions = crate::database::postgres_ingress_rules(
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
                    .map_err(|error| PersistenceError::Internal(error.to_string()))
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

                let rows = diesel::sql_query(
                    "SELECT event_id, device_id, stream_key, field_path, value_type,
                            value_double, value_int, value_text, value_bool, value_json,
                            occurred_at
                     FROM device_metric_samples
                     WHERE tenant_id = $1 AND device_id = $2
                       AND ($3::text IS NULL OR stream_key = $3)
                       AND ($4::text IS NULL OR field_path = $4)
                       AND ($5::timestamptz IS NULL OR occurred_at >= $5)
                       AND ($6::timestamptz IS NULL OR occurred_at < $6)
                     ORDER BY occurred_at DESC, event_id DESC, stream_key, field_path
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
                rows.into_iter()
                    .map(decode_metric)
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some)
            })
            .await
    }
}
