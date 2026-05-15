use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::tenancy::DEFAULT_TENANT_ID;

pub mod context;
pub mod policy;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,
    pub username: String,
    pub role: String,
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_permission_version")]
    pub permission_version: i32,
    pub exp: usize,
}

fn default_permission_version() -> i32 {
    1
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn create_token(
    user_id: i32,
    username: &str,
    role: &str,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    create_token_with_scopes(
        user_id,
        username,
        role,
        DEFAULT_TENANT_ID,
        Vec::new(),
        default_permission_version(),
        secret,
    )
}

pub fn create_token_with_scopes(
    user_id: i32,
    username: &str,
    role: &str,
    tenant_id: &str,
    scopes: Vec<String>,
    permission_version: i32,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        role: role.to_string(),
        tenant_id: Some(tenant_id.to_string()),
        scopes,
        permission_version,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn validate_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}
