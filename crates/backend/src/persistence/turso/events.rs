use async_trait::async_trait;
use turso::params;

use crate::domains::events::repository::DeviceEventRepository;
use crate::domains::events::types::{
    DeviceMetricQuery, DeviceMetricRecord, MetricValue, RecordDeviceEvent, RecordDeviceEventOutcome,
};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::{TursoAdapter, row};

#[async_trait]
impl DeviceEventRepository for TursoAdapter {
    async fn record(
        &self,
        tenant: &TenantId,
        event: RecordDeviceEvent,
    ) -> Result<RecordDeviceEventOutcome, PersistenceError> {
        let mut writer = self.database.writer().await;
        let transaction = writer.transaction().await.map_err(row::error)?;
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
            .map_err(row::error)?;
        if inserted == 0 {
            transaction.rollback().await.map_err(row::error)?;
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
            .map_err(row::error)?;

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
                        metric.stream_key,
                        metric.field_path,
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
                .map_err(row::error)?;
        }
        let actions_enqueued =
            super::devices::enqueue(&transaction, &event.pending_actions).await?;
        transaction.commit().await.map_err(row::error)?;
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
        let connection = self.database.connect()?;
        let mut existence = connection
            .query(
                "SELECT EXISTS(SELECT 1 FROM devices WHERE tenant_id = ?1 AND id = ?2)",
                params![tenant.as_str(), device_id],
            )
            .await
            .map_err(row::error)?;
        let exists = existence
            .next()
            .await
            .map_err(row::error)?
            .ok_or(PersistenceError::NotFound)?
            .get::<i64>(0)
            .map_err(row::error)?
            != 0;
        drop(existence);
        if !exists {
            return Ok(None);
        }

        let mut rows = connection
            .query(
                "SELECT event_id, device_id, stream_key, field_path, value_type,
                        value_double, value_int, value_text, value_bool, value_json,
                        occurred_at
                 FROM device_metric_samples
                 WHERE tenant_id = ?1 AND device_id = ?2
                   AND (?3 IS NULL OR stream_key = ?3)
                   AND (?4 IS NULL OR field_path = ?4)
                   AND (?5 IS NULL OR occurred_at >= ?5)
                   AND (?6 IS NULL OR occurred_at < ?6)
                 ORDER BY occurred_at DESC, event_id DESC, stream_key, field_path
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
            .map_err(row::error)?;
        let mut metrics = Vec::new();
        while let Some(record) = rows.next().await.map_err(row::error)? {
            let event_id = record.get::<String>(0).map_err(row::error)?;
            let value_type = record.get::<String>(4).map_err(row::error)?;
            let value = match value_type.as_str() {
                "float64" => record
                    .get::<Option<f64>>(5)
                    .map_err(row::error)?
                    .map(MetricValue::Float64),
                "int64" => record
                    .get::<Option<i64>>(6)
                    .map_err(row::error)?
                    .map(MetricValue::Int64),
                "string" => record
                    .get::<Option<String>>(7)
                    .map_err(row::error)?
                    .map(MetricValue::String),
                "boolean" => record
                    .get::<Option<i64>>(8)
                    .map_err(row::error)?
                    .map(|value| MetricValue::Boolean(value != 0)),
                "json" => record
                    .get::<Option<String>>(9)
                    .map_err(row::error)?
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
                event_id,
                device_id: record.get(1).map_err(row::error)?,
                stream_key: record.get(2).map_err(row::error)?,
                field_path: record.get(3).map_err(row::error)?,
                value,
                occurred_at: row::datetime(record.get(10).map_err(row::error)?)?,
            });
        }
        Ok(Some(metrics))
    }
}
