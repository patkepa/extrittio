use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::alerts::port::AlertRepository;
use crate::domains::alerts::types::{
    AlertListFilter, AlertRecord, AlertTransition, AlertTransitionOutcome, CooldownRecord,
};
use crate::error::AppError;

pub async fn list_with_repository(
    ctx: &RequestContext,
    repository: &dyn AlertRepository,
    filter: AlertListFilter,
) -> Result<(Vec<AlertRecord>, i64), AppError> {
    policy::require(ctx, Permission::ReadAlerts)?;
    Ok(repository.list(ctx.tenant_id(), filter).await?)
}

pub async fn get_with_repository(
    ctx: &RequestContext,
    repository: &dyn AlertRepository,
    id: &str,
) -> Result<AlertRecord, AppError> {
    policy::require(ctx, Permission::ReadAlerts)?;
    repository
        .get(ctx.tenant_id(), id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Alert '{id}' not found")))
}

pub async fn transition_with_repository(
    ctx: &RequestContext,
    repository: &dyn AlertRepository,
    id: &str,
    transition: AlertTransition,
) -> Result<AlertRecord, AppError> {
    policy::require(ctx, Permission::ManageAlerts)?;
    match repository
        .transition(ctx.tenant_id(), id, transition)
        .await?
    {
        AlertTransitionOutcome::NotFound => {
            Err(AppError::NotFound(format!("Alert '{id}' not found")))
        }
        AlertTransitionOutcome::InvalidStatus(status) => {
            let message = match transition {
                AlertTransition::Acknowledge => {
                    format!("Alert '{id}' cannot be acknowledged from status '{status}'")
                }
                AlertTransition::Resolve => format!("Alert '{id}' is already resolved"),
                AlertTransition::Reactivate => format!("Alert '{id}' is already active"),
            };
            Err(AppError::BadRequest(message))
        }
        AlertTransitionOutcome::Updated(alert) => Ok(*alert),
    }
}

pub async fn transition_many_with_repository(
    ctx: &RequestContext,
    repository: &dyn AlertRepository,
    ids: Vec<String>,
    transition: AlertTransition,
) -> Result<Vec<AlertRecord>, AppError> {
    policy::require(ctx, Permission::ManageAlerts)?;
    Ok(repository
        .transition_many(ctx.tenant_id(), ids, transition)
        .await?)
}

pub async fn summary_with_repository(
    ctx: &RequestContext,
    repository: &dyn AlertRepository,
) -> Result<Vec<(String, String, i64)>, AppError> {
    policy::require(ctx, Permission::ReadAlerts)?;
    Ok(repository.summary(ctx.tenant_id()).await?)
}

pub async fn persist_cooldowns_with_repository(
    repository: &dyn AlertRepository,
    cooldowns: Vec<CooldownRecord>,
) -> Result<(), AppError> {
    Ok(repository.persist_cooldowns(cooldowns).await?)
}
