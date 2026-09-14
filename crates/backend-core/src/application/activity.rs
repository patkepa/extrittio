use super::require_permission;
use crate::activity::{ActivityEventPage, ActivityQuery, ActivityRepository};
use crate::{ApplicationError, Permission, TenantContext};
#[derive(Clone)]
pub struct ActivityApplication {
    repository: std::sync::Arc<dyn ActivityRepository>,
}
const VALID_SOURCES: &[&str] = &["device", "audit", "alert", "command", "deployment"];
const VALID_SEVERITIES: &[&str] = &["debug", "info", "warning", "error"];

impl ActivityApplication {
    pub fn new(repository: std::sync::Arc<dyn ActivityRepository>) -> Self {
        Self { repository }
    }
    pub async fn list(
        &self,
        ctx: &TenantContext,
        mut query: ActivityQuery,
    ) -> Result<ActivityEventPage, ApplicationError> {
        require_permission(ctx, Permission::ReadLogs)?;

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
            return Err(ApplicationError::InvalidInput(
                "source must be one of: device, audit, alert, command, deployment".to_string(),
            ));
        }
        if query
            .severity
            .as_deref()
            .is_some_and(|severity| !VALID_SEVERITIES.contains(&severity))
        {
            return Err(ApplicationError::InvalidInput(
                "severity must be one of: debug, info, warning, error".to_string(),
            ));
        }
        if query
            .since
            .zip(query.until)
            .is_some_and(|(since, until)| since > until)
        {
            return Err(ApplicationError::InvalidInput(
                "since must be earlier than until".to_string(),
            ));
        }

        Ok(self.repository.list(ctx.tenant_id(), query).await?)
    }
}

fn normalize_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty() && value != "all")
}

fn normalize_search(value: Option<String>) -> Result<Option<String>, ApplicationError> {
    let value = value.map(|value| value.trim().to_string());
    if value.as_ref().is_some_and(|value| value.len() > 256) {
        return Err(ApplicationError::InvalidInput(
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
