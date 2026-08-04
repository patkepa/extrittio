// Alert service — business logic for alert management

use chrono::{NaiveDateTime, Utc};
use diesel::PgConnection;
use uuid::Uuid;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::RuleCooldown;
use crate::db::models::{Alert, NewAlert, UpdateAlert};
use crate::error::AppError;
use crate::repositories::alert_repo;
use crate::repositories::rule_repo;
use crate::tenancy::DEFAULT_TENANT_ID;

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

pub fn list_alerts(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    filter: alert_repo::AlertListFilter<'_>,
) -> Result<(Vec<Alert>, i64), AppError> {
    policy::require(ctx, Permission::ReadAlerts)?;

    Ok(alert_repo::list_alerts(conn, ctx.tenant_id_str(), filter)?)
}

pub fn get_alert(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: &str,
) -> Result<Alert, AppError> {
    policy::require(ctx, Permission::ReadAlerts)?;
    get_alert_for_tenant(conn, ctx.tenant_id_str(), id)
}

pub fn get_alert_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<Alert, AppError> {
    alert_repo::find_alert(conn, tenant_id, id).map_err(|e| match e {
        diesel::result::Error::NotFound => AppError::NotFound(format!("Alert '{id}' not found")),
        other => AppError::Database(other),
    })
}

pub fn create_alert(
    conn: &mut PgConnection,
    rule_id: Option<String>,
    device_id: String,
    severity: String,
    message: String,
    triggered_value: Option<String>,
) -> Result<Alert, AppError> {
    create_alert_for_tenant(
        conn,
        DEFAULT_TENANT_ID,
        rule_id,
        device_id,
        severity,
        message,
        triggered_value,
    )
}

pub fn create_alert_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    rule_id: Option<String>,
    device_id: String,
    severity: String,
    message: String,
    triggered_value: Option<String>,
) -> Result<Alert, AppError> {
    let id = Uuid::new_v4().to_string();
    let new_alert = NewAlert {
        id: id.clone(),
        tenant_id: tenant_id.to_string(),
        rule_id,
        device_id,
        severity,
        message,
        triggered_value,
    };
    alert_repo::insert_alert(conn, &new_alert)?;
    get_alert_for_tenant(conn, tenant_id, &id)
}

// ---------------------------------------------------------------------------
// State transitions
// ---------------------------------------------------------------------------

/// Transition an alert from `active` → `acknowledged`.
/// Returns `BadRequest` if the alert is not currently active.
pub fn acknowledge_alert(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: &str,
) -> Result<Alert, AppError> {
    policy::require(ctx, Permission::ManageAlerts)?;

    let alert = get_alert_for_tenant(conn, ctx.tenant_id_str(), id)?;
    if alert.status != "active" {
        return Err(AppError::BadRequest(format!(
            "Alert '{id}' cannot be acknowledged from status '{}'",
            alert.status
        )));
    }
    let now = Utc::now().naive_utc();
    let changeset = UpdateAlert {
        status: Some("acknowledged".to_string()),
        acknowledged_at: Some(Some(now)),
        resolved_at: None,
    };
    alert_repo::update_alert(conn, ctx.tenant_id_str(), id, &changeset)?;
    get_alert_for_tenant(conn, ctx.tenant_id_str(), id)
}

/// Transition an alert from `active` or `acknowledged` → `resolved`.
/// Returns `BadRequest` if the alert is already resolved.
pub fn resolve_alert(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: &str,
) -> Result<Alert, AppError> {
    policy::require(ctx, Permission::ManageAlerts)?;
    resolve_alert_for_tenant(conn, ctx.tenant_id_str(), id)
}

pub fn resolve_alert_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    id: &str,
) -> Result<Alert, AppError> {
    let alert = get_alert_for_tenant(conn, tenant_id, id)?;
    if alert.status == "resolved" {
        return Err(AppError::BadRequest(format!(
            "Alert '{id}' is already resolved"
        )));
    }
    let now = Utc::now().naive_utc();
    let changeset = UpdateAlert {
        status: Some("resolved".to_string()),
        resolved_at: Some(Some(now)),
        acknowledged_at: None,
    };
    alert_repo::update_alert(conn, tenant_id, id, &changeset)?;
    get_alert_for_tenant(conn, tenant_id, id)
}

/// Transition an alert from `acknowledged` or `resolved` → `active`.
/// Returns `BadRequest` if the alert is already active.
pub fn reactivate_alert(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: &str,
) -> Result<Alert, AppError> {
    policy::require(ctx, Permission::ManageAlerts)?;

    let alert = get_alert_for_tenant(conn, ctx.tenant_id_str(), id)?;
    if alert.status == "active" {
        return Err(AppError::BadRequest(format!(
            "Alert '{id}' is already active"
        )));
    }
    let changeset = UpdateAlert {
        status: Some("active".to_string()),
        acknowledged_at: Some(None),
        resolved_at: Some(None),
    };
    alert_repo::update_alert(conn, ctx.tenant_id_str(), id, &changeset)?;
    get_alert_for_tenant(conn, ctx.tenant_id_str(), id)
}

/// Update the `triggered_value` field of an existing alert (used by the rule
/// engine when the same condition fires again with a new sensor reading).
pub fn update_triggered_value(
    conn: &mut PgConnection,
    id: &str,
    value: String,
) -> Result<(), AppError> {
    // UpdateAlert doesn't have a triggered_value field — we use a raw Diesel
    // update targeting only that column.
    use crate::db::schema::alerts;
    use diesel::prelude::*;

    let rows = diesel::update(alerts::table.find(id))
        .set(alerts::triggered_value.eq(Some(value)))
        .execute(conn)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Alert '{id}' not found")));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Analytics
// ---------------------------------------------------------------------------

pub fn summary(
    ctx: &RequestContext,
    conn: &mut PgConnection,
) -> Result<Vec<(String, String, i64)>, AppError> {
    policy::require(ctx, Permission::ReadAlerts)?;
    Ok(alert_repo::count_by_status_and_severity(
        conn,
        ctx.tenant_id_str(),
    )?)
}

// ---------------------------------------------------------------------------
// Retention
// ---------------------------------------------------------------------------

pub fn delete_resolved_older_than(
    conn: &mut PgConnection,
    cutoff: NaiveDateTime,
) -> Result<usize, AppError> {
    Ok(alert_repo::delete_resolved_older_than(
        conn,
        DEFAULT_TENANT_ID,
        cutoff,
    )?)
}

/// Persist a cooldown entry to the database (fire-and-forget safe).
pub fn persist_cooldown(
    conn: &mut PgConnection,
    rule_id: &str,
    device_id: &str,
    last_fired_at: NaiveDateTime,
) -> Result<(), AppError> {
    persist_cooldown_for_tenant(conn, DEFAULT_TENANT_ID, rule_id, device_id, last_fired_at)
}

pub fn persist_cooldown_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    rule_id: &str,
    device_id: &str,
    last_fired_at: NaiveDateTime,
) -> Result<(), AppError> {
    rule_repo::upsert_cooldown(
        conn,
        &RuleCooldown {
            tenant_id: tenant_id.to_string(),
            rule_id: rule_id.to_string(),
            device_id: device_id.to_string(),
            last_fired_at,
        },
    )?;
    Ok(())
}
