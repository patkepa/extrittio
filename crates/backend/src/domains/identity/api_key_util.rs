use rand::Rng;
use sha2::{Digest, Sha256};

pub struct RandomApiKeyGenerator;

impl extrittio_backend_core::ApiKeyGenerator for RandomApiKeyGenerator {
    fn generate(&self) -> extrittio_backend_core::GeneratedApiKey {
        let plaintext = generate_api_key();
        extrittio_backend_core::GeneratedApiKey {
            hash: hash_api_key(&plaintext),
            prefix: key_prefix(&plaintext),
            plaintext,
        }
    }
}

/// Generate a new API key with `extr_` prefix and 32 bytes of randomness.
pub fn generate_api_key() -> String {
    let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().r#gen()).collect();
    let hex: String = random_bytes.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    });
    format!("extr_{hex}")
}

/// Hash an API key using SHA-256 for storage/lookup.
pub fn hash_api_key(key: &str) -> String {
    let hash = Sha256::digest(key.as_bytes());
    hash.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// Extract the display prefix from an API key (extr_ + first 8 random chars).
pub fn key_prefix(key: &str) -> String {
    key.chars().take(13).collect()
}
