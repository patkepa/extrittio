use std::env;

pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub allowed_origin: String,
    pub offline_timeout_secs: u64,
    pub command_timeout_secs: u64,
    pub certs_dir: String,
    pub zenoh_tls_enabled: bool,
    pub zenoh_tls_port: u16,
}

impl AppConfig {
    #[must_use] 
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "extrittio.db".to_string()),
            allowed_origin: env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".to_string()),
            offline_timeout_secs: env::var("OFFLINE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
            command_timeout_secs: env::var("COMMAND_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            certs_dir: env::var("EXTRITTIO_CERTS_DIR").unwrap_or_else(|_| "./certs".to_string()),
            zenoh_tls_enabled: env::var("ZENOH_TLS_ENABLED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            zenoh_tls_port: env::var("ZENOH_TLS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(7447),
        }
    }
}
