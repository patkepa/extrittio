use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::zones::port::ZoneRepository;
use crate::domains::zones::types::{
    DeleteZoneOutcome, NewZoneRecord, UpdateZoneRecord, ZoneRecord,
};
use crate::error::AppError;

#[derive(Debug)]
pub struct ZoneUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<JsonValue>,
    pub color: Option<String>,
}

pub async fn list_zones(
    ctx: &RequestContext,
    repository: &dyn ZoneRepository,
) -> Result<Vec<ZoneRecord>, AppError> {
    policy::require(ctx, Permission::ReadZones)?;
    Ok(repository.list(ctx.tenant_id()).await?)
}

pub async fn get_zone(
    ctx: &RequestContext,
    repository: &dyn ZoneRepository,
    zone_id: &str,
) -> Result<ZoneRecord, AppError> {
    policy::require(ctx, Permission::ReadZones)?;
    repository
        .get(ctx.tenant_id(), zone_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Zone '{zone_id}' not found")))
}

pub async fn create_zone(
    ctx: &RequestContext,
    repository: &dyn ZoneRepository,
    name: String,
    description: String,
    geometry_type: String,
    geometry_json: JsonValue,
    color: String,
) -> Result<ZoneRecord, AppError> {
    policy::require(ctx, Permission::ManageZones)?;
    validate_geometry(&geometry_type, &geometry_json)?;
    Ok(repository
        .create(
            ctx.tenant_id(),
            NewZoneRecord {
                id: Uuid::new_v4().to_string(),
                name,
                description,
                geometry_type,
                geometry_json,
                color,
            },
        )
        .await?)
}

pub async fn update_zone(
    ctx: &RequestContext,
    repository: &dyn ZoneRepository,
    zone_id: &str,
    update: ZoneUpdate,
) -> Result<ZoneRecord, AppError> {
    policy::require(ctx, Permission::ManageZones)?;
    let existing = repository
        .get(ctx.tenant_id(), zone_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Zone '{zone_id}' not found")))?;
    validate_geometry(
        update
            .geometry_type
            .as_deref()
            .unwrap_or(&existing.geometry_type),
        update
            .geometry_json
            .as_ref()
            .unwrap_or(&existing.geometry_json),
    )?;
    repository
        .update(
            ctx.tenant_id(),
            zone_id,
            UpdateZoneRecord {
                name: update.name,
                description: update.description,
                geometry_type: update.geometry_type,
                geometry_json: update.geometry_json,
                color: update.color,
                updated_at: chrono::Utc::now().naive_utc(),
            },
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Zone '{zone_id}' not found")))
}

pub async fn delete_zone(
    ctx: &RequestContext,
    repository: &dyn ZoneRepository,
    zone_id: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageZones)?;
    match repository.delete(ctx.tenant_id(), zone_id).await? {
        DeleteZoneOutcome::NotFound => {
            Err(AppError::NotFound(format!("Zone '{zone_id}' not found")))
        }
        DeleteZoneOutcome::InUse => Err(AppError::Conflict(
            "Cannot delete zone: referenced by rules".into(),
        )),
        DeleteZoneOutcome::Deleted => Ok(()),
    }
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
