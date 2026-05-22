use std::{env, path::PathBuf};

const DEFAULT_DB_POOL_SIZE: u32 = 16;
const RPI_DB_POOL_SIZE: u32 = 4;
const DEFAULT_OFFLINE_TIMEOUT_SECS: u64 = 300;
const RPI_OFFLINE_TIMEOUT_SECS: u64 = 600;
const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 30;
const RPI_COMMAND_TIMEOUT_SECS: u64 = 60;
const DEFAULT_ALERT_RETENTION_DAYS: u64 = 30;
const RPI_ALERT_RETENTION_DAYS: u64 = 7;
const DEFAULT_TELEMETRY_RETENTION_DAYS: u64 = 30;
const RPI_TELEMETRY_RETENTION_DAYS: u64 = 7;
const DEFAULT_LOG_RETENTION_DAYS: u64 = 30;
const RPI_LOG_RETENTION_DAYS: u64 = 7;
const DEFAULT_SYSTEM_METRICS_INTERVAL_SECS: u64 = 10;
const RPI_SYSTEM_METRICS_INTERVAL_SECS: u64 = 60;
const DEFAULT_APP_METRICS_FLUSH_INTERVAL_SECS: u64 = 10;
const RPI_APP_METRICS_FLUSH_INTERVAL_SECS: u64 = 60;
const DEFAULT_METRICS_RETENTION_HOURS: u64 = 24;
const DEFAULT_ZENOH_MAX_PAYLOAD_KB: usize = 256;

pub struct AppConfig {
    pub rpi_mode: bool,
    pub port: u16,
    pub public_url: String,
    pub database_url: String,
    pub allowed_origin: String,
    pub offline_timeout_secs: u64,
    pub command_timeout_secs: u64,
    pub certs_dir: String,
    pub zenoh_tls_enabled: bool,
    pub zenoh_cert_acl_enabled: bool,
    pub zenoh_tls_port: u16,
    pub zenoh_listen_host: String,
    pub db_pool_size: u32,
    pub max_firmware_size_bytes: usize,
    pub alert_retention_days: u64,
    pub telemetry_retention_days: u64,
    pub log_retention_days: u64,
    pub system_metrics_interval_secs: u64,
    pub app_metrics_flush_interval_secs: u64,
    pub metrics_retention_hours: u64,
    pub serve_ui: bool,
    pub ui_dir: Option<PathBuf>,
    pub enable_api_docs: bool,
    pub cookie_secure: bool,
    pub health_token: Option<String>,
    pub max_zenoh_payload_size_bytes: usize,
    pub auto_register_devices: bool,
    pub trusted_proxies: Vec<String>,
}

impl AppConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_reader(|key| env::var(key).ok())
    }

    fn from_env_reader<F>(read_env: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let rpi_mode = env_bool(&read_env, "RPI_MODE").unwrap_or(false);

        let max_firmware_mb: usize = env_parse(&read_env, "MAX_FIRMWARE_SIZE_MB").unwrap_or(100);

        let port = env_parse(&read_env, "PORT").unwrap_or(8080);
        let public_url = read_env("EXTRITTIO_PUBLIC_URL")
            .unwrap_or_else(|| format!("http://localhost:{port}"))
            .trim_end_matches('/')
            .to_string();
        let zenoh_tls_enabled = env_bool(&read_env, "ZENOH_TLS_ENABLED").unwrap_or(false);
        let zenoh_cert_acl_enabled =
            env_bool(&read_env, "EXTRITTIO_ZENOH_CERT_ACL_ENABLED").unwrap_or(zenoh_tls_enabled);
        let cookie_secure = env_bool(&read_env, "EXTRITTIO_COOKIE_SECURE")
            .unwrap_or_else(|| public_url.starts_with("https://"));
        let max_zenoh_payload_kb: usize =
            env_parse(&read_env, "ZENOH_MAX_PAYLOAD_KB").unwrap_or(DEFAULT_ZENOH_MAX_PAYLOAD_KB);

        Self {
            rpi_mode,
            port,
            public_url,
            database_url: read_env("DATABASE_URL").unwrap_or_else(|| {
                "postgres://extrittio:extrittio@localhost/extrittio".to_string()
            }),
            allowed_origin: read_env("CORS_ORIGIN")
                .unwrap_or_else(|| "http://localhost:5173".to_string()),
            offline_timeout_secs: env_parse(&read_env, "OFFLINE_TIMEOUT_SECS").unwrap_or(
                if rpi_mode {
                    RPI_OFFLINE_TIMEOUT_SECS
                } else {
                    DEFAULT_OFFLINE_TIMEOUT_SECS
                },
            ),
            command_timeout_secs: env_parse(&read_env, "COMMAND_TIMEOUT_SECS").unwrap_or(
                if rpi_mode {
                    RPI_COMMAND_TIMEOUT_SECS
                } else {
                    DEFAULT_COMMAND_TIMEOUT_SECS
                },
            ),
            certs_dir: read_env("EXTRITTIO_CERTS_DIR").unwrap_or_else(|| "./certs".to_string()),
            zenoh_tls_enabled,
            zenoh_cert_acl_enabled,
            zenoh_tls_port: env_parse(&read_env, "ZENOH_TLS_PORT").unwrap_or(7447),
            zenoh_listen_host: read_env("ZENOH_LISTEN_HOST")
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            db_pool_size: env_parse(&read_env, "DB_POOL_SIZE").unwrap_or(if rpi_mode {
                RPI_DB_POOL_SIZE
            } else {
                DEFAULT_DB_POOL_SIZE
            }),
            max_firmware_size_bytes: max_firmware_mb * 1024 * 1024,
            alert_retention_days: env_parse(&read_env, "ALERT_RETENTION_DAYS").unwrap_or(
                if rpi_mode {
                    RPI_ALERT_RETENTION_DAYS
                } else {
                    DEFAULT_ALERT_RETENTION_DAYS
                },
            ),
            telemetry_retention_days: env_parse(&read_env, "TELEMETRY_RETENTION_DAYS").unwrap_or(
                if rpi_mode {
                    RPI_TELEMETRY_RETENTION_DAYS
                } else {
                    DEFAULT_TELEMETRY_RETENTION_DAYS
                },
            ),
            log_retention_days: env_parse(&read_env, "LOG_RETENTION_DAYS").unwrap_or(if rpi_mode {
                RPI_LOG_RETENTION_DAYS
            } else {
                DEFAULT_LOG_RETENTION_DAYS
            }),
            system_metrics_interval_secs: env_parse(&read_env, "SYSTEM_METRICS_INTERVAL_SECS")
                .unwrap_or(if rpi_mode {
                    RPI_SYSTEM_METRICS_INTERVAL_SECS
                } else {
                    DEFAULT_SYSTEM_METRICS_INTERVAL_SECS
                }),
            app_metrics_flush_interval_secs: env_parse(
                &read_env,
                "APP_METRICS_FLUSH_INTERVAL_SECS",
            )
            .unwrap_or(if rpi_mode {
                RPI_APP_METRICS_FLUSH_INTERVAL_SECS
            } else {
                DEFAULT_APP_METRICS_FLUSH_INTERVAL_SECS
            }),
            metrics_retention_hours: env_parse(&read_env, "METRICS_RETENTION_HOURS")
                .unwrap_or(DEFAULT_METRICS_RETENTION_HOURS),
            serve_ui: env_bool(&read_env, "EXTRITTIO_SERVE_UI").unwrap_or(true),
            ui_dir: read_env("EXTRITTIO_UI_DIR").map(PathBuf::from),
            enable_api_docs: env_bool(&read_env, "EXTRITTIO_ENABLE_API_DOCS").unwrap_or(false),
            cookie_secure,
            health_token: read_env("EXTRITTIO_HEALTH_TOKEN")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            max_zenoh_payload_size_bytes: max_zenoh_payload_kb.saturating_mul(1024),
            auto_register_devices: env_bool(&read_env, "EXTRITTIO_AUTO_REGISTER_DEVICES")
                .unwrap_or(false),
            trusted_proxies: csv_env(&read_env, "EXTRITTIO_TRUSTED_PROXIES"),
        }
    }
}

fn csv_env<F>(read_env: &F, key: &str) -> Vec<String>
where
    F: Fn(&str) -> Option<String>,
{
    read_env(key)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn env_parse<F, T>(read_env: &F, key: &str) -> Option<T>
where
    F: Fn(&str) -> Option<String>,
    T: std::str::FromStr,
{
    read_env(key).and_then(|value| value.parse().ok())
}

fn env_bool<F>(read_env: &F, key: &str) -> Option<bool>
where
    F: Fn(&str) -> Option<String>,
{
    read_env(key).map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::AppConfig;

    fn config_from(vars: &[(&str, &str)]) -> AppConfig {
        let vars: HashMap<&str, &str> = vars.iter().copied().collect();
        AppConfig::from_env_reader(|key| vars.get(key).map(|value| (*value).to_string()))
    }

    #[test]
    fn uses_standard_defaults_without_rpi_mode() {
        let config = config_from(&[]);

        assert!(!config.rpi_mode);
        assert_eq!(config.db_pool_size, 16);
        assert_eq!(config.offline_timeout_secs, 300);
        assert_eq!(config.command_timeout_secs, 30);
        assert_eq!(config.alert_retention_days, 30);
        assert_eq!(config.telemetry_retention_days, 30);
        assert_eq!(config.log_retention_days, 30);
        assert_eq!(config.system_metrics_interval_secs, 10);
        assert_eq!(config.app_metrics_flush_interval_secs, 10);
        assert_eq!(config.metrics_retention_hours, 24);
        assert_eq!(config.zenoh_listen_host, "127.0.0.1");
        assert!(!config.zenoh_cert_acl_enabled);
        assert_eq!(
            config.max_zenoh_payload_size_bytes,
            super::DEFAULT_ZENOH_MAX_PAYLOAD_KB * 1024
        );
        assert_eq!(config.health_token, None);
        assert!(!config.auto_register_devices);
        assert!(config.trusted_proxies.is_empty());
    }

    #[test]
    fn rpi_mode_uses_sd_card_friendly_defaults() {
        let config = config_from(&[("RPI_MODE", "true")]);

        assert!(config.rpi_mode);
        assert_eq!(config.db_pool_size, 4);
        assert_eq!(config.offline_timeout_secs, 600);
        assert_eq!(config.command_timeout_secs, 60);
        assert_eq!(config.alert_retention_days, 7);
        assert_eq!(config.telemetry_retention_days, 7);
        assert_eq!(config.log_retention_days, 7);
        assert_eq!(config.system_metrics_interval_secs, 60);
        assert_eq!(config.app_metrics_flush_interval_secs, 60);
        assert_eq!(config.metrics_retention_hours, 24);
    }

    #[test]
    fn explicit_values_override_rpi_mode_defaults() {
        let config = config_from(&[
            ("RPI_MODE", "1"),
            ("DB_POOL_SIZE", "2"),
            ("OFFLINE_TIMEOUT_SECS", "900"),
            ("COMMAND_TIMEOUT_SECS", "120"),
            ("TELEMETRY_RETENTION_DAYS", "3"),
            ("LOG_RETENTION_DAYS", "2"),
            ("SYSTEM_METRICS_INTERVAL_SECS", "300"),
            ("APP_METRICS_FLUSH_INTERVAL_SECS", "120"),
            ("METRICS_RETENTION_HOURS", "6"),
            ("ZENOH_MAX_PAYLOAD_KB", "512"),
            ("EXTRITTIO_HEALTH_TOKEN", "ready-secret"),
            ("EXTRITTIO_AUTO_REGISTER_DEVICES", "true"),
            ("EXTRITTIO_TRUSTED_PROXIES", "127.0.0.1, 172.30.0.3"),
        ]);

        assert_eq!(config.db_pool_size, 2);
        assert_eq!(config.offline_timeout_secs, 900);
        assert_eq!(config.command_timeout_secs, 120);
        assert_eq!(config.telemetry_retention_days, 3);
        assert_eq!(config.log_retention_days, 2);
        assert_eq!(config.system_metrics_interval_secs, 300);
        assert_eq!(config.app_metrics_flush_interval_secs, 120);
        assert_eq!(config.metrics_retention_hours, 6);
        assert_eq!(config.max_zenoh_payload_size_bytes, 512 * 1024);
        assert_eq!(config.health_token.as_deref(), Some("ready-secret"));
        assert!(config.auto_register_devices);
        assert_eq!(
            config.trusted_proxies,
            vec!["127.0.0.1".to_string(), "172.30.0.3".to_string()]
        );
    }

    #[test]
    fn enables_zenoh_certificate_acl_when_tls_is_enabled() {
        let config = config_from(&[("ZENOH_TLS_ENABLED", "true")]);

        assert!(config.zenoh_tls_enabled);
        assert!(config.zenoh_cert_acl_enabled);
    }

    #[test]
    fn allows_overriding_zenoh_certificate_acl() {
        let config = config_from(&[
            ("ZENOH_TLS_ENABLED", "true"),
            ("EXTRITTIO_ZENOH_CERT_ACL_ENABLED", "false"),
        ]);

        assert!(config.zenoh_tls_enabled);
        assert!(!config.zenoh_cert_acl_enabled);
    }
}
