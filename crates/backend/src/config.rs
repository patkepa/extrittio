use std::{env, path::PathBuf, time::Duration};

use crate::persistence::BackendKind;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid value for {key}: `{value}` ({reason})")]
    InvalidValue {
        key: String,
        value: String,
        reason: String,
    },
    #[error("invalid configuration: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirmwareStorageConfig {
    Local {
        path: PathBuf,
    },
    S3 {
        bucket: String,
        region: String,
        endpoint: Option<String>,
        allow_http: bool,
        virtual_hosted_style: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentProfile {
    Production,
    Development,
    Hobby,
}

impl DeploymentProfile {
    fn parse(value: &str) -> Result<Self, ConfigError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "production" => Ok(Self::Production),
            "development" => Ok(Self::Development),
            "hobby" => Ok(Self::Hobby),
            _ => Err(ConfigError::InvalidValue {
                key: "EXTRITTIO_DEPLOYMENT_PROFILE".to_string(),
                value: value.to_string(),
                reason: "expected production, development, or hobby".to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseConfig {
    Postgres {
        url: String,
        pool_size: u32,
    },
    Turso {
        data_dir: PathBuf,
        database_path: PathBuf,
        busy_timeout: Duration,
        size_warning_bytes: u64,
    },
}

impl DatabaseConfig {
    #[must_use]
    pub const fn kind(&self) -> BackendKind {
        match self {
            Self::Postgres { .. } => BackendKind::Postgres,
            Self::Turso { .. } => BackendKind::Turso,
        }
    }
}

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
const DEFAULT_OUTBOX_BATCH_SIZE: i64 = 64;
const DEFAULT_OUTBOX_CONCURRENCY: usize = 8;
const DEFAULT_OUTBOX_IDLE_INTERVAL_MS: u64 = 500;
const DEFAULT_OUTBOX_LEASE_TIMEOUT_SECS: u64 = 60;

pub struct AppConfig {
    pub deployment_profile: DeploymentProfile,
    pub database: DatabaseConfig,
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
    pub firmware_storage: FirmwareStorageConfig,
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
    pub outbox_batch_size: i64,
    pub outbox_concurrency: usize,
    pub outbox_idle_interval_ms: u64,
    pub outbox_lease_timeout_secs: u64,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_env_reader(|key| env::var(key).ok())
    }

    fn from_env_reader<F>(read_env: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let rpi_mode = env_bool(&read_env, "RPI_MODE")?.unwrap_or(false);
        let deployment_profile = DeploymentProfile::parse(
            &read_env("EXTRITTIO_DEPLOYMENT_PROFILE").unwrap_or_else(|| {
                if cfg!(all(feature = "turso", not(feature = "postgres"))) {
                    "hobby".to_string()
                } else {
                    "development".to_string()
                }
            }),
        )?;
        let backend = parse_database_backend(read_env("EXTRITTIO_DATABASE_BACKEND"))?;
        let database_url = read_env("DATABASE_URL")
            .unwrap_or_else(|| "postgres://extrittio:extrittio@localhost/extrittio".to_string());
        let db_pool_size = env_parse(&read_env, "DB_POOL_SIZE")?.unwrap_or(if rpi_mode {
            RPI_DB_POOL_SIZE
        } else {
            DEFAULT_DB_POOL_SIZE
        });
        let data_dir =
            PathBuf::from(read_env("EXTRITTIO_DATA_DIR").unwrap_or_else(|| "./data".to_string()));
        let database = match backend {
            BackendKind::Postgres => DatabaseConfig::Postgres {
                url: database_url.clone(),
                pool_size: db_pool_size,
            },
            BackendKind::Turso => DatabaseConfig::Turso {
                database_path: read_env("EXTRITTIO_TURSO_DATABASE_PATH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| data_dir.join("extrittio.db")),
                data_dir,
                busy_timeout: Duration::from_millis(
                    env_parse(&read_env, "EXTRITTIO_TURSO_BUSY_TIMEOUT_MS")?.unwrap_or(5_000),
                ),
                size_warning_bytes: env_parse(&read_env, "EXTRITTIO_TURSO_SIZE_WARNING_BYTES")?
                    .unwrap_or(5 * 1024 * 1024 * 1024),
            },
        };
        let max_firmware_mb: usize = env_parse(&read_env, "MAX_FIRMWARE_SIZE_MB")?.unwrap_or(100);
        let port = env_parse(&read_env, "PORT")?.unwrap_or(8080);
        let public_url = read_env("EXTRITTIO_PUBLIC_URL")
            .unwrap_or_else(|| format!("http://localhost:{port}"))
            .trim_end_matches('/')
            .to_string();
        let zenoh_tls_enabled = env_bool(&read_env, "ZENOH_TLS_ENABLED")?.unwrap_or(false);
        let zenoh_cert_acl_enabled =
            env_bool(&read_env, "EXTRITTIO_ZENOH_CERT_ACL_ENABLED")?.unwrap_or(zenoh_tls_enabled);
        let cookie_secure = env_bool(&read_env, "EXTRITTIO_COOKIE_SECURE")?
            .unwrap_or_else(|| public_url.starts_with("https://"));
        let max_zenoh_payload_kb: usize =
            env_parse(&read_env, "ZENOH_MAX_PAYLOAD_KB")?.unwrap_or(DEFAULT_ZENOH_MAX_PAYLOAD_KB);
        let max_firmware_size_bytes =
            max_firmware_mb.checked_mul(1024 * 1024).ok_or_else(|| {
                ConfigError::Validation("MAX_FIRMWARE_SIZE_MB overflows address space".to_string())
            })?;
        let max_zenoh_payload_size_bytes =
            max_zenoh_payload_kb.checked_mul(1024).ok_or_else(|| {
                ConfigError::Validation("ZENOH_MAX_PAYLOAD_KB overflows address space".to_string())
            })?;
        let default_firmware_path = match &database {
            DatabaseConfig::Turso { data_dir, .. } => data_dir.join("firmware"),
            DatabaseConfig::Postgres { .. } => PathBuf::from("./data/firmware"),
        };
        let firmware_storage = match read_env("FIRMWARE_STORAGE_BACKEND")
            .unwrap_or_else(|| "local".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "local" => FirmwareStorageConfig::Local {
                path: read_env("FIRMWARE_STORAGE_PATH")
                    .map(PathBuf::from)
                    .unwrap_or(default_firmware_path),
            },
            "s3" => FirmwareStorageConfig::S3 {
                bucket: read_env("FIRMWARE_S3_BUCKET")
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        ConfigError::Validation(
                            "FIRMWARE_S3_BUCKET is required when FIRMWARE_STORAGE_BACKEND=s3"
                                .to_string(),
                        )
                    })?,
                region: read_env("FIRMWARE_S3_REGION").unwrap_or_else(|| "us-east-1".to_string()),
                endpoint: read_env("FIRMWARE_S3_ENDPOINT")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                allow_http: env_bool(&read_env, "FIRMWARE_S3_ALLOW_HTTP")?.unwrap_or(false),
                virtual_hosted_style: env_bool(&read_env, "FIRMWARE_S3_VIRTUAL_HOSTED_STYLE")?
                    .unwrap_or(false),
            },
            value => {
                return Err(ConfigError::InvalidValue {
                    key: "FIRMWARE_STORAGE_BACKEND".to_string(),
                    value: value.to_string(),
                    reason: "expected local or s3".to_string(),
                });
            }
        };
        let default_certs_dir = match &database {
            DatabaseConfig::Turso { data_dir, .. } => data_dir.join("certs").display().to_string(),
            DatabaseConfig::Postgres { .. } => "./certs".to_string(),
        };

        let config = Self {
            deployment_profile,
            database,
            rpi_mode,
            port,
            public_url,
            database_url,
            allowed_origin: read_env("CORS_ORIGIN")
                .unwrap_or_else(|| "http://localhost:5173".to_string()),
            offline_timeout_secs: env_parse(&read_env, "OFFLINE_TIMEOUT_SECS")?.unwrap_or(
                if rpi_mode {
                    RPI_OFFLINE_TIMEOUT_SECS
                } else {
                    DEFAULT_OFFLINE_TIMEOUT_SECS
                },
            ),
            command_timeout_secs: env_parse(&read_env, "COMMAND_TIMEOUT_SECS")?.unwrap_or(
                if rpi_mode {
                    RPI_COMMAND_TIMEOUT_SECS
                } else {
                    DEFAULT_COMMAND_TIMEOUT_SECS
                },
            ),
            certs_dir: read_env("EXTRITTIO_CERTS_DIR").unwrap_or(default_certs_dir),
            zenoh_tls_enabled,
            zenoh_cert_acl_enabled,
            zenoh_tls_port: env_parse(&read_env, "ZENOH_TLS_PORT")?.unwrap_or(7447),
            zenoh_listen_host: read_env("ZENOH_LISTEN_HOST")
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            db_pool_size,
            max_firmware_size_bytes,
            firmware_storage,
            alert_retention_days: env_parse(&read_env, "ALERT_RETENTION_DAYS")?.unwrap_or(
                if rpi_mode {
                    RPI_ALERT_RETENTION_DAYS
                } else {
                    DEFAULT_ALERT_RETENTION_DAYS
                },
            ),
            telemetry_retention_days: env_parse(&read_env, "TELEMETRY_RETENTION_DAYS")?.unwrap_or(
                if rpi_mode {
                    RPI_TELEMETRY_RETENTION_DAYS
                } else {
                    DEFAULT_TELEMETRY_RETENTION_DAYS
                },
            ),
            log_retention_days: env_parse(&read_env, "LOG_RETENTION_DAYS")?.unwrap_or(
                if rpi_mode {
                    RPI_LOG_RETENTION_DAYS
                } else {
                    DEFAULT_LOG_RETENTION_DAYS
                },
            ),
            system_metrics_interval_secs: env_parse(&read_env, "SYSTEM_METRICS_INTERVAL_SECS")?
                .unwrap_or(if rpi_mode {
                    RPI_SYSTEM_METRICS_INTERVAL_SECS
                } else {
                    DEFAULT_SYSTEM_METRICS_INTERVAL_SECS
                }),
            app_metrics_flush_interval_secs: env_parse(
                &read_env,
                "APP_METRICS_FLUSH_INTERVAL_SECS",
            )?
            .unwrap_or(if rpi_mode {
                RPI_APP_METRICS_FLUSH_INTERVAL_SECS
            } else {
                DEFAULT_APP_METRICS_FLUSH_INTERVAL_SECS
            }),
            metrics_retention_hours: env_parse(&read_env, "METRICS_RETENTION_HOURS")?
                .unwrap_or(DEFAULT_METRICS_RETENTION_HOURS),
            serve_ui: env_bool(&read_env, "EXTRITTIO_SERVE_UI")?.unwrap_or(true),
            ui_dir: read_env("EXTRITTIO_UI_DIR").map(PathBuf::from),
            enable_api_docs: env_bool(&read_env, "EXTRITTIO_ENABLE_API_DOCS")?.unwrap_or(false),
            cookie_secure,
            health_token: read_env("EXTRITTIO_HEALTH_TOKEN")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            max_zenoh_payload_size_bytes,
            auto_register_devices: env_bool(&read_env, "EXTRITTIO_AUTO_REGISTER_DEVICES")?
                .unwrap_or(false),
            trusted_proxies: csv_env(&read_env, "EXTRITTIO_TRUSTED_PROXIES"),
            outbox_batch_size: env_parse(&read_env, "RULE_ACTION_OUTBOX_BATCH_SIZE")?
                .unwrap_or(DEFAULT_OUTBOX_BATCH_SIZE),
            outbox_concurrency: env_parse(&read_env, "RULE_ACTION_OUTBOX_CONCURRENCY")?
                .unwrap_or(DEFAULT_OUTBOX_CONCURRENCY),
            outbox_idle_interval_ms: env_parse(&read_env, "RULE_ACTION_OUTBOX_IDLE_INTERVAL_MS")?
                .unwrap_or(DEFAULT_OUTBOX_IDLE_INTERVAL_MS),
            outbox_lease_timeout_secs: env_parse(
                &read_env,
                "RULE_ACTION_OUTBOX_LEASE_TIMEOUT_SECS",
            )?
            .unwrap_or(DEFAULT_OUTBOX_LEASE_TIMEOUT_SECS),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 || self.zenoh_tls_port == 0 {
            return Err(ConfigError::Validation(
                "PORT and ZENOH_TLS_PORT must be greater than zero".to_string(),
            ));
        }
        if self.zenoh_listen_host.trim().is_empty() {
            return Err(ConfigError::Validation(
                "ZENOH_LISTEN_HOST must not be empty".to_string(),
            ));
        }
        self.validate_database()?;
        if self.allowed_origin == "*" {
            return Err(ConfigError::Validation(
                "CORS_ORIGIN='*' is not allowed".to_string(),
            ));
        }
        self.allowed_origin
            .parse::<axum::http::HeaderValue>()
            .map_err(|error| ConfigError::Validation(format!("CORS_ORIGIN is invalid: {error}")))?;
        let public_url = reqwest::Url::parse(&self.public_url).map_err(|error| {
            ConfigError::Validation(format!("EXTRITTIO_PUBLIC_URL is invalid: {error}"))
        })?;
        if !matches!(public_url.scheme(), "http" | "https") || public_url.host_str().is_none() {
            return Err(ConfigError::Validation(
                "EXTRITTIO_PUBLIC_URL must be an absolute http(s) URL".to_string(),
            ));
        }
        if self.max_firmware_size_bytes == 0 || self.max_zenoh_payload_size_bytes == 0 {
            return Err(ConfigError::Validation(
                "payload size limits must be greater than zero".to_string(),
            ));
        }
        if let FirmwareStorageConfig::S3 {
            endpoint: Some(endpoint),
            allow_http,
            ..
        } = &self.firmware_storage
        {
            let endpoint = reqwest::Url::parse(endpoint).map_err(|error| {
                ConfigError::Validation(format!("FIRMWARE_S3_ENDPOINT is invalid: {error}"))
            })?;
            if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
                return Err(ConfigError::Validation(
                    "FIRMWARE_S3_ENDPOINT must be an absolute http(s) URL".to_string(),
                ));
            }
            if endpoint.scheme() == "http" && !allow_http {
                return Err(ConfigError::Validation(
                    "FIRMWARE_S3_ALLOW_HTTP=true is required for an HTTP S3 endpoint".to_string(),
                ));
            }
        }
        if self.offline_timeout_secs == 0
            || self.command_timeout_secs == 0
            || self.system_metrics_interval_secs == 0
            || self.app_metrics_flush_interval_secs == 0
        {
            return Err(ConfigError::Validation(
                "timeouts and worker intervals must be greater than zero".to_string(),
            ));
        }
        if self.outbox_batch_size <= 0
            || self.outbox_concurrency == 0
            || self.outbox_idle_interval_ms == 0
            || self.outbox_lease_timeout_secs == 0
        {
            return Err(ConfigError::Validation(
                "rule action outbox settings must be greater than zero".to_string(),
            ));
        }
        if self.zenoh_cert_acl_enabled && !self.zenoh_tls_enabled {
            return Err(ConfigError::Validation(
                "EXTRITTIO_ZENOH_CERT_ACL_ENABLED requires ZENOH_TLS_ENABLED".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_database(&self) -> Result<(), ConfigError> {
        match &self.database {
            DatabaseConfig::Postgres { url, pool_size } => {
                if !cfg!(feature = "postgres") {
                    return Err(ConfigError::Validation(
                        "PostgreSQL was requested but this binary was built without the `postgres` feature"
                            .to_string(),
                    ));
                }
                if *pool_size == 0 {
                    return Err(ConfigError::Validation(
                        "DB_POOL_SIZE must be greater than zero".to_string(),
                    ));
                }
                let parsed = reqwest::Url::parse(url).map_err(|error| {
                    ConfigError::Validation(format!("DATABASE_URL is invalid: {error}"))
                })?;
                if !matches!(parsed.scheme(), "postgres" | "postgresql") {
                    return Err(ConfigError::Validation(
                        "DATABASE_URL must use the postgres or postgresql scheme".to_string(),
                    ));
                }
            }
            DatabaseConfig::Turso {
                data_dir,
                database_path,
                busy_timeout,
                size_warning_bytes,
            } => {
                if !cfg!(feature = "turso") {
                    return Err(ConfigError::Validation(
                        "Turso was requested but this binary was built without the `turso` feature"
                            .to_string(),
                    ));
                }
                if self.deployment_profile == DeploymentProfile::Production {
                    return Err(ConfigError::Validation(
                        "production deployments require PostgreSQL; Turso is single-node hobby support"
                            .to_string(),
                    ));
                }
                if busy_timeout.is_zero() || *size_warning_bytes == 0 {
                    return Err(ConfigError::Validation(
                        "Turso busy timeout and size warning threshold must be greater than zero"
                            .to_string(),
                    ));
                }
                let data_dir = absolute_lexical(data_dir)?;
                let database_path = absolute_lexical(database_path)?;
                if !database_path.starts_with(&data_dir)
                    && !env_bool(
                        &|key| env::var(key).ok(),
                        "EXTRITTIO_ALLOW_UNSAFE_TURSO_PATH",
                    )?
                    .unwrap_or(false)
                {
                    return Err(ConfigError::Validation(
                        "EXTRITTIO_TURSO_DATABASE_PATH must be inside EXTRITTIO_DATA_DIR"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn parse_database_backend(value: Option<String>) -> Result<BackendKind, ConfigError> {
    let default = if cfg!(feature = "postgres") {
        BackendKind::Postgres
    } else {
        BackendKind::Turso
    };
    match value.map(|value| value.trim().to_ascii_lowercase()) {
        None => Ok(default),
        Some(value) if value == "postgres" => Ok(BackendKind::Postgres),
        Some(value) if value == "turso" => Ok(BackendKind::Turso),
        Some(value) => Err(ConfigError::InvalidValue {
            key: "EXTRITTIO_DATABASE_BACKEND".to_string(),
            value,
            reason: "expected postgres or turso".to_string(),
        }),
    }
}

fn absolute_lexical(path: &std::path::Path) -> Result<PathBuf, ConfigError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| ConfigError::Validation(format!("cannot resolve data path: {error}")))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
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

fn env_parse<F, T>(read_env: &F, key: &str) -> Result<Option<T>, ConfigError>
where
    F: Fn(&str) -> Option<String>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    read_env(key)
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| ConfigError::InvalidValue {
                    key: key.to_string(),
                    value,
                    reason: error.to_string(),
                })
        })
        .transpose()
}

fn env_bool<F>(read_env: &F, key: &str) -> Result<Option<bool>, ConfigError>
where
    F: Fn(&str) -> Option<String>,
{
    read_env(key)
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(ConfigError::InvalidValue {
                key: key.to_string(),
                value,
                reason: "expected true/false, yes/no, on/off, or 1/0".to_string(),
            }),
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{AppConfig, FirmwareStorageConfig};

    fn config_from(vars: &[(&str, &str)]) -> AppConfig {
        let vars: HashMap<&str, &str> = vars.iter().copied().collect();
        AppConfig::from_env_reader(|key| vars.get(key).map(|value| (*value).to_string())).unwrap()
    }

    fn config_result(vars: &[(&str, &str)]) -> Result<AppConfig, super::ConfigError> {
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
        assert_eq!(config.outbox_batch_size, 64);
        assert_eq!(config.outbox_concurrency, 8);
        assert_eq!(config.outbox_idle_interval_ms, 500);
        assert_eq!(config.outbox_lease_timeout_secs, 60);
        assert_eq!(
            config.firmware_storage,
            FirmwareStorageConfig::Local {
                path: "./data/firmware".into()
            }
        );
    }

    #[cfg(feature = "turso")]
    #[test]
    fn rejects_turso_in_production() {
        let error = config_result(&[
            ("EXTRITTIO_DATABASE_BACKEND", "turso"),
            ("EXTRITTIO_DEPLOYMENT_PROFILE", "production"),
        ])
        .err()
        .expect("production Turso must fail");
        assert!(
            error
                .to_string()
                .contains("production deployments require PostgreSQL")
        );
    }

    #[cfg(all(feature = "postgres", not(feature = "turso")))]
    #[test]
    fn rejects_backend_missing_from_the_build() {
        let error = config_result(&[("EXTRITTIO_DATABASE_BACKEND", "turso")])
            .err()
            .expect("unavailable Turso must fail");
        assert!(
            error
                .to_string()
                .contains("built without the `turso` feature")
        );
    }

    #[cfg(feature = "turso")]
    #[test]
    fn rejects_turso_database_outside_data_directory() {
        let error = config_result(&[
            ("EXTRITTIO_DATABASE_BACKEND", "turso"),
            ("EXTRITTIO_DATA_DIR", "/tmp/extrittio-data"),
            ("EXTRITTIO_TURSO_DATABASE_PATH", "/tmp/outside.db"),
        ])
        .err()
        .expect("outside path must fail");
        assert!(
            error
                .to_string()
                .contains("must be inside EXTRITTIO_DATA_DIR")
        );
    }

    #[cfg(feature = "turso")]
    #[test]
    fn turso_keeps_mutable_defaults_under_the_data_directory() {
        let config = config_from(&[
            ("EXTRITTIO_DATABASE_BACKEND", "turso"),
            ("EXTRITTIO_DEPLOYMENT_PROFILE", "hobby"),
            ("EXTRITTIO_DATA_DIR", "/tmp/extrittio-hobby"),
        ]);
        assert_eq!(config.certs_dir, "/tmp/extrittio-hobby/certs");
        assert_eq!(
            config.firmware_storage,
            FirmwareStorageConfig::Local {
                path: "/tmp/extrittio-hobby/firmware".into()
            }
        );
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

    #[test]
    fn rejects_malformed_numbers_instead_of_using_defaults() {
        let result =
            AppConfig::from_env_reader(|key| (key == "DB_POOL_SIZE").then(|| "many".to_string()));

        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_boolean_values() {
        let result =
            AppConfig::from_env_reader(|key| (key == "RPI_MODE").then(|| "sometimes".to_string()));

        assert!(result.is_err());
    }

    #[test]
    fn rejects_zero_capacity_settings() {
        let result = AppConfig::from_env_reader(|key| {
            (key == "RULE_ACTION_OUTBOX_CONCURRENCY").then(|| "0".to_string())
        });

        assert!(result.is_err());
    }

    #[test]
    fn requires_a_bucket_for_s3_firmware_storage() {
        let result = AppConfig::from_env_reader(|key| {
            (key == "FIRMWARE_STORAGE_BACKEND").then(|| "s3".to_string())
        });

        assert!(result.is_err());
    }

    #[test]
    fn rejects_insecure_s3_endpoint_without_explicit_opt_in() {
        let result = AppConfig::from_env_reader(|key| match key {
            "FIRMWARE_STORAGE_BACKEND" => Some("s3".to_string()),
            "FIRMWARE_S3_BUCKET" => Some("firmware".to_string()),
            "FIRMWARE_S3_ENDPOINT" => Some("http://minio:9000".to_string()),
            _ => None,
        });

        assert!(result.is_err());
    }
}
