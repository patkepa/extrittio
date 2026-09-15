use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyRecord {
    pub id: i32,
    pub name: String,
    pub key_prefix: String,
    pub blueprint_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeySummary {
    pub key: ApiKeyRecord,
    pub blueprint_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateApiKeyRecord {
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub blueprint_id: Option<String>,
}

use async_trait::async_trait;

use crate::{PersistenceError, TenantId};

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateApiKeyRecord,
    ) -> Result<ApiKeyRecord, PersistenceError>;

    async fn list(&self, tenant: &TenantId) -> Result<Vec<ApiKeySummary>, PersistenceError>;

    async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError>;
}

pub struct CreateApiKey {
    pub name: String,
    pub blueprint_id: Option<String>,
}

/// Secret material returned only by creation; deliberately not Debug or Clone.
pub struct GeneratedApiKey {
    pub plaintext: String,
    pub hash: String,
    pub prefix: String,
}

pub struct CreatedApiKey {
    pub record: ApiKeyRecord,
    pub plaintext: String,
}

/// Host implementation retains the existing random key format and hash algorithm.
pub trait ApiKeyGenerator: Send + Sync {
    fn generate(&self) -> GeneratedApiKey;
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{Actor, ApiKeyApplication, ApplicationError, PermissionSet, TenantContext};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Default)]
    pub(crate) struct RecordingRepository {
        calls: Mutex<Vec<(String, TenantId)>>,
    }
    #[async_trait]
    impl ApiKeyRepository for RecordingRepository {
        async fn create(
            &self,
            tenant: &TenantId,
            record: CreateApiKeyRecord,
        ) -> Result<ApiKeyRecord, PersistenceError> {
            self.calls
                .lock()
                .unwrap()
                .push(("create".into(), tenant.clone()));
            assert_eq!(record.key_hash, "hash");
            Ok(ApiKeyRecord {
                id: 1,
                name: record.name,
                key_prefix: record.key_prefix,
                blueprint_id: record.blueprint_id,
                created_at: DateTime::UNIX_EPOCH,
                last_used_at: None,
            })
        }
        async fn list(&self, tenant: &TenantId) -> Result<Vec<ApiKeySummary>, PersistenceError> {
            self.calls
                .lock()
                .unwrap()
                .push(("list".into(), tenant.clone()));
            Ok(vec![])
        }
        async fn delete(&self, tenant: &TenantId, id: i32) -> Result<bool, PersistenceError> {
            self.calls
                .lock()
                .unwrap()
                .push(("delete".into(), tenant.clone()));
            Ok(id == 1)
        }
    }
    pub(crate) struct TestGenerator;
    impl ApiKeyGenerator for TestGenerator {
        fn generate(&self) -> GeneratedApiKey {
            GeneratedApiKey {
                plaintext: "secret".into(),
                hash: "hash".into(),
                prefix: "extr_test".into(),
            }
        }
    }
    #[derive(Default)]
    struct CountingGenerator(AtomicUsize);
    impl ApiKeyGenerator for CountingGenerator {
        fn generate(&self) -> GeneratedApiKey {
            self.0.fetch_add(1, Ordering::SeqCst);
            TestGenerator.generate()
        }
    }
    fn context(allowed: bool) -> TenantContext {
        TenantContext::new(
            TenantId::new("tenant-a").unwrap(),
            Actor::User {
                id: 1,
                username: "operator".into(),
                role: "custom".into(),
            },
            if allowed {
                PermissionSet::from_keys(["api_keys.manage"])
            } else {
                PermissionSet::default()
            },
        )
    }
    fn input(name: &str) -> CreateApiKey {
        CreateApiKey {
            name: name.into(),
            blueprint_id: Some("blueprint-42".into()),
        }
    }
    #[test]
    fn passes_tenant_to_every_operation_and_returns_secret_only_on_creation() {
        futures::executor::block_on(async {
            let repository = Arc::new(RecordingRepository::default());
            let application = ApiKeyApplication::new(repository.clone(), Arc::new(TestGenerator));
            let context = context(true);
            let created = application.create(&context, input(" CI ")).await.unwrap();
            assert_eq!(created.plaintext, "secret");
            assert_eq!(created.record.name, " CI ");
            assert_eq!(created.record.key_prefix, "extr_test");
            assert_eq!(created.record.blueprint_id, Some("blueprint-42".into()));
            application.list(&context).await.unwrap();
            application.delete(&context, 1).await.unwrap();
            assert_eq!(
                *repository.calls.lock().unwrap(),
                ["create", "list", "delete"].map(|op| (op.into(), context.tenant_id().clone()))
            );
            assert!(matches!(
                application.delete(&context, 999).await,
                Err(ApplicationError::NotFound(_))
            ));
        });
    }
    #[test]
    fn rejected_requests_do_not_generate_secrets_or_touch_persistence() {
        futures::executor::block_on(async {
            let repository = Arc::new(RecordingRepository::default());
            let generator = Arc::new(CountingGenerator::default());
            let application = ApiKeyApplication::new(repository.clone(), generator.clone());
            assert!(matches!(
                application.create(&context(false), input("CI")).await,
                Err(ApplicationError::Forbidden(_))
            ));
            assert!(matches!(
                application.create(&context(true), input("  ")).await,
                Err(ApplicationError::InvalidInput(_))
            ));
            assert!(matches!(
                application.create(&context(false), input("  ")).await,
                Err(ApplicationError::InvalidInput(_))
            ));
            assert!(matches!(
                application.list(&context(false)).await,
                Err(ApplicationError::Forbidden(_))
            ));
            assert!(matches!(
                application.delete(&context(false), 1).await,
                Err(ApplicationError::Forbidden(_))
            ));
            assert!(repository.calls.lock().unwrap().is_empty());
            assert_eq!(generator.0.load(Ordering::SeqCst), 0);
        });
    }
}
