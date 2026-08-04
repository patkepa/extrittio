use chrono::{Duration, Utc};
use diesel::PgConnection;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Nullable, Text, Timestamptz};

use crate::db::models::{NewRuleActionOutboxEvent, RuleActionOutboxEvent};
use crate::db::schema::rule_action_outbox;

#[derive(Debug, QueryableByName)]
pub struct RuleActionOutboxSummary {
    #[diesel(sql_type = BigInt)]
    pub pending_count: i64,
    #[diesel(sql_type = BigInt)]
    pub processing_count: i64,
    #[diesel(sql_type = BigInt)]
    pub failed_count: i64,
    #[diesel(sql_type = BigInt)]
    pub dead_letter_count: i64,
    #[diesel(sql_type = BigInt)]
    pub succeeded_count: i64,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    pub oldest_pending_at: Option<chrono::NaiveDateTime>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub oldest_pending_age_seconds: Option<i64>,
}

pub fn insert_event(
    conn: &mut PgConnection,
    event: &NewRuleActionOutboxEvent,
) -> QueryResult<usize> {
    diesel::insert_into(rule_action_outbox::table)
        .values(event)
        .on_conflict_do_nothing()
        .execute(conn)
}

pub fn claim_batch(
    conn: &mut PgConnection,
    worker_id: &str,
    limit: i64,
    lease_timeout: std::time::Duration,
) -> QueryResult<Vec<RuleActionOutboxEvent>> {
    conn.transaction(|conn| {
        let now = Utc::now().naive_utc();
        let lease_seconds = i64::try_from(lease_timeout.as_secs()).unwrap_or(i64::MAX);
        let lease_cutoff = now - Duration::seconds(lease_seconds);

        // A worker may have crashed during its final allowed delivery attempt.
        // Move those expired leases to the dead-letter state instead of leaving
        // them permanently stuck as `processing`.
        diesel::update(
            rule_action_outbox::table
                .filter(rule_action_outbox::status.eq("processing"))
                .filter(rule_action_outbox::locked_at.le(Some(lease_cutoff)))
                .filter(rule_action_outbox::attempts.ge(rule_action_outbox::max_attempts)),
        )
        .set((
            rule_action_outbox::status.eq("dead_letter"),
            rule_action_outbox::locked_at.eq::<Option<chrono::NaiveDateTime>>(None),
            rule_action_outbox::locked_by.eq::<Option<String>>(None),
            rule_action_outbox::last_error.eq(Some(
                "delivery lease expired after final attempt".to_string(),
            )),
            rule_action_outbox::updated_at.eq(now),
        ))
        .execute(conn)?;

        // Rank work within each tenant, then order by rank across tenants. This
        // prevents a high-volume tenant from monopolizing every batch. Row
        // locks with SKIP LOCKED allow multiple workers to claim safely.
        diesel::sql_query(
            r#"
            WITH ranked AS (
                SELECT
                    id,
                    row_number() OVER (
                        PARTITION BY tenant_id
                        ORDER BY available_at, created_at
                    ) AS tenant_rank,
                    available_at,
                    created_at
                FROM rule_action_outbox
                WHERE attempts < max_attempts
                  AND (
                    (status IN ('pending', 'failed') AND available_at <= $1)
                    OR (status = 'processing' AND locked_at <= $2)
                  )
            ), candidates AS (
                SELECT outbox.id
                FROM rule_action_outbox AS outbox
                JOIN ranked ON ranked.id = outbox.id
                ORDER BY ranked.tenant_rank, ranked.available_at, ranked.created_at
                LIMIT $3
                FOR UPDATE OF outbox SKIP LOCKED
            )
            UPDATE rule_action_outbox AS outbox
            SET status = 'processing',
                attempts = outbox.attempts + 1,
                locked_at = $1,
                locked_by = $4,
                updated_at = $1
            FROM candidates
            WHERE outbox.id = candidates.id
            RETURNING outbox.*
            "#,
        )
        .bind::<Timestamptz, _>(now)
        .bind::<Timestamptz, _>(lease_cutoff)
        .bind::<BigInt, _>(limit)
        .bind::<Text, _>(worker_id)
        .load::<RuleActionOutboxEvent>(conn)
    })
}

pub fn mark_succeeded(
    conn: &mut PgConnection,
    event_id: &str,
    worker_id: &str,
) -> QueryResult<usize> {
    diesel::update(
        rule_action_outbox::table
            .filter(rule_action_outbox::id.eq(event_id))
            .filter(rule_action_outbox::status.eq("processing"))
            .filter(rule_action_outbox::locked_by.eq(worker_id)),
    )
    .set((
        rule_action_outbox::status.eq("succeeded"),
        rule_action_outbox::locked_at.eq::<Option<chrono::NaiveDateTime>>(None),
        rule_action_outbox::locked_by.eq::<Option<String>>(None),
        rule_action_outbox::last_error.eq::<Option<String>>(None),
        rule_action_outbox::updated_at.eq(Utc::now().naive_utc()),
    ))
    .execute(conn)
}

pub fn mark_failed(
    conn: &mut PgConnection,
    event_id: &str,
    worker_id: &str,
    attempts: i32,
    max_attempts: i32,
    error: &str,
) -> QueryResult<usize> {
    let terminal = attempts >= max_attempts;
    let status = if terminal { "dead_letter" } else { "failed" };
    let delay_seconds = retry_delay_seconds(attempts);

    diesel::update(
        rule_action_outbox::table
            .filter(rule_action_outbox::id.eq(event_id))
            .filter(rule_action_outbox::status.eq("processing"))
            .filter(rule_action_outbox::locked_by.eq(worker_id)),
    )
    .set((
        rule_action_outbox::status.eq(status),
        rule_action_outbox::available_at
            .eq(Utc::now().naive_utc() + Duration::seconds(delay_seconds)),
        rule_action_outbox::locked_at.eq::<Option<chrono::NaiveDateTime>>(None),
        rule_action_outbox::locked_by.eq::<Option<String>>(None),
        rule_action_outbox::last_error.eq(Some(error.chars().take(2000).collect::<String>())),
        rule_action_outbox::updated_at.eq(Utc::now().naive_utc()),
    ))
    .execute(conn)
}

fn retry_delay_seconds(attempts: i32) -> i64 {
    let exponent = attempts.clamp(1, 8) as u32;
    2_i64.pow(exponent).min(300)
}

pub fn summarize_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
) -> QueryResult<RuleActionOutboxSummary> {
    diesel::sql_query(
        r#"
        SELECT
            count(*) FILTER (WHERE status = 'pending')::bigint AS pending_count,
            count(*) FILTER (WHERE status = 'processing')::bigint AS processing_count,
            count(*) FILTER (WHERE status = 'failed')::bigint AS failed_count,
            count(*) FILTER (WHERE status = 'dead_letter')::bigint AS dead_letter_count,
            count(*) FILTER (WHERE status = 'succeeded')::bigint AS succeeded_count,
            min(created_at) FILTER (WHERE status IN ('pending', 'failed')) AS oldest_pending_at,
            extract(epoch FROM (
                now() - min(created_at) FILTER (WHERE status IN ('pending', 'failed'))
            ))::bigint AS oldest_pending_age_seconds
        FROM rule_action_outbox
        WHERE tenant_id = $1
        "#,
    )
    .bind::<Text, _>(tenant_id)
    .get_result(conn)
}

pub fn list_dead_letters(
    conn: &mut PgConnection,
    tenant_id: &str,
    limit: i64,
    offset: i64,
) -> QueryResult<Vec<RuleActionOutboxEvent>> {
    rule_action_outbox::table
        .filter(rule_action_outbox::tenant_id.eq(tenant_id))
        .filter(rule_action_outbox::status.eq("dead_letter"))
        .order(rule_action_outbox::updated_at.desc())
        .limit(limit)
        .offset(offset)
        .select(RuleActionOutboxEvent::as_select())
        .load(conn)
}

pub fn replay_dead_letters(
    conn: &mut PgConnection,
    tenant_id: &str,
    event_ids: Option<&[String]>,
) -> QueryResult<usize> {
    let now = Utc::now().naive_utc();
    let changes = (
        rule_action_outbox::status.eq("pending"),
        rule_action_outbox::attempts.eq(0),
        rule_action_outbox::available_at.eq(now),
        rule_action_outbox::locked_at.eq::<Option<chrono::NaiveDateTime>>(None),
        rule_action_outbox::locked_by.eq::<Option<String>>(None),
        rule_action_outbox::last_error.eq::<Option<String>>(None),
        rule_action_outbox::updated_at.eq(now),
    );

    if let Some(event_ids) = event_ids {
        diesel::update(
            rule_action_outbox::table
                .filter(rule_action_outbox::tenant_id.eq(tenant_id))
                .filter(rule_action_outbox::status.eq("dead_letter"))
                .filter(rule_action_outbox::id.eq_any(event_ids)),
        )
        .set(changes)
        .execute(conn)
    } else {
        diesel::update(
            rule_action_outbox::table
                .filter(rule_action_outbox::tenant_id.eq(tenant_id))
                .filter(rule_action_outbox::status.eq("dead_letter")),
        )
        .set(changes)
        .execute(conn)
    }
}
