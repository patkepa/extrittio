// Repository functions for alerts

use chrono::NaiveDateTime;
use diesel::PgConnection;
use diesel::prelude::*;

use crate::db::models::{Alert, NewAlert, UpdateAlert};
use crate::db::schema::alerts;

// ---------------------------------------------------------------------------
// List with filtering + count for pagination
// ---------------------------------------------------------------------------

pub fn list_alerts(
    conn: &mut PgConnection,
    status: Option<&str>,
    severity: Option<&str>,
    device_id: Option<&str>,
    rule_id: Option<&str>,
    since: Option<NaiveDateTime>,
    before: Option<NaiveDateTime>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Alert>, i64), diesel::result::Error> {
    // Build a shared filter closure so the logic is applied consistently to
    // both the count query and the data query.
    macro_rules! apply_filters {
        ($q:expr) => {{
            let mut q = $q;
            if let Some(s) = status {
                q = q.filter(alerts::status.eq(s));
            }
            if let Some(sev) = severity {
                q = q.filter(alerts::severity.eq(sev));
            }
            if let Some(did) = device_id {
                q = q.filter(alerts::device_id.eq(did));
            }
            if let Some(rid) = rule_id {
                q = q.filter(alerts::rule_id.eq(rid));
            }
            if let Some(since_dt) = since {
                q = q.filter(alerts::created_at.gt(since_dt));
            }
            if let Some(before_dt) = before {
                q = q.filter(alerts::created_at.lt(before_dt));
            }
            q
        }};
    }

    let total: i64 = apply_filters!(alerts::table.into_boxed())
        .count()
        .get_result(conn)?;

    let data: Vec<Alert> = apply_filters!(alerts::table.into_boxed())
        .order(alerts::created_at.desc())
        .limit(limit)
        .offset(offset)
        .select(Alert::as_select())
        .load(conn)?;

    Ok((data, total))
}

// ---------------------------------------------------------------------------
// Single record lookup
// ---------------------------------------------------------------------------

pub fn find_alert(
    conn: &mut PgConnection,
    id: &str,
) -> Result<Alert, diesel::result::Error> {
    alerts::table
        .find(id)
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
    id: &str,
    changeset: &UpdateAlert,
) -> Result<usize, diesel::result::Error> {
    diesel::update(alerts::table.find(id))
        .set(changeset)
        .execute(conn)
}

// ---------------------------------------------------------------------------
// Summary / analytics queries
// ---------------------------------------------------------------------------

/// Returns `(status, severity, count)` rows — useful for the dashboard summary
/// endpoint. Relies on SQLite's GROUP BY support.
pub fn count_by_status_and_severity(
    conn: &mut PgConnection,
) -> Result<Vec<(String, String, i64)>, diesel::result::Error> {
    alerts::table
        .group_by((alerts::status, alerts::severity))
        .select((alerts::status, alerts::severity, diesel::dsl::count_star()))
        .load::<(String, String, i64)>(conn)
}

// ---------------------------------------------------------------------------
// Convenience loaders
// ---------------------------------------------------------------------------

/// Load all alerts whose status is "active" or "acknowledged".
/// Both states represent alerts the rule engine should track to avoid creating
/// duplicate alerts for the same rule+device pair.
pub fn load_active_alerts(
    conn: &mut PgConnection,
) -> Result<Vec<Alert>, diesel::result::Error> {
    alerts::table
        .filter(alerts::status.eq("active").or(alerts::status.eq("acknowledged")))
        .select(Alert::as_select())
        .load(conn)
}

// ---------------------------------------------------------------------------
// Retention / cleanup
// ---------------------------------------------------------------------------

/// Delete resolved alerts whose `created_at` is older than `cutoff`.
pub fn delete_resolved_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(
        alerts::table
            .filter(alerts::status.eq("resolved"))
            .filter(alerts::created_at.lt(cutoff)),
    )
    .execute(conn)
}
