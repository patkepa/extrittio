// Zone service — business logic for zone management

use diesel::PgConnection;
use diesel::prelude::*;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{NewZone, Zone};
use crate::error::AppError;
use crate::repositories::zone_repo;

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
    name: Option<String>,
    description: Option<String>,
    geometry_type: Option<String>,
    geometry_json: Option<JsonValue>,
    color: Option<String>,
) -> Result<Zone, AppError> {
    policy::require(ctx, Permission::ManageZones)?;

    // Verify zone exists first
    get_zone_for_tenant(conn, ctx.tenant_id_str(), zone_id)?;

    Ok(zone_repo::update_zone(
        conn,
        ctx.tenant_id_str(),
        zone_id,
        name,
        description,
        geometry_type,
        geometry_json,
        color,
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
