use crate::auth::context::RequestContext;
use crate::auth::policy::{self, Permission};
use crate::domains::dashboard::repository::DashboardReadRepository;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardStats {
    pub total_devices: i64,
    pub active_devices: i64,
    pub offline_devices: i64,
    pub total_messages: i64,
}

pub async fn get_stats(
    ctx: &RequestContext,
    repository: &dyn DashboardReadRepository,
) -> Result<DashboardStats, AppError> {
    policy::require(ctx, Permission::ReadDevices)?;
    let counts = repository.get_summary(ctx.tenant_id()).await?;
    Ok(DashboardStats {
        total_devices: counts.total_devices,
        active_devices: counts.online_devices,
        offline_devices: counts.offline_devices,
        total_messages: counts.total_messages,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::auth::Claims;
    use crate::domains::dashboard::types::DashboardSummary;
    use crate::persistence::PersistenceError;
    use crate::tenancy::TenantId;

    struct RecordingRepository {
        tenants: Mutex<Vec<TenantId>>,
    }

    impl RecordingRepository {
        fn new() -> Self {
            Self {
                tenants: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl DashboardReadRepository for RecordingRepository {
        async fn get_summary(
            &self,
            tenant: &TenantId,
        ) -> Result<DashboardSummary, PersistenceError> {
            self.tenants.lock().unwrap().push(tenant.clone());
            Ok(DashboardSummary {
                total_devices: 4,
                online_devices: 2,
                offline_devices: 1,
                total_messages: 9,
            })
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
    async fn passes_validated_tenant_identity_to_the_repository() {
        let repository = RecordingRepository::new();

        let stats = get_stats(&context("admin", "tenant-a"), &repository)
            .await
            .unwrap();

        assert_eq!(stats.total_devices, 4);
        assert_eq!(stats.active_devices, 2);
        assert_eq!(stats.offline_devices, 1);
        assert_eq!(stats.total_messages, 9);
        assert_eq!(
            repository.tenants.lock().unwrap().as_slice(),
            &[TenantId::new("tenant-a").unwrap()]
        );
    }

    #[tokio::test]
    async fn authorization_happens_before_persistence() {
        let repository = RecordingRepository::new();

        let error = get_stats(&context("viewer", "tenant-a"), &repository)
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::Forbidden(_)));
        assert!(repository.tenants.lock().unwrap().is_empty());
    }
}
