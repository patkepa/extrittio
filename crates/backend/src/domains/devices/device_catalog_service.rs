use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::devices::repository::DeviceRepository;
use crate::domains::devices::types::{
    CreateDeviceRecord, DeviceContractRecord, DeviceDetails, DeviceFilter, DeviceListQuery,
    UpdateDeviceRecord,
};
use crate::domains::identity::certificate_repository::CertificateRepository;
use crate::error::AppError;
use crate::persistence::PersistenceError;
use crate::services::cert_service;

fn validate_device_id(device_id: &str) -> Result<(), AppError> {
    if extrittio_common::topics::is_valid_device_id(device_id) {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "Device ID must be 1-128 characters and contain only letters, numbers, '-', '_', '.', or ':'"
                .into(),
        ))
    }
}

fn map_device_write_error(error: PersistenceError) -> AppError {
    match error {
        PersistenceError::UniqueViolation { .. } => {
            AppError::Conflict("A device with this name already exists".into())
        }
        other => other.into(),
    }
}

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    query: DeviceListQuery,
) -> Result<(Vec<DeviceDetails>, i64), AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    let result = repository.list(ctx.tenant_id(), query).await?;
    Ok((result.records, result.total))
}

pub async fn resolve_identity(
    repository: &dyn DeviceRepository,
    device_id: &str,
) -> Result<Option<crate::tenancy::DeviceIdentity>, AppError> {
    validate_device_id(device_id)?;
    Ok(repository.resolve_identity(device_id).await?)
}

pub async fn get(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    device_id: &str,
) -> Result<DeviceDetails, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    repository
        .get(ctx.tenant_id(), device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub async fn assigned_contract(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    device_id: &str,
) -> Result<DeviceContractRecord, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    repository
        .assigned_contract(ctx.tenant_id(), device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' has no assigned contract")))
}

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    certificates: &dyn CertificateRepository,
    record: CreateDeviceRecord,
) -> Result<DeviceDetails, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    validate_device_id(&record.id)?;
    if record.contract.is_none() {
        return Err(AppError::BadRequest(
            "A compiled device contract is required; create the device from a published blueprint"
                .into(),
        ));
    }
    let certificate = if let Some(ca) = certificates.get_ca().await? {
        let tenant_id = ctx.tenant_id_str().to_string();
        let device_id = record.id.clone();
        let generated = tokio::task::spawn_blocking(move || {
            cert_service::generate_device_certificate_for_tenant(&tenant_id, &device_id, &ca)
        })
        .await
        .map_err(|error| AppError::Internal(format!("certificate task failed: {error}")))??;
        Some(generated)
    } else {
        None
    };
    repository
        .create(ctx.tenant_id(), record, certificate)
        .await
        .map_err(map_device_write_error)
}

pub async fn update(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    device_id: &str,
    record: UpdateDeviceRecord,
) -> Result<DeviceDetails, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    repository
        .update(ctx.tenant_id(), device_id, record)
        .await
        .map_err(map_device_write_error)?
        .ok_or_else(|| AppError::NotFound(format!("Device '{device_id}' not found")))
}

pub struct DeviceTargetSelection<'a> {
    pub device_ids: Option<&'a [String]>,
    pub select_all: bool,
    pub status: Option<&'a str>,
    pub search: Option<&'a str>,
    pub fleet_id: Option<i32>,
    pub max_size: usize,
}

pub async fn resolve_target_ids(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    selection: DeviceTargetSelection<'_>,
) -> Result<Vec<String>, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    let ids = if selection.select_all {
        repository
            .resolve_ids(
                ctx.tenant_id(),
                DeviceFilter {
                    status: selection.status.map(ToOwned::to_owned),
                    search: selection.search.map(ToOwned::to_owned),
                    fleet_id: selection.fleet_id,
                },
            )
            .await?
    } else {
        selection
            .device_ids
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Either device_ids or select_all with filters is required".into(),
                )
            })?
            .to_vec()
    };
    if ids.len() > selection.max_size {
        return Err(AppError::BadRequest(format!(
            "Too many devices ({}). Maximum is {}. Narrow your filters.",
            ids.len(),
            selection.max_size
        )));
    }
    Ok(ids)
}

pub async fn delete(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    device_id: &str,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    if !repository.delete(ctx.tenant_id(), device_id).await? {
        return Err(AppError::NotFound(format!(
            "Device '{device_id}' not found"
        )));
    }
    Ok(())
}

pub async fn bulk_assign_fleet(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    device_ids: Vec<String>,
    fleet_id: Option<i32>,
) -> Result<usize, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    Ok(repository
        .bulk_assign_fleet(ctx.tenant_id(), device_ids, fleet_id, chrono::Utc::now())
        .await?)
}

pub async fn bulk_delete(
    ctx: &RequestContext,
    repository: &dyn DeviceRepository,
    device_ids: Vec<String>,
) -> Result<usize, AppError> {
    policy::require(ctx, Permission::ManageDevices)?;
    Ok(repository.bulk_delete(ctx.tenant_id(), device_ids).await?)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::auth::Claims;
    use crate::domains::devices::types::{
        DeviceIngressContext, DeviceList, DeviceWriteOutcome, HeartbeatWrite, OfflineTransition,
        OfflineWriteOutcome,
    };
    use crate::domains::identity::certificate_types::NewDeviceCertificateRecord;
    use crate::tenancy::TenantId;

    #[derive(Default)]
    struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId)>>,
    }

    impl RecordingRepository {
        fn record(&self, operation: &str, tenant: &TenantId) {
            self.calls
                .lock()
                .unwrap()
                .push((operation.to_string(), tenant.clone()));
        }
    }

    #[async_trait]
    impl DeviceRepository for RecordingRepository {
        async fn resolve_identity(
            &self,
            _device_id: &str,
        ) -> Result<Option<crate::tenancy::DeviceIdentity>, PersistenceError> {
            Ok(None)
        }

        async fn ingress_context(
            &self,
            _identity: &crate::tenancy::DeviceIdentity,
        ) -> Result<Option<DeviceIngressContext>, PersistenceError> {
            Ok(None)
        }

        async fn apply_heartbeat(
            &self,
            _identity: &crate::tenancy::DeviceIdentity,
            _write: HeartbeatWrite,
        ) -> Result<DeviceWriteOutcome, PersistenceError> {
            Ok(DeviceWriteOutcome {
                applied: false,
                actions_enqueued: 0,
            })
        }

        async fn offline_candidates(
            &self,
            _cutoff: chrono::NaiveDateTime,
        ) -> Result<Vec<DeviceIngressContext>, PersistenceError> {
            Ok(Vec::new())
        }

        async fn apply_offline_transitions(
            &self,
            _cutoff: chrono::NaiveDateTime,
            _transitions: Vec<OfflineTransition>,
        ) -> Result<OfflineWriteOutcome, PersistenceError> {
            Ok(OfflineWriteOutcome {
                devices_updated: 0,
                actions_enqueued: 0,
            })
        }

        async fn list(
            &self,
            tenant: &TenantId,
            _query: DeviceListQuery,
        ) -> Result<DeviceList, PersistenceError> {
            self.record("list", tenant);
            Ok(DeviceList {
                records: Vec::new(),
                total: 0,
            })
        }

        async fn get(
            &self,
            tenant: &TenantId,
            _device_id: &str,
        ) -> Result<Option<DeviceDetails>, PersistenceError> {
            self.record("get", tenant);
            Ok(None)
        }

        async fn assigned_contract(
            &self,
            tenant: &TenantId,
            _device_id: &str,
        ) -> Result<Option<crate::domains::devices::types::DeviceContractRecord>, PersistenceError>
        {
            self.record("assigned_contract", tenant);
            Ok(None)
        }

        async fn create(
            &self,
            tenant: &TenantId,
            _record: CreateDeviceRecord,
            _certificate: Option<NewDeviceCertificateRecord>,
        ) -> Result<DeviceDetails, PersistenceError> {
            self.record("create", tenant);
            Err(PersistenceError::Internal("not used".into()))
        }

        async fn update(
            &self,
            tenant: &TenantId,
            _device_id: &str,
            _record: UpdateDeviceRecord,
        ) -> Result<Option<DeviceDetails>, PersistenceError> {
            self.record("update", tenant);
            Ok(None)
        }

        async fn delete(
            &self,
            tenant: &TenantId,
            _device_id: &str,
        ) -> Result<bool, PersistenceError> {
            self.record("delete", tenant);
            Ok(true)
        }

        async fn resolve_ids(
            &self,
            tenant: &TenantId,
            _filter: DeviceFilter,
        ) -> Result<Vec<String>, PersistenceError> {
            self.record("resolve_ids", tenant);
            Ok(vec!["device-a".into()])
        }

        async fn bulk_assign_fleet(
            &self,
            tenant: &TenantId,
            device_ids: Vec<String>,
            _fleet_id: Option<i32>,
            _updated_at: chrono::DateTime<chrono::Utc>,
        ) -> Result<usize, PersistenceError> {
            self.record("bulk_assign_fleet", tenant);
            Ok(device_ids.len())
        }

        async fn bulk_delete(
            &self,
            tenant: &TenantId,
            device_ids: Vec<String>,
        ) -> Result<usize, PersistenceError> {
            self.record("bulk_delete", tenant);
            Ok(device_ids.len())
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
            auth_epoch: Some("test-auth-epoch".to_string()),
            exp: 0,
        })
        .expect("test claims contain a valid tenant")
    }

    #[tokio::test]
    async fn catalog_and_bulk_operations_preserve_tenant_identity() {
        let repository = RecordingRepository::default();
        let ctx = context("admin", "tenant-a");

        list(
            &ctx,
            &repository,
            DeviceListQuery {
                status: None,
                search: None,
                fleet_id: None,
                limit: 25,
                offset: 0,
            },
        )
        .await
        .unwrap();
        let ids = resolve_target_ids(
            &ctx,
            &repository,
            DeviceTargetSelection {
                device_ids: None,
                select_all: true,
                status: None,
                search: None,
                fleet_id: None,
                max_size: 10,
            },
        )
        .await
        .unwrap();
        bulk_assign_fleet(&ctx, &repository, ids.clone(), Some(4))
            .await
            .unwrap();
        bulk_delete(&ctx, &repository, ids).await.unwrap();

        let tenant = TenantId::new("tenant-a").unwrap();
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[
                ("list".into(), tenant.clone()),
                ("resolve_ids".into(), tenant.clone()),
                ("bulk_assign_fleet".into(), tenant.clone()),
                ("bulk_delete".into(), tenant),
            ]
        );
    }

    #[tokio::test]
    async fn authorization_happens_before_bulk_persistence() {
        let repository = RecordingRepository::default();
        let error = bulk_delete(
            &context("viewer", "tenant-a"),
            &repository,
            vec!["device-a".into()],
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::Forbidden(_)));
        assert!(repository.calls.lock().unwrap().is_empty());
    }
}
