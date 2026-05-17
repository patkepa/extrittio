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
    pub zenoh_tls_port: u16,
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
            zenoh_tls_enabled: env_bool(&read_env, "ZENOH_TLS_ENABLED").unwrap_or(false),
            zenoh_tls_port: env_parse(&read_env, "ZENOH_TLS_PORT").unwrap_or(7447),
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
        }
    }
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
        ]);

        assert_eq!(config.db_pool_size, 2);
        assert_eq!(config.offline_timeout_secs, 900);
        assert_eq!(config.command_timeout_secs, 120);
        assert_eq!(config.telemetry_retention_days, 3);
        assert_eq!(config.log_retention_days, 2);
        assert_eq!(config.system_metrics_interval_secs, 300);
        assert_eq!(config.app_metrics_flush_interval_secs, 120);
        assert_eq!(config.metrics_retention_hours, 6);
    }
}
