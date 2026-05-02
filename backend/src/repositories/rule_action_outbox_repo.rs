use chrono::{Duration, Utc};
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{NewRuleActionOutboxEvent, RuleActionOutboxEvent};
use crate::db::schema::rule_action_outbox;

pub fn insert_event(
    conn: &mut PgConnection,
    event: &NewRuleActionOutboxEvent,
) -> QueryResult<usize> {
    diesel::insert_into(rule_action_outbox::table)
        .values(event)
        .execute(conn)
}

pub fn claim_batch(
    conn: &mut PgConnection,
    worker_id: &str,
    limit: i64,
) -> QueryResult<Vec<RuleActionOutboxEvent>> {
    conn.transaction(|conn| {
        let ids = rule_action_outbox::table
            .filter(
                rule_action_outbox::status
                    .eq("pending")
                    .or(rule_action_outbox::status.eq("failed")),
            )
            .filter(rule_action_outbox::available_at.le(Utc::now().naive_utc()))
            .filter(rule_action_outbox::attempts.lt(rule_action_outbox::max_attempts))
            .order((
                rule_action_outbox::available_at.asc(),
                rule_action_outbox::created_at.asc(),
            ))
            .limit(limit)
            .for_update()
            .skip_locked()
            .select(rule_action_outbox::id)
            .load::<String>(conn)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        diesel::update(rule_action_outbox::table.filter(rule_action_outbox::id.eq_any(&ids)))
            .set((
                rule_action_outbox::status.eq("processing"),
                rule_action_outbox::attempts.eq(rule_action_outbox::attempts + 1),
                rule_action_outbox::locked_at.eq(Some(Utc::now().naive_utc())),
                rule_action_outbox::locked_by.eq(Some(worker_id.to_string())),
                rule_action_outbox::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(conn)?;

        rule_action_outbox::table
            .filter(rule_action_outbox::id.eq_any(ids))
            .order(rule_action_outbox::created_at.asc())
            .select(RuleActionOutboxEvent::as_select())
            .load(conn)
    })
}

pub fn mark_succeeded(conn: &mut PgConnection, event_id: &str) -> QueryResult<usize> {
    diesel::update(rule_action_outbox::table.filter(rule_action_outbox::id.eq(event_id)))
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
    attempts: i32,
    max_attempts: i32,
    error: &str,
) -> QueryResult<usize> {
    let terminal = attempts >= max_attempts;
    let status = if terminal { "dead_letter" } else { "failed" };
    let delay_seconds = retry_delay_seconds(attempts);

    diesel::update(rule_action_outbox::table.filter(rule_action_outbox::id.eq(event_id)))
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
