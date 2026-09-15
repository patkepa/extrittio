// Repository functions for alerts

use chrono::NaiveDateTime;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::models::{Alert, NewAlert, UpdateAlert};
use crate::schema::alerts;

#[derive(Debug, Clone, Copy)]
pub struct AlertListFilter<'a> {
    pub status: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub device_id: Option<&'a str>,
    pub rule_id: Option<&'a str>,
    pub since: Option<NaiveDateTime>,
    pub before: Option<NaiveDateTime>,
    pub limit: i64,
    pub offset: i64,
}

// ---------------------------------------------------------------------------
// List with filtering + count for pagination
// ---------------------------------------------------------------------------

pub fn list_alerts(
    conn: &mut PgConnection,
    tenant_id: &str,
    filter: AlertListFilter<'_>,
) -> Result<(Vec<Alert>, i64), diesel::result::Error> {
    // Build a shared filter closure so the logic is applied consistently to
    // both the count query and the data query.
    macro_rules! apply_filters {
        ($q:expr) => {{
            let mut q = $q;
            q = q.filter(alerts::tenant_id.eq(tenant_id));
            if let Some(s) = filter.status {
                q = q.filter(alerts::status.eq(s));
            }
            if let Some(sev) = filter.severity {
                q = q.filter(alerts::severity.eq(sev));
            }
            if let Some(did) = filter.device_id {
                q = q.filter(alerts::device_id.eq(did));
            }
            if let Some(rid) = filter.rule_id {
                q = q.filter(alerts::rule_id.eq(rid));
            }
            if let Some(since_dt) = filter.since {
                q = q.filter(alerts::created_at.ge(since_dt));
            }
            if let Some(before_dt) = filter.before {
                q = q.filter(alerts::created_at.lt(before_dt));
            }
            q
        }};
    }

    let total: i64 = apply_filters!(alerts::table.into_boxed())
        .count()
        .get_result(conn)?;

    let data: Vec<Alert> = apply_filters!(alerts::table.into_boxed())
        .order((alerts::created_at.desc(), alerts::id.desc()))
        .limit(filter.limit)
        .offset(filter.offset)
        .select(Alert::as_select())
        .load(conn)?;

    Ok((data, total))
}

// ---------------------------------------------------------------------------
// Single record lookup
// ---------------------------------------------------------------------------

pub fn find_alert(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<Alert, diesel::result::Error> {
    alerts::table
        .filter(alerts::tenant_id.eq(tenant_id))
        .filter(alerts::id.eq(id))
        .select(Alert::as_select())
        .first(conn)
}

// ---------------------------------------------------------------------------
// Insert / update
// ---------------------------------------------------------------------------

pub fn insert_alert(
    conn: &mut PgConnection,
    new_alert: &NewAlert,
) -> Result<(), diesel::result::Error> {
    diesel::insert_into(alerts::table)
        .values(new_alert)
        .execute(conn)?;
    Ok(())
}

pub fn update_alert(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
    changeset: &UpdateAlert,
) -> Result<usize, diesel::result::Error> {
    diesel::update(
        alerts::table
            .filter(alerts::tenant_id.eq(tenant_id))
            .filter(alerts::id.eq(id)),
    )
    .set(changeset)
    .execute(conn)
}

// ---------------------------------------------------------------------------
// Summary / analytics queries
// ---------------------------------------------------------------------------

/// Returns `(status, severity, count)` rows — useful for the dashboard summary
/// endpoint, ordered by status and severity.
pub fn count_by_status_and_severity(
    conn: &mut PgConnection,
    tenant_id: &str,
) -> Result<Vec<(String, String, i64)>, diesel::result::Error> {
    alerts::table
        .filter(alerts::tenant_id.eq(tenant_id))
        .group_by((alerts::status, alerts::severity))
        .order((alerts::status.asc(), alerts::severity.asc()))
        .select((alerts::status, alerts::severity, diesel::dsl::count_star()))
        .load::<(String, String, i64)>(conn)
}

use crate::models::RuleCooldown;

pub fn upsert_cooldown(
    conn: &mut PgConnection,
    cooldown: &RuleCooldown,
) -> Result<(), diesel::result::Error> {
    // Caller holds the device serialization lock; timestamps never move backward.
    diesel::sql_query("INSERT INTO rule_cooldowns(tenant_id,rule_id,device_id,last_fired_at)
        VALUES ($1,$2,$3,$4) ON CONFLICT(rule_id,device_id) DO UPDATE SET last_fired_at=GREATEST(rule_cooldowns.last_fired_at,EXCLUDED.last_fired_at)")
        .bind::<diesel::sql_types::Text,_>(&cooldown.tenant_id)
        .bind::<diesel::sql_types::Text,_>(&cooldown.rule_id)
        .bind::<diesel::sql_types::Text,_>(&cooldown.device_id)
        .bind::<diesel::sql_types::Timestamptz,_>(cooldown.last_fired_at)
        .execute(conn)?;
    Ok(())
}
