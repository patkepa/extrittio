use std::env;

pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub zenoh_connect: Vec<String>,
    pub offline_timeout_secs: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "extrittio.db".to_string()),
            zenoh_connect: env::var("ZENOH_CONNECT")
                .ok()
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            offline_timeout_secs: env::var("OFFLINE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
        }
    }
}
