use async_trait::async_trait;
use turso::params;

use extrittio_backend_core::PersistenceError;
use extrittio_backend_core::TenantId;
use extrittio_backend_core::activity::ActivityRepository;
use extrittio_backend_core::activity::{ActivityEventPage, ActivityEventRecord, ActivityQuery};

use crate::{TursoConnectionHandles, row};
#[derive(Clone)]
pub struct TursoActivityRepository {
    handles: TursoConnectionHandles,
}
impl TursoActivityRepository {
    pub fn from_handles(handles: TursoConnectionHandles) -> Self {
        Self { handles }
    }
}

const ACTIVITY_QUERY: &str = r#"
WITH activity AS (
    SELECT
        'device:' || CAST(logs.id AS TEXT) AS id,
        'device' AS source,
        CASE upper(logs.level)
            WHEN 'DEBUG' THEN 'debug'
            WHEN 'WARN' THEN 'warning'
            WHEN 'ERROR' THEN 'error'
            ELSE 'info'
        END AS severity,
        'device.log' AS event_type,
        'device' AS category,
        logs.message AS message,
        'device' AS actor_type,
        logs.device_id AS actor_id,
        'device' AS resource_type,
        logs.device_id AS resource_id,
        NULL AS request_id,
        json_object('level', logs.level) AS metadata,
        logs.created_at AS occurred_at
    FROM device_logs logs
    WHERE logs.tenant_id = ?1

    UNION ALL

    SELECT
        'audit:' || audit.id AS id,
        'audit' AS source,
        CASE WHEN audit.outcome = 'success' THEN 'info' ELSE 'error' END AS severity,
        audit.action AS event_type,
        audit.resource_type AS category,
        COALESCE(NULLIF(CASE WHEN json_type(audit.metadata, '$.method') = 'text' THEN json_extract(audit.metadata, '$.method') END, ''), upper(CASE WHEN instr(audit.action, '.') = 0 THEN ''
                ELSE substr(substr(audit.action, instr(audit.action, '.') + 1), 1,
                    instr(substr(audit.action, instr(audit.action, '.') + 1) || '.', '.') - 1) END))
            || ' ' || COALESCE(NULLIF(CASE WHEN json_type(audit.metadata, '$.path') = 'text' THEN json_extract(audit.metadata, '$.path') END, ''), audit.resource_type) AS message,
        audit.actor_type AS actor_type,
        audit.actor_id AS actor_id,
        audit.resource_type AS resource_type,
        audit.resource_id AS resource_id,
        audit.request_id AS request_id,
        audit.metadata AS metadata,
        audit.occurred_at AS occurred_at
    FROM audit_events audit
    WHERE audit.tenant_id = ?1

    UNION ALL

    SELECT
        'alert:' || alert.id AS id,
        'alert' AS source,
        CASE lower(alert.severity)
            WHEN 'critical' THEN 'error'
            WHEN 'warning' THEN 'warning'
            ELSE 'info'
        END AS severity,
        'alert.' || lower(alert.status) AS event_type,
        'alerts' AS category,
        alert.message AS message,
        'rule' AS actor_type,
        alert.rule_id AS actor_id,
        'device' AS resource_type,
        alert.device_id AS resource_id,
        NULL AS request_id,
        json_object(
            'rule_id', alert.rule_id,
            'status', alert.status,
            'triggered_value', alert.triggered_value
        ) AS metadata,
        COALESCE(alert.resolved_at, alert.acknowledged_at, alert.created_at) AS occurred_at
    FROM alerts alert
    WHERE alert.tenant_id = ?1

    UNION ALL

    SELECT
        'command:' || command.id AS id,
        'command' AS source,
        CASE lower(command.status)
            WHEN 'failed' THEN 'error'
            WHEN 'timed_out' THEN 'error'
            WHEN 'timeout' THEN 'error'
            WHEN 'pending' THEN 'debug'
            ELSE 'info'
        END AS severity,
        'command.' || lower(command.status) AS event_type,
        'commands' AS category,
        'Command ' || command.command || ' ' || replace(command.status, '_', ' ') AS message,
        'system' AS actor_type,
        NULL AS actor_id,
        'device' AS resource_type,
        command.device_id AS resource_id,
        NULL AS request_id,
        json_object(
            'command', command.command,
            'params', json(command.params),
            'status', command.status,
            'response_payload', json(command.response_payload)
        ) AS metadata,
        command.updated_at AS occurred_at
    FROM command_history command
    WHERE command.tenant_id = ?1

    UNION ALL

    SELECT
        'deployment:' || CAST(deployment.id AS TEXT) AS id,
        'deployment' AS source,
        CASE lower(deployment.status)
            WHEN 'failed' THEN 'error'
            WHEN 'pending' THEN 'debug'
            ELSE 'info'
        END AS severity,
        'ota.' || lower(deployment.status) AS event_type,
        'firmware' AS category,
        'Firmware deployment ' || replace(deployment.status, '_', ' ') AS message,
        'system' AS actor_type,
        NULL AS actor_id,
        'device' AS resource_type,
        deployment.device_id AS resource_id,
        NULL AS request_id,
        json_object(
            'firmware_update_id', deployment.firmware_update_id,
            'status', deployment.status,
            'error_message', deployment.error_message
        ) AS metadata,
        COALESCE(deployment.completed_at, deployment.initiated_at) AS occurred_at
    FROM ota_deployments deployment
    WHERE deployment.tenant_id = ?1
), filtered AS (
    SELECT *
    FROM activity
    WHERE (?2 IS NULL OR source = ?2)
      AND (?3 IS NULL OR severity = ?3)
      AND (?4 IS NULL OR category = ?4)
      AND (?5 IS NULL OR (actor_type = 'device' AND actor_id = ?5) OR (resource_type = 'device' AND resource_id = ?5))
      AND (?6 IS NULL OR occurred_at >= ?6)
      AND (?7 IS NULL OR occurred_at <= ?7)
      AND (
          ?8 IS NULL
          OR lower(message) LIKE ?8
          OR lower(event_type) LIKE ?8
          OR lower(category) LIKE ?8
          OR lower(COALESCE(actor_id, '')) LIKE ?8
          OR lower(COALESCE(resource_id, '')) LIKE ?8
          OR lower(COALESCE(request_id, '')) LIKE ?8
      )
)
SELECT
    id,
    source,
    severity,
    event_type,
    category,
    message,
    actor_type,
    actor_id,
    resource_type,
    resource_id,
    request_id,
    metadata,
    occurred_at,
    count(*) OVER () AS total_count
FROM filtered
ORDER BY occurred_at DESC, id COLLATE BINARY DESC
LIMIT ?9 OFFSET ?10
"#;

#[async_trait]
impl ActivityRepository for TursoActivityRepository {
    async fn list(
        &self,
        tenant: &TenantId,
        query: ActivityQuery,
    ) -> Result<ActivityEventPage, PersistenceError> {
        let mut raw_connection = self
            .handles
            .connect_raw()
            .map_err(|error| PersistenceError::Unavailable(error.to_string()))?;
        let connection = raw_connection
            .transaction()
            .await
            .map_err(row::legacy_error)?;
        let search = query
            .search
            .map(|value| format!("%{}%", value.to_ascii_lowercase()));
        let mut rows = connection
            .query(
                ACTIVITY_QUERY,
                params![
                    tenant.as_str(),
                    query.source.clone(),
                    query.severity.clone(),
                    query.category.clone(),
                    query.device_id.clone(),
                    query.since.map(|value| value.and_utc().timestamp_micros()),
                    query.until.map(|value| value.and_utc().timestamp_micros()),
                    search.clone(),
                    query.limit,
                    query.offset
                ],
            )
            .await
            .map_err(row::legacy_error)?;

        let mut data = Vec::new();
        let mut total = 0;
        while let Some(record) = rows.next().await.map_err(row::legacy_error)? {
            total = record.get(13).map_err(row::legacy_error)?;
            let metadata: String = record.get(11).map_err(row::legacy_error)?;
            data.push(ActivityEventRecord {
                id: record.get(0).map_err(row::legacy_error)?,
                source: record.get(1).map_err(row::legacy_error)?,
                severity: record.get(2).map_err(row::legacy_error)?,
                event_type: record.get(3).map_err(row::legacy_error)?,
                category: record.get(4).map_err(row::legacy_error)?,
                message: record.get(5).map_err(row::legacy_error)?,
                actor_type: record.get(6).map_err(row::legacy_error)?,
                actor_id: record.get(7).map_err(row::legacy_error)?,
                resource_type: record.get(8).map_err(row::legacy_error)?,
                resource_id: record.get(9).map_err(row::legacy_error)?,
                request_id: record.get(10).map_err(row::legacy_error)?,
                metadata: serde_json::from_str(&metadata)
                    .map_err(|error| PersistenceError::Internal(error.to_string()))?,
                occurred_at: row::datetime(record.get(12).map_err(row::legacy_error)?)?.naive_utc(),
            });
        }
        drop(rows);
        if data.is_empty() && query.offset > 0 {
            let mut first_page = connection
                .query(
                    ACTIVITY_QUERY,
                    params![
                        tenant.as_str(),
                        query.source,
                        query.severity,
                        query.category,
                        query.device_id,
                        query.since.map(|value| value.and_utc().timestamp_micros()),
                        query.until.map(|value| value.and_utc().timestamp_micros()),
                        search,
                        1_i64,
                        0_i64
                    ],
                )
                .await
                .map_err(row::legacy_error)?;
            if let Some(record) = first_page.next().await.map_err(row::legacy_error)? {
                total = record.get(13).map_err(row::legacy_error)?;
            }
        }
        connection.commit().await.map_err(row::legacy_error)?;
        Ok(ActivityEventPage { data, total })
    }
}
