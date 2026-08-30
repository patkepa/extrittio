use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::activity::repository::ActivityRepository;
use crate::domains::activity::types::{ActivityEventPage, ActivityQuery};
use crate::error::AppError;

const VALID_SOURCES: &[&str] = &["device", "audit", "alert", "command", "deployment"];
const VALID_SEVERITIES: &[&str] = &["debug", "info", "warning", "error"];

pub async fn list(
    ctx: &RequestContext,
    repository: &dyn ActivityRepository,
    mut query: ActivityQuery,
) -> Result<ActivityEventPage, AppError> {
    policy::require(ctx, Permission::ReadLogs)?;

    query.source = normalize_filter(query.source);
    query.severity = normalize_filter(query.severity);
    query.category = normalize_filter(query.category);
    query.device_id = normalize_filter(query.device_id);
    query.search = normalize_search(query.search)?;

    if query
        .source
        .as_deref()
        .is_some_and(|source| !VALID_SOURCES.contains(&source))
    {
        return Err(AppError::BadRequest(
            "source must be one of: device, audit, alert, command, deployment".to_string(),
        ));
    }
    if query
        .severity
        .as_deref()
        .is_some_and(|severity| !VALID_SEVERITIES.contains(&severity))
    {
        return Err(AppError::BadRequest(
            "severity must be one of: debug, info, warning, error".to_string(),
        ));
    }
    if query
        .since
        .zip(query.until)
        .is_some_and(|(since, until)| since > until)
    {
        return Err(AppError::BadRequest(
            "since must be earlier than until".to_string(),
        ));
    }

    Ok(repository.list(ctx.tenant_id(), query).await?)
}

fn normalize_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty() && value != "all")
}

fn normalize_search(value: Option<String>) -> Result<Option<String>, AppError> {
    let value = value.map(|value| value.trim().to_string());
    if value.as_ref().is_some_and(|value| value.len() > 256) {
        return Err(AppError::BadRequest(
            "search must be at most 256 characters".to_string(),
        ));
    }
    Ok(value.filter(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_all_and_blank_filters() {
        assert_eq!(normalize_filter(Some(" ALL ".to_string())), None);
        assert_eq!(normalize_filter(Some("  ".to_string())), None);
        assert_eq!(
            normalize_filter(Some(" Device ".to_string())),
            Some("device".to_string())
        );
    }

    #[test]
    fn rejects_oversized_search() {
        assert!(normalize_search(Some("x".repeat(257))).is_err());
    }
}
