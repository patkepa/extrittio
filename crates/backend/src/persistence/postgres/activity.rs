use async_trait::async_trait;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Jsonb, Nullable, Text, Timestamptz};
use serde_json::Value;

use crate::domains::activity::repository::ActivityRepository;
use crate::domains::activity::types::{ActivityEventPage, ActivityEventRecord, ActivityQuery};
use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::PostgresAdapter;
use super::executor::map_diesel_error;

const ACTIVITY_QUERY: &str = r#"
WITH activity AS (
    SELECT
        'device:' || logs.id::text AS id,
        'device'::text AS source,
        CASE upper(logs.level)
            WHEN 'DEBUG' THEN 'debug'
            WHEN 'WARN' THEN 'warning'
            WHEN 'ERROR' THEN 'error'
            ELSE 'info'
        END::text AS severity,
        'device.log'::text AS event_type,
        'device'::text AS category,
        logs.message,
        'device'::text AS actor_type,
        logs.device_id AS actor_id,
        'device'::text AS resource_type,
        logs.device_id AS resource_id,
        NULL::text AS request_id,
        jsonb_build_object('level', logs.level) AS metadata,
        logs.created_at AS occurred_at
    FROM device_logs logs
    WHERE logs.tenant_id = $1

    UNION ALL

    SELECT
        'audit:' || audit.id AS id,
        'audit'::text AS source,
        CASE WHEN audit.outcome = 'success' THEN 'info' ELSE 'error' END::text AS severity,
        audit.action AS event_type,
        audit.resource_type AS category,
        concat(
            COALESCE(NULLIF(audit.metadata->>'method', ''), upper(split_part(audit.action, '.', 2))),
            ' ',
            COALESCE(NULLIF(audit.metadata->>'path', ''), audit.resource_type)
        ) AS message,
        audit.actor_type,
        audit.actor_id,
        audit.resource_type,
        audit.resource_id,
        audit.request_id,
        audit.metadata,
        audit.occurred_at
    FROM audit_events audit
    WHERE audit.tenant_id = $1

    UNION ALL

    SELECT
        'alert:' || alert.id AS id,
        'alert'::text AS source,
        CASE lower(alert.severity)
            WHEN 'critical' THEN 'error'
            WHEN 'warning' THEN 'warning'
            ELSE 'info'
        END::text AS severity,
        'alert.' || lower(alert.status) AS event_type,
        'alerts'::text AS category,
        alert.message,
        'rule'::text AS actor_type,
        alert.rule_id AS actor_id,
        'device'::text AS resource_type,
        alert.device_id AS resource_id,
        NULL::text AS request_id,
        jsonb_build_object(
            'rule_id', alert.rule_id,
            'status', alert.status,
            'triggered_value', alert.triggered_value
        ) AS metadata,
        COALESCE(alert.resolved_at, alert.acknowledged_at, alert.created_at) AS occurred_at
    FROM alerts alert
    WHERE alert.tenant_id = $1

    UNION ALL

    SELECT
        'command:' || command.id AS id,
        'command'::text AS source,
        CASE lower(command.status)
            WHEN 'failed' THEN 'error'
            WHEN 'timed_out' THEN 'error'
            WHEN 'timeout' THEN 'error'
            WHEN 'pending' THEN 'debug'
            ELSE 'info'
        END::text AS severity,
        'command.' || lower(command.status) AS event_type,
        'commands'::text AS category,
        concat('Command ', command.command, ' ', replace(command.status, '_', ' ')) AS message,
        'system'::text AS actor_type,
        NULL::text AS actor_id,
        'device'::text AS resource_type,
        command.device_id AS resource_id,
        NULL::text AS request_id,
        jsonb_build_object(
            'command', command.command,
            'params', command.params,
            'status', command.status,
            'response_payload', command.response_payload
        ) AS metadata,
        command.updated_at AS occurred_at
    FROM command_history command
    WHERE command.tenant_id = $1

    UNION ALL

    SELECT
        'deployment:' || deployment.id::text AS id,
        'deployment'::text AS source,
        CASE lower(deployment.status)
            WHEN 'failed' THEN 'error'
            WHEN 'pending' THEN 'debug'
            ELSE 'info'
        END::text AS severity,
        'ota.' || lower(deployment.status) AS event_type,
        'firmware'::text AS category,
        concat('Firmware deployment ', replace(deployment.status, '_', ' ')) AS message,
        'system'::text AS actor_type,
        NULL::text AS actor_id,
        'device'::text AS resource_type,
        deployment.device_id AS resource_id,
        NULL::text AS request_id,
        jsonb_build_object(
            'firmware_update_id', deployment.firmware_update_id,
            'status', deployment.status,
            'error_message', deployment.error_message
        ) AS metadata,
        COALESCE(deployment.completed_at, deployment.initiated_at) AS occurred_at
    FROM ota_deployments deployment
    WHERE deployment.tenant_id = $1
), filtered AS (
    SELECT *
    FROM activity
    WHERE ($2 IS NULL OR source = $2)
      AND ($3 IS NULL OR severity = $3)
      AND ($4 IS NULL OR category = $4)
      AND ($5 IS NULL OR (actor_type = 'device' AND actor_id = $5) OR (resource_type = 'device' AND resource_id = $5))
      AND ($6 IS NULL OR occurred_at >= $6)
      AND ($7 IS NULL OR occurred_at <= $7)
      AND (
          $8 IS NULL
          OR message ILIKE $8
          OR event_type ILIKE $8
          OR category ILIKE $8
          OR COALESCE(actor_id, '') ILIKE $8
          OR COALESCE(resource_id, '') ILIKE $8
          OR COALESCE(request_id, '') ILIKE $8
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
    count(*) OVER ()::bigint AS total_count
FROM filtered
ORDER BY occurred_at DESC, id DESC
LIMIT $9 OFFSET $10
"#;

#[derive(QueryableByName)]
struct ActivityRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = Text)]
    severity: String,
    #[diesel(sql_type = Text)]
    event_type: String,
    #[diesel(sql_type = Text)]
    category: String,
    #[diesel(sql_type = Text)]
    message: String,
    #[diesel(sql_type = Text)]
    actor_type: String,
    #[diesel(sql_type = Nullable<Text>)]
    actor_id: Option<String>,
    #[diesel(sql_type = Text)]
    resource_type: String,
    #[diesel(sql_type = Nullable<Text>)]
    resource_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    request_id: Option<String>,
    #[diesel(sql_type = Jsonb)]
    metadata: Value,
    #[diesel(sql_type = Timestamptz)]
    occurred_at: NaiveDateTime,
    #[diesel(sql_type = BigInt)]
    total_count: i64,
}

#[async_trait]
impl ActivityRepository for PostgresAdapter {
    async fn list(
        &self,
        tenant: &TenantId,
        query: ActivityQuery,
    ) -> Result<ActivityEventPage, PersistenceError> {
        let tenant_id = tenant.as_str().to_string();
        let search = query.search.map(|value| format!("%{value}%"));
        self.executor
            .run(move |connection| {
                let rows = diesel::sql_query(ACTIVITY_QUERY)
                    .bind::<Text, _>(tenant_id)
                    .bind::<Nullable<Text>, _>(query.source)
                    .bind::<Nullable<Text>, _>(query.severity)
                    .bind::<Nullable<Text>, _>(query.category)
                    .bind::<Nullable<Text>, _>(query.device_id)
                    .bind::<Nullable<Timestamptz>, _>(query.since)
                    .bind::<Nullable<Timestamptz>, _>(query.until)
                    .bind::<Nullable<Text>, _>(search)
                    .bind::<BigInt, _>(query.limit)
                    .bind::<BigInt, _>(query.offset)
                    .load::<ActivityRow>(connection)
                    .map_err(map_diesel_error)?;
                let total = rows.first().map_or(0, |row| row.total_count);
                let data = rows
                    .into_iter()
                    .map(|row| ActivityEventRecord {
                        id: row.id,
                        source: row.source,
                        severity: row.severity,
                        event_type: row.event_type,
                        category: row.category,
                        message: row.message,
                        actor_type: row.actor_type,
                        actor_id: row.actor_id,
                        resource_type: row.resource_type,
                        resource_id: row.resource_id,
                        request_id: row.request_id,
                        metadata: row.metadata,
                        occurred_at: row.occurred_at,
                    })
                    .collect();
                Ok(ActivityEventPage { data, total })
            })
            .await
    }
}
