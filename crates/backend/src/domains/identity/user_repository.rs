use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::persistence::PersistenceError;
use crate::tenancy::TenantId;

use super::user_types::{
    CreateUserOutcome, CreateUserRecord, DeleteUserOutcome, SetUserRolesOutcome, UserCredentials,
    UserDetails, UserList,
};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn list(
        &self,
        tenant: &TenantId,
        limit: i64,
        offset: i64,
    ) -> Result<UserList, PersistenceError>;

    async fn create(
        &self,
        tenant: &TenantId,
        record: CreateUserRecord,
    ) -> Result<CreateUserOutcome, PersistenceError>;

    async fn change_password(
        &self,
        tenant: &TenantId,
        user_id: i32,
        password_hash: String,
    ) -> Result<bool, PersistenceError>;

    async fn delete(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<DeleteUserOutcome, PersistenceError>;

    async fn set_roles(
        &self,
        tenant: &TenantId,
        user_id: i32,
        role_ids: Vec<i32>,
    ) -> Result<SetUserRolesOutcome, PersistenceError>;

    async fn find_credentials_by_username(
        &self,
        tenant: &TenantId,
        username: &str,
    ) -> Result<Option<UserCredentials>, PersistenceError>;

    async fn get_details(
        &self,
        tenant: &TenantId,
        user_id: i32,
    ) -> Result<Option<UserDetails>, PersistenceError>;

    async fn record_successful_login(
        &self,
        tenant: &TenantId,
        user_id: i32,
        logged_in_at: DateTime<Utc>,
    ) -> Result<bool, PersistenceError>;
}
