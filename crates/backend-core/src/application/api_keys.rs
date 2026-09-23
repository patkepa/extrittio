use super::require_permission;
use crate::{
    ApiKeyGenerator, ApiKeyRepository, ApiKeySummary, ApplicationError, CreateApiKey,
    CreateApiKeyRecord, CreatedApiKey, Permission, TenantContext,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiKeyApplication {
    repository: Arc<dyn ApiKeyRepository>,
    generator: Arc<dyn ApiKeyGenerator>,
}

impl ApiKeyApplication {
    pub fn new(repository: Arc<dyn ApiKeyRepository>, generator: Arc<dyn ApiKeyGenerator>) -> Self {
        Self {
            repository,
            generator,
        }
    }

    pub async fn create(
        &self,
        context: &TenantContext,
        input: CreateApiKey,
    ) -> Result<CreatedApiKey, ApplicationError> {
        // Preserve the HTTP endpoint's validation precedence.
        if input.name.trim().is_empty() {
            return Err(ApplicationError::InvalidInput("name is required".into()));
        }
        require_permission(context, Permission::ManageApiKeys)?;
        if input
            .blueprint_id
            .as_deref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(ApplicationError::InvalidInput(
                "blueprint_id must not be blank".into(),
            ));
        }
        let generated = self.generator.generate();
        let record = self
            .repository
            .create(
                context.tenant_id(),
                CreateApiKeyRecord {
                    name: input.name,
                    blueprint_id: input.blueprint_id,
                    key_hash: generated.hash,
                    key_prefix: generated.prefix,
                },
            )
            .await?;
        Ok(CreatedApiKey {
            record,
            plaintext: generated.plaintext,
        })
    }

    pub async fn list(
        &self,
        context: &TenantContext,
    ) -> Result<Vec<ApiKeySummary>, ApplicationError> {
        require_permission(context, Permission::ManageApiKeys)?;
        Ok(self.repository.list(context.tenant_id()).await?)
    }

    pub async fn delete(&self, context: &TenantContext, id: i32) -> Result<(), ApplicationError> {
        require_permission(context, Permission::ManageApiKeys)?;
        if self.repository.delete(context.tenant_id(), id).await? {
            Ok(())
        } else {
            Err(ApplicationError::NotFound("API key not found".into()))
        }
    }
}
