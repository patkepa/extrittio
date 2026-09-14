use super::require_permission;
use crate::fleets::*;
use crate::{ApplicationError, Permission, TenantContext};
use std::sync::Arc;

#[derive(Clone)]
pub struct FleetApplication {
    repository: Arc<dyn FleetRepository>,
}
impl FleetApplication {
    pub fn new(repository: Arc<dyn FleetRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<FleetSummary>, i64), ApplicationError> {
        require_permission(ctx, Permission::ReadFleets)?;

        let result = self.repository.list(ctx.tenant_id(), limit, offset).await?;
        Ok((result.records, result.total))
    }

    pub async fn create(
        &self,
        ctx: &TenantContext,
        name: &str,
    ) -> Result<FleetRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageFleets)?;

        if name.trim().is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Fleet name must not be empty".into(),
            ));
        }
        Ok(self
            .repository
            .create(
                ctx.tenant_id(),
                CreateFleetRecord {
                    name: name.to_string(),
                },
            )
            .await?)
    }

    pub async fn rename(
        &self,
        ctx: &TenantContext,
        id: i32,
        new_name: &str,
    ) -> Result<FleetRecord, ApplicationError> {
        require_permission(ctx, Permission::ManageFleets)?;

        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Fleet name must not be empty".into(),
            ));
        }
        self.repository
            .rename(ctx.tenant_id(), id, trimmed.to_string())
            .await?
            .ok_or_else(|| ApplicationError::NotFound(format!("Fleet {id} not found")))
    }

    pub async fn delete(&self, ctx: &TenantContext, id: i32) -> Result<(), ApplicationError> {
        require_permission(ctx, Permission::ManageFleets)?;

        let deleted = self.repository.delete(ctx.tenant_id(), id).await?;
        if !deleted {
            return Err(ApplicationError::NotFound(format!("Fleet {id} not found")));
        }
        Ok(())
    }
}
