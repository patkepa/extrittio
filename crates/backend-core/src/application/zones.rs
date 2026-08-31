use std::sync::Arc;

use serde_json::Value;
use uuid::Uuid;

use crate::TenantContext;
use crate::context::Permission;
use crate::error::ApplicationError;
use crate::zones::{DeleteZoneOutcome, NewZone, Zone, ZonePatch, ZoneRepository};

#[derive(Debug, Clone, PartialEq)]
pub struct CreateZone {
    pub name: String,
    pub description: String,
    pub geometry_type: String,
    pub geometry_json: Value,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub geometry_type: Option<String>,
    pub geometry_json: Option<Value>,
    pub color: Option<String>,
}

#[derive(Clone)]
pub struct ZoneApplication {
    repository: Arc<dyn ZoneRepository>,
}

impl ZoneApplication {
    #[must_use]
    pub fn new(repository: Arc<dyn ZoneRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(&self, context: &TenantContext) -> Result<Vec<Zone>, ApplicationError> {
        require(context, Permission::ReadZones)?;
        Ok(self.repository.list(context.tenant_id()).await?)
    }

    pub async fn get(
        &self,
        context: &TenantContext,
        zone_id: &str,
    ) -> Result<Zone, ApplicationError> {
        require(context, Permission::ReadZones)?;
        self.repository
            .get(context.tenant_id(), zone_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Zone '{zone_id}' not found")))
    }

    pub async fn create(
        &self,
        context: &TenantContext,
        input: CreateZone,
    ) -> Result<Zone, ApplicationError> {
        require(context, Permission::ManageZones)?;
        validate_geometry(&input.geometry_type, &input.geometry_json)?;
        Ok(self
            .repository
            .create(
                context.tenant_id(),
                NewZone {
                    id: Uuid::new_v4().to_string(),
                    name: input.name,
                    description: input.description,
                    geometry_type: input.geometry_type,
                    geometry_json: input.geometry_json,
                    color: input.color,
                },
            )
            .await?)
    }

    pub async fn update(
        &self,
        context: &TenantContext,
        zone_id: &str,
        update: ZoneUpdate,
    ) -> Result<Zone, ApplicationError> {
        require(context, Permission::ManageZones)?;
        let existing = self
            .repository
            .get(context.tenant_id(), zone_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Zone '{zone_id}' not found")))?;
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
        self.repository
            .update(
                context.tenant_id(),
                zone_id,
                ZonePatch {
                    name: update.name,
                    description: update.description,
                    geometry_type: update.geometry_type,
                    geometry_json: update.geometry_json,
                    color: update.color,
                    updated_at: chrono::Utc::now(),
                },
            )
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Zone '{zone_id}' not found")))
    }

    pub async fn delete(
        &self,
        context: &TenantContext,
        zone_id: &str,
    ) -> Result<(), ApplicationError> {
        require(context, Permission::ManageZones)?;
        match self.repository.delete(context.tenant_id(), zone_id).await? {
            DeleteZoneOutcome::NotFound => Err(ApplicationError::NotFound(format!(
                "Zone '{zone_id}' not found"
            ))),
            DeleteZoneOutcome::InUse => Err(ApplicationError::Conflict(
                "Cannot delete zone: referenced by rules".into(),
            )),
            DeleteZoneOutcome::Deleted => Ok(()),
        }
    }
}

fn require(context: &TenantContext, permission: Permission) -> Result<(), ApplicationError> {
    if context.permissions().contains(permission) {
        Ok(())
    } else {
        Err(ApplicationError::Forbidden(format!(
            "Missing permission '{}'",
            permission.key()
        )))
    }
}

fn validate_geometry(geometry_type: &str, geometry_json: &Value) -> Result<(), ApplicationError> {
    match geometry_type {
        "circle" => {
            let center = geometry_json
                .get("center")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ApplicationError::InvalidInput("Circle geometry requires center".into())
                })?;
            if center.len() != 2
                || !center[0].as_f64().is_some_and(f64::is_finite)
                || !center[1].as_f64().is_some_and(f64::is_finite)
            {
                return Err(ApplicationError::InvalidInput(
                    "Circle center must be [latitude, longitude]".into(),
                ));
            }
            let radius = geometry_json
                .get("radius_meters")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    ApplicationError::InvalidInput("Circle geometry requires radius_meters".into())
                })?;
            if !radius.is_finite() || radius <= 0.0 {
                return Err(ApplicationError::InvalidInput(
                    "Circle radius_meters must be a positive number".into(),
                ));
            }
            Ok(())
        }
        "polygon" => {
            let points = geometry_json
                .get("points")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ApplicationError::InvalidInput("Polygon geometry requires points".into())
                })?;
            if points.len() < 3 {
                return Err(ApplicationError::InvalidInput(
                    "Polygon geometry requires at least three points".into(),
                ));
            }
            for point in points {
                let coordinates = point.as_array().ok_or_else(|| {
                    ApplicationError::InvalidInput(
                        "Polygon points must be [latitude, longitude] arrays".into(),
                    )
                })?;
                if coordinates.len() != 2
                    || !coordinates[0].as_f64().is_some_and(f64::is_finite)
                    || !coordinates[1].as_f64().is_some_and(f64::is_finite)
                {
                    return Err(ApplicationError::InvalidInput(
                        "Polygon points must contain finite latitude and longitude values".into(),
                    ));
                }
            }
            Ok(())
        }
        _ => Err(ApplicationError::InvalidInput(format!(
            "Unsupported zone geometry type '{geometry_type}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::Utc;
    use futures::executor::block_on;
    use serde_json::json;

    use super::*;
    use crate::TenantId;
    use crate::context::{Actor, PermissionSet};
    use crate::error::PersistenceError;
    use crate::zones::DeleteZoneOutcome;

    #[derive(Default)]
    struct FakeZones {
        created: Mutex<Vec<(TenantId, NewZone)>>,
    }

    #[async_trait]
    impl ZoneRepository for FakeZones {
        async fn list(&self, _tenant: &TenantId) -> Result<Vec<Zone>, PersistenceError> {
            Ok(Vec::new())
        }

        async fn get(
            &self,
            _tenant: &TenantId,
            _zone_id: &str,
        ) -> Result<Option<Zone>, PersistenceError> {
            Ok(None)
        }

        async fn create(&self, tenant: &TenantId, zone: NewZone) -> Result<Zone, PersistenceError> {
            self.created
                .lock()
                .expect("fake lock")
                .push((tenant.clone(), zone.clone()));
            let now = Utc::now();
            Ok(Zone {
                id: zone.id,
                tenant_id: tenant.clone(),
                name: zone.name,
                description: zone.description,
                geometry_type: zone.geometry_type,
                geometry_json: zone.geometry_json,
                color: zone.color,
                created_at: now,
                updated_at: now,
            })
        }

        async fn update(
            &self,
            _tenant: &TenantId,
            _zone_id: &str,
            _patch: ZonePatch,
        ) -> Result<Option<Zone>, PersistenceError> {
            Ok(None)
        }

        async fn delete(
            &self,
            _tenant: &TenantId,
            _zone_id: &str,
        ) -> Result<DeleteZoneOutcome, PersistenceError> {
            Ok(DeleteZoneOutcome::NotFound)
        }
    }

    fn context(keys: &[&str]) -> TenantContext {
        TenantContext::new(
            TenantId::new("tenant-a").unwrap(),
            Actor::User {
                id: 7,
                username: "operator".into(),
                role: "operator".into(),
            },
            PermissionSet::from_keys(keys),
        )
    }

    #[test]
    fn create_validates_before_calling_the_repository() {
        let repository = Arc::new(FakeZones::default());
        let application = ZoneApplication::new(repository.clone());
        let error = block_on(application.create(
            &context(&["zones.manage"]),
            CreateZone {
                name: "Invalid".into(),
                description: String::new(),
                geometry_type: "circle".into(),
                geometry_json: json!({"center": [1.0, 2.0], "radius_meters": 0}),
                color: "#fff".into(),
            },
        ))
        .unwrap_err();

        assert!(matches!(error, ApplicationError::InvalidInput(_)));
        assert!(repository.created.lock().unwrap().is_empty());
    }

    #[test]
    fn geometry_validation_preserves_public_error_messages() {
        let cases = [
            (
                "circle",
                json!({"center": [1.0], "radius_meters": 10}),
                "Circle center must be [latitude, longitude]",
            ),
            (
                "circle",
                json!({"center": [1.0, 2.0], "radius_meters": 0}),
                "Circle radius_meters must be a positive number",
            ),
            (
                "polygon",
                json!({"points": [[1.0, 2.0], [3.0, 4.0]]}),
                "Polygon geometry requires at least three points",
            ),
            ("line", json!({}), "Unsupported zone geometry type 'line'"),
        ];

        for (geometry_type, geometry_json, expected) in cases {
            let error = validate_geometry(geometry_type, &geometry_json).unwrap_err();
            assert!(
                matches!(error, ApplicationError::InvalidInput(message) if message == expected)
            );
        }
    }

    #[test]
    fn create_passes_the_authenticated_tenant_to_the_port() {
        let repository = Arc::new(FakeZones::default());
        let application = ZoneApplication::new(repository.clone());
        let created = block_on(application.create(
            &context(&["zones.manage"]),
            CreateZone {
                name: "Office".into(),
                description: String::new(),
                geometry_type: "circle".into(),
                geometry_json: json!({"center": [1.0, 2.0], "radius_meters": 10}),
                color: "#fff".into(),
            },
        ))
        .unwrap();

        assert_eq!(created.tenant_id.as_str(), "tenant-a");
        assert_eq!(repository.created.lock().unwrap().len(), 1);
    }

    #[test]
    fn read_requires_the_read_or_manage_permission() {
        let application = ZoneApplication::new(Arc::new(FakeZones::default()));
        let error = block_on(application.list(&context(&[]))).unwrap_err();
        assert!(matches!(error, ApplicationError::Forbidden(_)));
        assert!(block_on(application.list(&context(&["zones.manage"]))).is_ok());
    }
}
