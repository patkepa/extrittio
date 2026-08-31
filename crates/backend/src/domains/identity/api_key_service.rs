use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::identity::api_key_repository::ApiKeyRepository;
use crate::domains::identity::api_key_types::{ApiKeyRecord, ApiKeySummary, CreateApiKeyRecord};
use crate::error::AppError;

pub async fn create(
    ctx: &RequestContext,
    repository: &dyn ApiKeyRepository,
    record: CreateApiKeyRecord,
) -> Result<ApiKeyRecord, AppError> {
    policy::require(ctx, Permission::ManageApiKeys)?;
    if record.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name is required".into()));
    }
    Ok(repository.create(ctx.tenant_id(), record).await?)
}

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn ApiKeyRepository,
) -> Result<Vec<ApiKeySummary>, AppError> {
    policy::require(ctx, Permission::ManageApiKeys)?;
    Ok(repository.list(ctx.tenant_id()).await?)
}

pub async fn delete(
    ctx: &RequestContext,
    repository: &dyn ApiKeyRepository,
    id: i32,
) -> Result<(), AppError> {
    policy::require(ctx, Permission::ManageApiKeys)?;
    if repository.delete(ctx.tenant_id(), id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("API key not found".into()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::auth::Claims;
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
    impl ApiKeyRepository for RecordingRepository {
        async fn create(
            &self,
            tenant: &TenantId,
            record: CreateApiKeyRecord,
        ) -> Result<ApiKeyRecord, PersistenceError> {
            self.record("create", tenant);
            Ok(ApiKeyRecord {
                id: 1,
                name: record.name,
                key_prefix: record.key_prefix,
                device_type_id: record.device_type_id,
                created_at: Utc.timestamp_opt(1, 0).unwrap(),
                last_used_at: None,
            })
        }

        async fn list(&self, tenant: &TenantId) -> Result<Vec<ApiKeySummary>, PersistenceError> {
            self.record("list", tenant);
            Ok(Vec::new())
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
            exp: 0,
        })
        .expect("test claims contain a valid tenant")
    }

    fn create_record(name: &str) -> CreateApiKeyRecord {
        CreateApiKeyRecord {
            name: name.to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "extr_test".to_string(),
            device_type_id: None,
        }
    }

    #[tokio::test]
    async fn passes_tenant_identity_to_every_operation() {
        let repository = RecordingRepository::default();
        let context = context("admin", "tenant-a");

        create(&context, &repository, create_record("CI"))
            .await
            .unwrap();
        list(&context, &repository).await.unwrap();
        delete(&context, &repository, 1).await.unwrap();

        let tenant = TenantId::new("tenant-a").unwrap();
        assert_eq!(
            repository.calls.lock().unwrap().as_slice(),
            &[
                ("create".to_string(), tenant.clone()),
                ("list".to_string(), tenant.clone()),
                ("delete".to_string(), tenant),
            ]
        );
    }

    #[tokio::test]
    async fn authorization_and_validation_happen_before_persistence() {
        let repository = RecordingRepository::default();

        let error = create(
            &context("viewer", "tenant-a"),
            &repository,
            create_record("CI"),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::Forbidden(_)));

        let error = create(
            &context("admin", "tenant-a"),
            &repository,
            create_record("   "),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::UnprocessableEntity(_)));
        assert!(repository.calls.lock().unwrap().is_empty());
    }
}
