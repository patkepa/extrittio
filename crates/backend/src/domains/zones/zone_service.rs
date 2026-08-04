// Zone service — business logic for zone management

use diesel::PgConnection;
use diesel::prelude::*;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{NewZone, UpdateZone, Zone};
use crate::error::AppError;
use crate::repositories::zone_repo;

#[derive(Debug)]
pub struct ZoneUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<JsonValue>,
    pub color: Option<String>,
}

pub fn list_zones(ctx: &RequestContext, conn: &mut PgConnection) -> Result<Vec<Zone>, AppError> {
    policy::require(ctx, Permission::ReadZones)?;
    Ok(zone_repo::list_zones(conn, ctx.tenant_id_str())?)
}

pub fn get_zone(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    zone_id: &str,
) -> Result<Zone, AppError> {
    policy::require(ctx, Permission::ReadZones)?;
    get_zone_for_tenant(conn, ctx.tenant_id_str(), zone_id)
}

fn get_zone_for_tenant(
    conn: &mut PgConnection,
    tenant_id: &str,
    zone_id: &str,
) -> Result<Zone, AppError> {
    zone_repo::get_zone(conn, tenant_id, zone_id).map_err(|e| match e {
        diesel::result::Error::NotFound => {
            AppError::NotFound(format!("Zone '{zone_id}' not found"))
        }
        other => AppError::Database(other),
    })
}

pub fn create_zone(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    name: String,
    description: String,
    geometry_type: String,
    geometry_json: JsonValue,
    color: String,
) -> Result<Zone, AppError> {
    policy::require(ctx, Permission::ManageZones)?;
    validate_geometry(&geometry_type, &geometry_json)?;

    let zone_id = Uuid::new_v4().to_string();

    let new_zone = NewZone {
        id: zone_id,
        tenant_id: ctx.tenant_id_str().to_string(),
        name,
        description,
        geometry_type,
        geometry_json,
        color,
    };

    Ok(zone_repo::insert_zone(
        conn,
        ctx.tenant_id_str(),
        &new_zone,
    )?)
}

pub fn update_zone(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    zone_id: &str,
    update: ZoneUpdate,
) -> Result<Zone, AppError> {
    policy::require(ctx, Permission::ManageZones)?;

    // Verify zone exists first
    let existing = get_zone_for_tenant(conn, ctx.tenant_id_str(), zone_id)?;
    let effective_geometry_type = update
        .geometry_type
        .as_deref()
        .unwrap_or(existing.geometry_type.as_str());
    let effective_geometry_json = update
        .geometry_json
        .as_ref()
        .unwrap_or(&existing.geometry_json);
    validate_geometry(effective_geometry_type, effective_geometry_json)?;

    let changeset = UpdateZone {
        name: update.name,
        description: update.description,
        geometry_type: update.geometry_type,
        geometry_json: update.geometry_json,
        color: update.color,
        updated_at: Some(chrono::Utc::now().naive_utc()),
    };

    Ok(zone_repo::update_zone(
        conn,
        ctx.tenant_id_str(),
        zone_id,
        &changeset,
    )?)
}

pub fn delete_zone(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    zone_id: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageZones)?;

    // Verify zone exists first
    get_zone_for_tenant(conn, ctx.tenant_id_str(), zone_id)?;

    // Check if any rule_conditions reference this zone
    use crate::db::schema::rule_conditions;
    let count: i64 = rule_conditions::table
        .filter(rule_conditions::tenant_id.eq(ctx.tenant_id_str()))
        .filter(rule_conditions::zone_id.eq(zone_id))
        .count()
        .get_result(conn)?;
    if count > 0 {
        return Err(AppError::Conflict(
            "Cannot delete zone: referenced by rules".into(),
        ));
    }

    let rows = zone_repo::delete_zone(conn, ctx.tenant_id_str(), zone_id)?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("Zone '{zone_id}' not found")));
    }

    Ok(())
}

fn validate_geometry(geometry_type: &str, geometry_json: &JsonValue) -> Result<(), AppError> {
    match geometry_type {
        "circle" => {
            let center = geometry_json
                .get("center")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| AppError::BadRequest("Circle geometry requires center".into()))?;
            if center.len() != 2
                || !center[0].as_f64().is_some_and(f64::is_finite)
                || !center[1].as_f64().is_some_and(f64::is_finite)
            {
                return Err(AppError::BadRequest(
                    "Circle center must be [latitude, longitude]".into(),
                ));
            }
            let radius = geometry_json
                .get("radius_meters")
                .and_then(JsonValue::as_f64)
                .ok_or_else(|| {
                    AppError::BadRequest("Circle geometry requires radius_meters".into())
                })?;
            if !radius.is_finite() || radius <= 0.0 {
                return Err(AppError::BadRequest(
                    "Circle radius_meters must be a positive number".into(),
                ));
            }
            Ok(())
        }
        "polygon" => {
            let points = geometry_json
                .get("points")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| AppError::BadRequest("Polygon geometry requires points".into()))?;
            if points.len() < 3 {
                return Err(AppError::BadRequest(
                    "Polygon geometry requires at least three points".into(),
                ));
            }
            for point in points {
                let Some(coords) = point.as_array() else {
                    return Err(AppError::BadRequest(
                        "Polygon points must be [latitude, longitude] arrays".into(),
                    ));
                };
                if coords.len() != 2
                    || !coords[0].as_f64().is_some_and(f64::is_finite)
                    || !coords[1].as_f64().is_some_and(f64::is_finite)
                {
                    return Err(AppError::BadRequest(
                        "Polygon points must contain finite latitude and longitude values".into(),
                    ));
                }
            }
            Ok(())
        }
        _ => Err(AppError::BadRequest(format!(
            "Unsupported zone geometry type '{geometry_type}'"
        ))),
    }
}
