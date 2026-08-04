use diesel::PgConnection;
use std::collections::HashMap;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::db::models::{Fleet, NewFleet};
use crate::error::AppError;
use crate::repositories::fleet_repo;

pub struct FleetWithCount {
    pub fleet: Fleet,
    pub device_count: i64,
}

pub fn list(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FleetWithCount>, i64), AppError> {
    policy::require(ctx, Permission::ReadFleets)?;

    let (fleets, counts, total) =
        fleet_repo::list_fleets(conn, ctx.tenant_id_str(), limit, offset)?;

    let count_map: HashMap<i32, i64> = counts
        .into_iter()
        .filter_map(|(fleet_id, count)| fleet_id.map(|fid| (fid, count)))
        .collect();

    let enriched = fleets
        .into_iter()
        .map(|f| {
            let device_count = count_map.get(&f.id).copied().unwrap_or(0);
            FleetWithCount {
                fleet: f,
                device_count,
            }
        })
        .collect();

    Ok((enriched, total))
}

pub fn create(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    name: &str,
) -> Result<Fleet, AppError> {
    policy::require(ctx, Permission::ManageFleets)?;

    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }
    Ok(fleet_repo::insert_fleet(
        conn,
        ctx.tenant_id_str(),
        &NewFleet {
            tenant_id: ctx.tenant_id_str().to_string(),
            name: name.to_string(),
        },
    )?)
}

pub fn rename(
    ctx: &RequestContext,
    conn: &mut PgConnection,
    id: i32,
    new_name: &str,
) -> Result<Fleet, AppError> {
    policy::require(ctx, Permission::ManageFleets)?;

    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }
    fleet_repo::update_fleet_name(conn, ctx.tenant_id_str(), id, trimmed)?
        .ok_or_else(|| AppError::NotFound(format!("Fleet {id} not found")))
}

pub fn delete(ctx: &RequestContext, conn: &mut PgConnection, id: i32) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageFleets)?;

    let deleted = fleet_repo::delete_fleet(conn, ctx.tenant_id_str(), id)?;
    if !deleted {
        return Err(AppError::NotFound(format!("Fleet {id} not found")));
    }
    Ok(())
}
