use chrono::Utc;
use serde_json::{Map, Value};

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::configuration::repository::DeviceConfigRepository;
use crate::domains::configuration::types::{
    DeviceConfigRecord, GetDeviceConfigOutcome, MergeDeviceConfigOutcome,
};
use crate::error::AppError;

/// Get the current configuration for a device, or `None` if it has not been
/// set. A missing tenant-scoped device remains a 404.
pub async fn get_config(
    ctx: &RequestContext,
    repository: &dyn DeviceConfigRepository,
    device_id: &str,
) -> Result<Option<DeviceConfigRecord>, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;

    match repository
        .get_for_device(ctx.tenant_id(), device_id)
        .await?
    {
        GetDeviceConfigOutcome::DeviceNotFound => Err(AppError::NotFound(format!(
            "Device '{device_id}' not found"
        ))),
        GetDeviceConfigOutcome::Found(config) => Ok(config),
    }
}

/// Atomically merge a JSON patch into the latest device configuration. Null
/// values remove keys; all other values replace or add keys.
pub async fn merge_and_update(
    ctx: &RequestContext,
    repository: &dyn DeviceConfigRepository,
    device_id: &str,
    patch: &Map<String, Value>,
) -> Result<DeviceConfigRecord, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;

    match repository
        .merge_for_device(ctx.tenant_id(), device_id, patch.clone(), Utc::now())
        .await?
    {
        MergeDeviceConfigOutcome::DeviceNotFound => Err(AppError::NotFound(format!(
            "Device '{device_id}' not found"
        ))),
        MergeDeviceConfigOutcome::Updated(config) => Ok(config),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use serde_json::json;

    use super::*;
    use crate::auth::Claims;
    use crate::persistence::PersistenceError;
    use crate::tenancy::TenantId;

    #[derive(Default)]
    struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId, String)>>,
    }

    impl RecordingRepository {
        fn record(&self, operation: &str, tenant: &TenantId, device_id: &str) {
            self.calls.lock().unwrap().push((
                operation.to_string(),
                tenant.clone(),
                device_id.to_string(),
            ));
        }
    }

    #[async_trait]
    impl DeviceConfigRepository for RecordingRepository {
        async fn get_for_device(
            &self,
            tenant: &TenantId,
            device_id: &str,
        ) -> Result<GetDeviceConfigOutcome, PersistenceError> {
            self.record("get", tenant, device_id);
            Ok(GetDeviceConfigOutcome::Found(None))
        }

        async fn merge_for_device(
            &self,
            tenant: &TenantId,
            device_id: &str,
            patch: Map<String, Value>,
            updated_at: DateTime<Utc>,
        ) -> Result<MergeDeviceConfigOutcome, PersistenceError> {
            self.record("merge", tenant, device_id);
            Ok(MergeDeviceConfigOutcome::Updated(DeviceConfigRecord {
                device_id: device_id.to_string(),
                config: Value::Object(patch),
                updated_at,
            }))
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
        .expect("test claims contain a valid tenant")
    }

    #[tokio::test]
    async fn passes_tenant_identity_to_get_and_atomic_merge() {
        let repository = RecordingRepository::default();
        let context = context("admin", "tenant-a");

        get_config(&context, &repository, "device-1").await.unwrap();
        merge_and_update(
            &context,
            &repository,
            "device-1",
            json!({ "enabled": true }).as_object().unwrap(),
        )
        .await
        .unwrap();

        let tenant = TenantId::new("tenant-a").unwrap();
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[
                ("get".to_string(), tenant.clone(), "device-1".to_string()),
                ("merge".to_string(), tenant, "device-1".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn authorization_happens_before_persistence() {
        let repository = RecordingRepository::default();
        let patch = Map::default();

        let error = merge_and_update(
            &context("viewer", "tenant-a"),
            &repository,
            "device-1",
            &patch,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::Forbidden(_)));
        assert!(repository.calls.lock().unwrap().is_empty());
    }
}
