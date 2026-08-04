use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::device_types::repository::DeviceTypeRepository;
use crate::domains::device_types::types::{
    CreateDeviceTypeRecord, DeleteDeviceTypeOutcome, DeviceTypeRecord, UpdateDeviceTypeRecord,
};
use crate::error::AppError;

const DEFAULT_ICON: &str = "cube";
const DEFAULT_COLOR_HEX: &str = "#8ABBFF";

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn DeviceTypeRepository,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DeviceTypeRecord>, i64), AppError> {
    policy::require(ctx, Permission::ReadDeviceTypes)?;
    let result = repository.list(ctx.tenant_id(), limit, offset).await?;
    Ok((result.records, result.total))
}

fn validate_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(
            "Device type name must not be empty".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_icon(icon: &str) -> Result<String, AppError> {
    let trimmed = icon.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(AppError::BadRequest(
            "Device type icon must be 1-64 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AppError::BadRequest(
            "Device type icon must use lowercase letters, digits, hyphen, or underscore".into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_color_hex(color_hex: &str) -> Result<String, AppError> {
    let trimmed = color_hex.trim();
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if !valid {
        return Err(AppError::BadRequest(
            "Device type color must be a #RRGGBB hex value".into(),
        ));
    }
    Ok(trimmed.to_ascii_uppercase())
}

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn DeviceTypeRepository,
    name: &str,
    icon: Option<&str>,
    color_hex: Option<&str>,
) -> Result<DeviceTypeRecord, AppError> {
    policy::require(ctx, Permission::ManageDeviceTypes)?;

    let record = CreateDeviceTypeRecord {
        name: validate_name(name)?,
        icon: validate_icon(icon.unwrap_or(DEFAULT_ICON))?,
        color_hex: validate_color_hex(color_hex.unwrap_or(DEFAULT_COLOR_HEX))?,
    };
    Ok(repository.create(ctx.tenant_id(), record).await?)
}

pub async fn update(
    ctx: &RequestContext,
    repository: &dyn DeviceTypeRepository,
    id: i32,
    name: Option<&str>,
    icon: Option<&str>,
    color_hex: Option<&str>,
) -> Result<DeviceTypeRecord, AppError> {
    policy::require(ctx, Permission::ManageDeviceTypes)?;

    let changes = UpdateDeviceTypeRecord {
        name: name.map(validate_name).transpose()?,
        icon: icon.map(validate_icon).transpose()?,
        color_hex: color_hex.map(validate_color_hex).transpose()?,
    };

    let record = if changes.name.is_none() && changes.icon.is_none() && changes.color_hex.is_none()
    {
        repository.get_by_id(ctx.tenant_id(), id).await?
    } else {
        repository.update(ctx.tenant_id(), id, changes).await?
    };
    record.ok_or_else(|| AppError::NotFound(format!("Device type {id} not found")))
}

pub async fn delete(
    ctx: &RequestContext,
    repository: &dyn DeviceTypeRepository,
    id: i32,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageDeviceTypes)?;

    if id == 1 {
        return Err(AppError::UnprocessableEntity(
            "Cannot delete the default device type".into(),
        ));
    }

    match repository.delete_if_unused(ctx.tenant_id(), id).await? {
        DeleteDeviceTypeOutcome::Deleted => Ok(()),
        DeleteDeviceTypeOutcome::NotFound => {
            Err(AppError::NotFound(format!("Device type {id} not found")))
        }
        DeleteDeviceTypeOutcome::InUse { device_count } => Err(AppError::Conflict(format!(
            "Cannot delete device type: {device_count} device(s) still reference it"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::auth::Claims;
    use crate::domains::device_types::types::DeviceTypeList;
    use crate::persistence::PersistenceError;
    use crate::tenancy::TenantId;

    struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId)>>,
        delete_outcome: DeleteDeviceTypeOutcome,
    }

    impl RecordingRepository {
        fn new(delete_outcome: DeleteDeviceTypeOutcome) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                delete_outcome,
            }
        }

        fn record(&self, operation: &str, tenant: &TenantId) {
            self.calls
                .lock()
                .unwrap()
                .push((operation.to_string(), tenant.clone()));
        }
    }

    #[async_trait]
    impl DeviceTypeRepository for RecordingRepository {
        async fn list(
            &self,
            tenant: &TenantId,
            _limit: i64,
            _offset: i64,
        ) -> Result<DeviceTypeList, PersistenceError> {
            self.record("list", tenant);
            Ok(DeviceTypeList {
                records: Vec::new(),
                total: 0,
            })
        }

        async fn create(
            &self,
            tenant: &TenantId,
            record: CreateDeviceTypeRecord,
        ) -> Result<DeviceTypeRecord, PersistenceError> {
            self.record("create", tenant);
            Ok(DeviceTypeRecord {
                id: 2,
                name: record.name,
                icon: record.icon,
                color_hex: record.color_hex,
            })
        }

        async fn update(
            &self,
            tenant: &TenantId,
            id: i32,
            record: UpdateDeviceTypeRecord,
        ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
            self.record("update", tenant);
            Ok(Some(DeviceTypeRecord {
                id,
                name: record.name.unwrap_or_else(|| "Type".to_string()),
                icon: record.icon.unwrap_or_else(|| DEFAULT_ICON.to_string()),
                color_hex: record
                    .color_hex
                    .unwrap_or_else(|| DEFAULT_COLOR_HEX.to_string()),
            }))
        }

        async fn get_by_id(
            &self,
            tenant: &TenantId,
            id: i32,
        ) -> Result<Option<DeviceTypeRecord>, PersistenceError> {
            self.record("get", tenant);
            Ok(Some(DeviceTypeRecord {
                id,
                name: "Type".to_string(),
                icon: DEFAULT_ICON.to_string(),
                color_hex: DEFAULT_COLOR_HEX.to_string(),
            }))
        }

        async fn delete_if_unused(
            &self,
            tenant: &TenantId,
            _id: i32,
        ) -> Result<DeleteDeviceTypeOutcome, PersistenceError> {
            self.record("delete", tenant);
            Ok(self.delete_outcome.clone())
        }
    }

    fn context(role: &str, tenant_id: &str) -> RequestContext {
        RequestContext::from_claims(Claims {
            sub: 1,
            username: "operator".to_string(),
            role: role.to_string(),
            tenant_id: Some(tenant_id.to_string()),
            scopes: Vec::new(),
            permission_version: 1,
            exp: 0,
        })
    }

    #[tokio::test]
    async fn passes_tenant_and_normalized_values_to_the_repository() {
        let repository = RecordingRepository::new(DeleteDeviceTypeOutcome::Deleted);
        let context = context("admin", "tenant-a");

        let created = create(
            &context,
            &repository,
            "  Sensor  ",
            Some("air-quality"),
            Some("#aabbcc"),
        )
        .await
        .unwrap();
        assert_eq!(created.name, "Sensor");
        assert_eq!(created.color_hex, "#AABBCC");
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[("create".to_string(), TenantId::new("tenant-a").unwrap())]
        );
    }

    #[tokio::test]
    async fn authorization_validation_and_default_protection_precede_persistence() {
        let repository = RecordingRepository::new(DeleteDeviceTypeOutcome::Deleted);

        let error = create(
            &context("viewer", "tenant-a"),
            &repository,
            "Sensor",
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));

        let error = create(
            &context("admin", "tenant-a"),
            &repository,
            "Sensor",
            Some("INVALID"),
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));

        let error = delete(&context("admin", "tenant-a"), &repository, 1)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::UnprocessableEntity(_)));
        assert!(repository.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn maps_atomic_in_use_outcome_to_conflict() {
        let repository =
            RecordingRepository::new(DeleteDeviceTypeOutcome::InUse { device_count: 3 });

        let error = delete(&context("admin", "tenant-a"), &repository, 2)
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Conflict(message) if message.contains('3')));
    }
}
