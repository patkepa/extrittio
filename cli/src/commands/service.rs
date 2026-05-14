use anyhow::{Context, Result};
use extrittio_backend::{app, config::AppConfig, init as backend_init};
use serde::Serialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    args::{DatabaseArgs, InitArgs, ServeArgs, ServiceConfigArgs},
    output::{OutputFormat, output},
};

pub(crate) async fn serve(args: ServeArgs) -> Result<()> {
    load_dotenv();
    init_tracing();

    let config = app_config(args.config);
    info!("Starting extrittio on port {}", config.port);

    let state = app::boot::initialize_state(&config).await?;
    app::workers::spawn_background_tasks(&config, state.clone());
    app::http::serve(&config, state).await
}

pub(crate) fn migrate(args: DatabaseArgs, output_format: OutputFormat) -> Result<()> {
    load_dotenv();
    init_tracing();

    let config = app_config(ServiceConfigArgs {
        database: args,
        port: None,
        public_url: None,
        cors_origin: None,
        certs_dir: None,
        zenoh_tls_enabled: None,
        zenoh_tls_port: None,
        offline_timeout_secs: None,
        command_timeout_secs: None,
        max_firmware_size_mb: None,
        alert_retention_days: None,
        telemetry_retention_days: None,
    });

    let pool = backend_init::create_db_pool(&config.database_url, config.db_pool_size)?;
    let mut conn = pool
        .get()
        .context("Failed to get DB connection for migrations")?;
    backend_init::run_migrations(&mut conn)?;

    let result = ServiceCommandResult {
        status: "ok",
        database_url: redact_database_url(&config.database_url),
    };
    output(output_format, &result, || {
        format!("Database migrations completed ({})", result.database_url)
    })
}

pub(crate) fn init(args: InitArgs, output_format: OutputFormat) -> Result<()> {
    load_dotenv();
    init_tracing();

    let mut config = AppConfig::from_env();
    apply_database_args(&mut config, args.database);
    if let Some(certs_dir) = args.certs_dir {
        config.certs_dir = certs_dir;
    }
    let pool = backend_init::create_db_pool(&config.database_url, config.db_pool_size)?;
    let mut conn = pool
        .get()
        .context("Failed to get DB connection for initialization")?;

    backend_init::run_migrations(&mut conn)?;
    backend_init::seed_default_device_types(&mut conn)?;
    backend_init::init_jwt_secret(&mut conn)?;
    backend_init::seed_admin_user(&mut conn)?;
    backend_init::init_ca_certificate(&mut conn)?;
    backend_init::write_tls_certs(&mut conn, &config.certs_dir)?;

    let result = InitCommandResult {
        status: "ok",
        database_url: redact_database_url(&config.database_url),
        certs_dir: config.certs_dir,
    };
    output(output_format, &result, || {
        format!(
            "Service initialized ({}, certs_dir={})",
            result.database_url, result.certs_dir
        )
    })
}

fn app_config(args: ServiceConfigArgs) -> AppConfig {
    let mut config = AppConfig::from_env();

    apply_database_args(&mut config, args.database);
    if let Some(port) = args.port {
        config.port = port;
    }
    if let Some(public_url) = args.public_url {
        config.public_url = public_url.trim_end_matches('/').to_string();
    }
    if let Some(cors_origin) = args.cors_origin {
        config.allowed_origin = cors_origin;
    }
    if let Some(certs_dir) = args.certs_dir {
        config.certs_dir = certs_dir;
    }
    if let Some(zenoh_tls_enabled) = args.zenoh_tls_enabled {
        config.zenoh_tls_enabled = zenoh_tls_enabled;
    }
    if let Some(zenoh_tls_port) = args.zenoh_tls_port {
        config.zenoh_tls_port = zenoh_tls_port;
    }
    if let Some(offline_timeout_secs) = args.offline_timeout_secs {
        config.offline_timeout_secs = offline_timeout_secs;
    }
    if let Some(command_timeout_secs) = args.command_timeout_secs {
        config.command_timeout_secs = command_timeout_secs;
    }
    if let Some(max_firmware_size_mb) = args.max_firmware_size_mb {
        config.max_firmware_size_bytes = max_firmware_size_mb * 1024 * 1024;
    }
    if let Some(alert_retention_days) = args.alert_retention_days {
        config.alert_retention_days = alert_retention_days;
    }
    if let Some(telemetry_retention_days) = args.telemetry_retention_days {
        config.telemetry_retention_days = telemetry_retention_days;
    }

    config
}

fn apply_database_args(config: &mut AppConfig, args: DatabaseArgs) {
    if let Some(database_url) = args.database_url {
        config.database_url = database_url;
    }
    if let Some(db_pool_size) = args.db_pool_size {
        config.db_pool_size = db_pool_size;
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("extrittio=info,extrittio_backend=info")),
        )
        .try_init();
}

fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

fn redact_database_url(database_url: &str) -> String {
    let Some((scheme, rest)) = database_url.split_once("://") else {
        return database_url.to_string();
    };
    let Some((userinfo, host)) = rest.split_once('@') else {
        return database_url.to_string();
    };
    let Some((username, _password)) = userinfo.split_once(':') else {
        return database_url.to_string();
    };
    format!("{scheme}://{username}:***@{host}")
}

#[derive(Serialize)]
struct ServiceCommandResult {
    status: &'static str,
    database_url: String,
}

#[derive(Serialize)]
struct InitCommandResult {
    status: &'static str,
    database_url: String,
    certs_dir: String,
}
