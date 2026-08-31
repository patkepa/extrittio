use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::warn;

use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::firmware::port::FirmwareRepository;
use crate::domains::firmware::types::OtaStatusUpdate;
use crate::domains::shadows::repository::ShadowRepository;
use crate::domains::shadows::types::ShadowRecord;
use crate::error::AppError;
use crate::state::ZenohMetrics;
use crate::tenancy::DeviceIdentity;
use crate::tenancy::TenantId;

pub async fn get_shadow(
    ctx: &RequestContext,
    repository: &dyn ShadowRepository,
    device_id: &str,
) -> Result<ShadowRecord, AppError> {
    policy::require(ctx, Permission::ReadShadows)?;
    get_shadow_for_tenant(repository, ctx.tenant_id(), device_id).await
}

pub async fn get_shadow_for_tenant(
    repository: &dyn ShadowRepository,
    tenant: &TenantId,
    device_id: &str,
) -> Result<ShadowRecord, AppError> {
    repository
        .get(tenant, device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Shadow for device '{device_id}' not found")))
}

/// Merge desired state, commit it, and only then publish its delta.
pub async fn update_desired(
    ctx: &RequestContext,
    repository: &dyn ShadowRepository,
    zenoh_session: &Arc<zenoh::Session>,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
    zenoh_metrics: &ZenohMetrics,
) -> Result<ShadowRecord, AppError> {
    policy::require(ctx, Permission::ManageShadows)?;

    let shadow = repository
        .update_desired(
            ctx.tenant_id(),
            device_id,
            patch.clone(),
            chrono::Utc::now(),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Shadow for device '{device_id}' not found")))?;

    publish_delta_if_nonempty(
        zenoh_session,
        device_id,
        &shadow.delta,
        shadow.version,
        zenoh_metrics,
    )
    .await;
    Ok(shadow)
}

pub async fn update_reported(
    ctx: &RequestContext,
    repository: &dyn ShadowRepository,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<ShadowRecord, AppError> {
    // This preserves the endpoint's prior effective permission while ensuring
    // authorization happens before the state mutation.
    policy::require(ctx, Permission::ReadShadows)?;
    update_reported_for_tenant(repository, ctx.tenant_id(), device_id, patch).await
}

pub async fn update_reported_for_tenant(
    repository: &dyn ShadowRepository,
    tenant: &TenantId,
    device_id: &str,
    patch: &serde_json::Map<String, Value>,
) -> Result<ShadowRecord, AppError> {
    repository
        .update_reported(tenant, device_id, patch.clone(), chrono::Utc::now())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Shadow for device '{device_id}' not found")))
}

pub async fn delete_shadow(
    ctx: &RequestContext,
    repository: &dyn ShadowRepository,
    device_id: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageShadows)?;
    let reset = repository
        .reset(ctx.tenant_id(), device_id, chrono::Utc::now())
        .await?;
    if reset {
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "Shadow for device '{device_id}' not found"
        )))
    }
}

/// Publish a ShadowDelta via Zenoh if the delta is non-empty.
pub async fn publish_delta_if_nonempty(
    session: &Arc<zenoh::Session>,
    device_id: &str,
    delta: &Value,
    version: i32,
    zenoh_metrics: &ZenohMetrics,
) {
    if delta.as_object().is_some_and(|obj| !obj.is_empty()) {
        let delta_json = match serde_json::to_string(delta) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to serialize shadow delta for {device_id}: {e}");
                return;
            }
        };
        let delta_msg = extrittio_common::extrittio::ShadowDelta {
            device_id: device_id.to_string(),
            delta_json,
            version: i64::from(version),
        };
        let payload = prost::Message::encode_to_vec(&delta_msg);
        let topic = extrittio_common::topics::shadow_delta(device_id);
        if let Err(e) = session.put(&topic, payload).await {
            warn!("Failed to publish shadow delta to device {device_id}: {e}");
        } else {
            zenoh_metrics.messages_out.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub async fn process_ota_from_report_with_repository(
    repository: &dyn FirmwareRepository,
    identity: &DeviceIdentity,
    reported: &serde_json::Value,
) -> Result<(), AppError> {
    use extrittio_common::ota::{fields as ota_fields, status as ota_status_consts};

    let Some(serde_json::Value::Object(ota)) = reported.get(ota_fields::SHADOW_KEY) else {
        return Ok(());
    };
    let Some(status_raw) = ota
        .get(ota_fields::STATUS)
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(());
    };
    let status = status_raw.to_lowercase();
    let firmware_update_id = ota
        .get(ota_fields::FIRMWARE_UPDATE_ID)
        .and_then(serde_json::Value::as_i64)
        .and_then(|id| i32::try_from(id).ok());
    let error_message = ota
        .get(ota_fields::ERROR)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let completed_at =
        ota_status_consts::is_terminal(&status).then(|| chrono::Utc::now().naive_utc());
    repository
        .apply_ota_status(
            identity,
            OtaStatusUpdate {
                firmware_update_id,
                status,
                error_message,
                completed_at,
            },
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone, Utc};
    use serde_json::{Map, json};

    use super::*;
    use crate::auth::Claims;
    use crate::persistence::PersistenceError;

    #[derive(Default)]
    struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId, String)>>,
    }

    impl RecordingRepository {
        fn shadow(device_id: &str, updated_at: DateTime<Utc>) -> ShadowRecord {
            ShadowRecord {
                device_id: device_id.to_string(),
                desired: json!({}),
                reported: json!({}),
                delta: json!({}),
                version: 1,
                updated_at,
            }
        }

        fn record(&self, operation: &str, tenant: &TenantId, device_id: &str) {
            self.calls.lock().unwrap().push((
                operation.to_string(),
                tenant.clone(),
                device_id.to_string(),
            ));
        }
    }

    #[async_trait]
    impl ShadowRepository for RecordingRepository {
        async fn get(
            &self,
            tenant: &TenantId,
            device_id: &str,
        ) -> Result<Option<ShadowRecord>, PersistenceError> {
            self.record("get", tenant, device_id);
            Ok(Some(Self::shadow(
                device_id,
                Utc.timestamp_opt(1, 0).unwrap(),
            )))
        }

        async fn update_desired(
            &self,
            tenant: &TenantId,
            device_id: &str,
            _patch: Map<String, Value>,
            updated_at: DateTime<Utc>,
        ) -> Result<Option<ShadowRecord>, PersistenceError> {
            self.record("desired", tenant, device_id);
            Ok(Some(Self::shadow(device_id, updated_at)))
        }

        async fn update_reported(
            &self,
            tenant: &TenantId,
            device_id: &str,
            _patch: Map<String, Value>,
            updated_at: DateTime<Utc>,
        ) -> Result<Option<ShadowRecord>, PersistenceError> {
            self.record("reported", tenant, device_id);
            Ok(Some(Self::shadow(device_id, updated_at)))
        }

        async fn reset(
            &self,
            tenant: &TenantId,
            device_id: &str,
            _updated_at: DateTime<Utc>,
        ) -> Result<bool, PersistenceError> {
            self.record("reset", tenant, device_id);
            Ok(true)
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
    async fn passes_tenant_identity_to_reads_reports_and_resets() {
        let repository = RecordingRepository::default();
        let context = context("admin", "tenant-a");
        let patch = Map::default();

        get_shadow(&context, &repository, "device-1").await.unwrap();
        update_reported(&context, &repository, "device-1", &patch)
            .await
            .unwrap();
        delete_shadow(&context, &repository, "device-1")
            .await
            .unwrap();

        let tenant = TenantId::new("tenant-a").unwrap();
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[
                ("get".to_string(), tenant.clone(), "device-1".to_string()),
                (
                    "reported".to_string(),
                    tenant.clone(),
                    "device-1".to_string()
                ),
                ("reset".to_string(), tenant, "device-1".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn reported_update_authorizes_before_persistence() {
        let repository = RecordingRepository::default();
        let patch = Map::default();

        let error = update_reported(
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
