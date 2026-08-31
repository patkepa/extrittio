use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::fleets::repository::FleetRepository;
use crate::domains::fleets::types::{CreateFleetRecord, FleetRecord, FleetSummary};
use crate::error::AppError;

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn FleetRepository,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FleetSummary>, i64), AppError> {
    policy::require(ctx, Permission::ReadFleets)?;

    let result = repository.list(ctx.tenant_id(), limit, offset).await?;
    Ok((result.records, result.total))
}

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn FleetRepository,
    name: &str,
) -> Result<FleetRecord, AppError> {
    policy::require(ctx, Permission::ManageFleets)?;

    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }
    Ok(repository
        .create(
            ctx.tenant_id(),
            CreateFleetRecord {
                name: name.to_string(),
            },
        )
        .await?)
}

pub async fn rename(
    ctx: &RequestContext,
    repository: &dyn FleetRepository,
    id: i32,
    new_name: &str,
) -> Result<FleetRecord, AppError> {
    policy::require(ctx, Permission::ManageFleets)?;

    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("Fleet name must not be empty".into()));
    }
    repository
        .rename(ctx.tenant_id(), id, trimmed.to_string())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Fleet {id} not found")))
}

pub async fn delete(
    ctx: &RequestContext,
    repository: &dyn FleetRepository,
    id: i32,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageFleets)?;

    let deleted = repository.delete(ctx.tenant_id(), id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!("Fleet {id} not found")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::auth::Claims;
    use crate::domains::fleets::types::FleetList;
    use crate::persistence::PersistenceError;
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
    impl FleetRepository for RecordingRepository {
        async fn list(
            &self,
            tenant: &TenantId,
            _limit: i64,
            _offset: i64,
        ) -> Result<FleetList, PersistenceError> {
            self.record("list", tenant);
            Ok(FleetList {
                records: Vec::new(),
                total: 0,
            })
        }

        async fn create(
            &self,
            tenant: &TenantId,
            record: CreateFleetRecord,
        ) -> Result<FleetRecord, PersistenceError> {
            self.record("create", tenant);
            Ok(FleetRecord {
                id: 1,
                name: record.name,
            })
        }

        async fn rename(
            &self,
            tenant: &TenantId,
            id: i32,
            name: String,
        ) -> Result<Option<FleetRecord>, PersistenceError> {
            self.record("rename", tenant);
            Ok(Some(FleetRecord { id, name }))
        }

        async fn delete(&self, tenant: &TenantId, _id: i32) -> Result<bool, PersistenceError> {
            self.record("delete", tenant);
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
            auth_epoch: Some("test-auth-epoch".to_string()),
            exp: 0,
        })
        .expect("test claims contain a valid tenant")
    }

    #[tokio::test]
    async fn passes_validated_tenant_identity_to_every_operation() {
        let repository = RecordingRepository::default();
        let context = context("admin", "tenant-a");

        list(&context, &repository, 25, 0).await.unwrap();
        create(&context, &repository, "Production").await.unwrap();
        rename(&context, &repository, 1, "Renamed").await.unwrap();
        delete(&context, &repository, 1).await.unwrap();

        let tenant = TenantId::new("tenant-a").unwrap();
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[
                ("list".to_string(), tenant.clone()),
                ("create".to_string(), tenant.clone()),
                ("rename".to_string(), tenant.clone()),
                ("delete".to_string(), tenant),
            ]
        );
    }

    #[tokio::test]
    async fn authorization_and_validation_happen_before_persistence() {
        let repository = RecordingRepository::default();

        let error = create(&context("viewer", "tenant-a"), &repository, "Fleet")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));

        let error = create(&context("admin", "tenant-a"), &repository, "   ")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
        assert!(repository.calls.lock().unwrap().is_empty());
    }
}
