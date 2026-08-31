use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use extrittio_backend_core::{
    Clock, EncodedPasswordHash, PasswordHasher as CorePasswordHasher, PasswordHasherError,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::tenancy::DEFAULT_TENANT_ID;

pub mod context;
pub mod policy;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,
    pub username: String,
    pub role: String,
    #[serde(
        default,
        deserialize_with = "deserialize_tenant_id_claim",
        skip_serializing_if = "Option::is_none"
    )]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_permission_version")]
    pub permission_version: i32,
    pub exp: usize,
}

fn deserialize_tenant_id_claim<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // `default` handles an absent field as `None`. If the field is present,
    // preserve an explicit JSON null as an invalid empty claim so it cannot be
    // confused with a legacy token that genuinely omitted the claim.
    Ok(Some(
        Option::<String>::deserialize(deserializer)?.unwrap_or_default(),
    ))
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

/// Host implementation of the core password boundary. Argon2 work stays off
/// Tokio worker threads, and owned plaintext is zeroized when the blocking
/// task completes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Argon2PasswordHasher;

#[async_trait]
impl CorePasswordHasher for Argon2PasswordHasher {
    async fn hash(
        &self,
        plaintext: String,
    ) -> Result<EncodedPasswordHash, PasswordHasherError> {
        tokio::task::spawn_blocking(move || {
            let plaintext = Zeroizing::new(plaintext);
            hash_password(&plaintext)
                .map(EncodedPasswordHash::new)
                .map_err(|error| PasswordHasherError::new(error.to_string()))
        })
        .await
        .map_err(|error| PasswordHasherError::new(format!("password task failed: {error}")))?
    }

    async fn verify(
        &self,
        plaintext: String,
        password_hash: EncodedPasswordHash,
    ) -> Result<bool, PasswordHasherError> {
        tokio::task::spawn_blocking(move || {
            let plaintext = Zeroizing::new(plaintext);
            let password_hash = Zeroizing::new(password_hash.into_inner());
            verify_password(&plaintext, &password_hash)
        })
        .await
        .map_err(|error| PasswordHasherError::new(format!("password task failed: {error}")))
    }
}

/// Production UTC clock supplied to core application use cases.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::context::{UserClaimsContextError, map_validated_user_claims};

    const SECRET: &str = "test-auth-secret-with-enough-entropy";

    #[derive(Serialize)]
    struct LegacyClaimsWithoutTenant<'a> {
        sub: i32,
        username: &'a str,
        role: &'a str,
        scopes: Vec<String>,
        permission_version: i32,
        exp: usize,
    }

    #[derive(Serialize)]
    struct ClaimsWithNullTenant<'a> {
        sub: i32,
        username: &'a str,
        role: &'a str,
        tenant_id: Option<String>,
        scopes: Vec<String>,
        permission_version: i32,
        exp: usize,
    }

    fn expiration() -> usize {
        usize::try_from((chrono::Utc::now() + chrono::Duration::hours(1)).timestamp()).unwrap()
    }

    #[test]
    fn signed_legacy_token_with_absent_tenant_uses_compatibility_mapping() {
        let token = encode(
            &Header::default(),
            &LegacyClaimsWithoutTenant {
                sub: 1,
                username: "legacy",
                role: "viewer",
                scopes: Vec::new(),
                permission_version: 1,
                exp: expiration(),
            },
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();

        let claims = validate_token(&token, SECRET).unwrap();
        let before = crate::auth::context::legacy_missing_tenant_claim_count();
        let mapped = map_validated_user_claims(claims).unwrap();

        assert_eq!(mapped.tenant_id().as_str(), "default");
        assert!(crate::auth::context::legacy_missing_tenant_claim_count() > before);
    }

    #[test]
    fn signed_token_with_explicit_null_tenant_fails_closed() {
        let token = encode(
            &Header::default(),
            &ClaimsWithNullTenant {
                sub: 1,
                username: "invalid",
                role: "viewer",
                tenant_id: None,
                scopes: Vec::new(),
                permission_version: 1,
                exp: expiration(),
            },
            &EncodingKey::from_secret(SECRET.as_bytes()),
        )
        .unwrap();

        let claims = validate_token(&token, SECRET).unwrap();

        assert!(matches!(
            map_validated_user_claims(claims),
            Err(UserClaimsContextError::InvalidTenant(_))
        ));
    }

    #[test]
    fn default_login_compatibility_token_contains_an_explicit_tenant() {
        let token = create_token(1, "owner", "owner", SECRET).unwrap();
        let claims = validate_token(&token, SECRET).unwrap();

        assert_eq!(claims.tenant_id.as_deref(), Some("default"));
    }

    #[test]
    fn verifies_a_preexisting_phc_encoded_argon2_password() {
        // Argon2's published PHC-format vector for the plaintext `password`.
        // This locks the decoder path used by credentials stored before the
        // crate extraction, independently of newly generated random salts.
        let encoded = "$argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ$CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc";

        assert!(verify_password("password", encoded));
        assert!(!verify_password("not-password", encoded));
        assert!(!verify_password("password", "not-a-phc-password-hash"));
    }

    #[tokio::test]
    async fn host_password_port_round_trips_and_uses_unique_salts() {
        let hasher = Argon2PasswordHasher;
        let first = CorePasswordHasher::hash(&hasher, "Secret123!456".to_string())
            .await
            .unwrap();
        let second = CorePasswordHasher::hash(&hasher, "Secret123!456".to_string())
            .await
            .unwrap();

        assert_ne!(first, second);
        assert!(
            CorePasswordHasher::verify(&hasher, "Secret123!456".to_string(), first)
                .await
                .unwrap()
        );
        assert!(
            !CorePasswordHasher::verify(&hasher, "Wrong123!456".to_string(), second)
                .await
                .unwrap()
        );
    }
}
