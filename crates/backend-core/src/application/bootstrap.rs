use crate::bootstrap::*;
use crate::{ApplicationError, PasswordHasher, TenantId, validate_password};
use std::sync::Arc;

/// Explicit system bootstrap: global first-owner semantics, not tenant self-signup.
#[derive(Clone)]
pub struct BootstrapApplication {
    repository: Arc<dyn BootstrapRepository>,
    hasher: Arc<dyn PasswordHasher>,
}
impl BootstrapApplication {
    pub fn new(repository: Arc<dyn BootstrapRepository>, hasher: Arc<dyn PasswordHasher>) -> Self {
        Self { repository, hasher }
    }
    pub async fn users_exist(&self) -> Result<bool, ApplicationError> {
        Ok(self.repository.users_exist().await?)
    }
    pub async fn seed_device_types(&self, tenant: &TenantId) -> Result<(), ApplicationError> {
        let values = [
            ("default", "cube", "#8ABBFF"),
            ("mac-device", "desktop", "#F7C948"),
            ("OrganBath", "heatmap", "#E76A6E"),
        ];
        self.repository
            .seed_builtin_device_types(
                tenant,
                values
                    .into_iter()
                    .map(|(name, icon, color_hex)| BuiltinDeviceType {
                        name: name.into(),
                        icon: icon.into(),
                        color_hex: color_hex.into(),
                    })
                    .collect(),
            )
            .await?;
        Ok(())
    }
    /// Generated material is supplied by the host; persistence selects the winner.
    pub async fn jwt_secret(&self, generated: String) -> Result<String, ApplicationError> {
        Ok(self
            .repository
            .get_or_create_server_config("jwt_secret", generated)
            .await?)
    }
    pub async fn seed_owner(
        &self,
        tenant: &TenantId,
        username: String,
        password: String,
    ) -> Result<SeedOwnerOutcome, ApplicationError> {
        let username = username.trim().to_owned();
        if username.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "Bootstrap admin username must not be empty".into(),
            ));
        }
        super::users::validate_username_characters(&username)?;
        validate_password(&password).map_err(|error| {
            ApplicationError::InvalidInput(format!("Invalid bootstrap admin password: {error}"))
        })?;
        if password == "admin" || password == username {
            return Err(ApplicationError::InvalidInput(
                "Bootstrap admin password must not be a default or match the username".into(),
            ));
        }
        self.seed_prepared_owner(tenant, username, password).await
    }
    /// Only the explicitly local, single-binary flow allows the legacy admin pair.
    pub async fn seed_local_owner(
        &self,
        tenant: &TenantId,
        username: String,
        password: String,
    ) -> Result<SeedOwnerOutcome, ApplicationError> {
        if username.trim() == "admin" && password == "admin" {
            return self
                .seed_prepared_owner(tenant, "admin".into(), password)
                .await;
        }
        self.seed_owner(tenant, username, password).await
    }
    async fn seed_prepared_owner(
        &self,
        tenant: &TenantId,
        username: String,
        password: String,
    ) -> Result<SeedOwnerOutcome, ApplicationError> {
        let password_hash = self.hasher.hash(password).await.map_err(|error| {
            ApplicationError::Internal(format!("Failed to hash default password: {error}"))
        })?;
        Ok(self
            .repository
            .seed_owner_if_empty(
                tenant,
                BootstrapOwner {
                    username,
                    password_hash,
                },
            )
            .await?)
    }
}
