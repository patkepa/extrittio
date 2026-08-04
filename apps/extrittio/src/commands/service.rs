use anyhow::{Context, Result};
use extrittio_backend::{app, config::AppConfig, init as backend_init, observability};
use serde::Serialize;
use tracing::info;

use crate::{
    args::{DatabaseArgs, InitArgs, ServeArgs, ServiceConfigArgs},
    output::{OutputFormat, output},
};

pub(crate) async fn serve(args: ServeArgs) -> Result<()> {
    load_dotenv();
    let observability = observability::init("extrittio", "extrittio=info,extrittio_backend=info")?;

    let mut config = app_config(args.config)?;
    if let Some(ui_dir) = args.ui_dir {
        config.ui_dir = Some(ui_dir);
    }
    if args.no_ui {
        config.serve_ui = false;
    }
    info!("Starting extrittio on port {}", config.port);

    let state = app::boot::initialize_state(&config).await?;
    let supervisor = app::workers::spawn_background_tasks(&config, state.clone());
    let server_result = app::http::serve(&config, state, supervisor.cancellation_token()).await;
    let worker_result = supervisor.shutdown().await;
    let observability_result = observability.shutdown();
    server_result?;
    worker_result?;
    observability_result
}

pub(crate) async fn migrate(args: DatabaseArgs, output_format: OutputFormat) -> Result<()> {
    load_dotenv();
    let _observability = observability::init("extrittio", "extrittio=info,extrittio_backend=info")?;

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
    })?;

    let pool = backend_init::create_db_pool(&config.database_url, config.db_pool_size)?;
    let persistence = extrittio_backend::persistence::postgres::create_persistence(pool);
    backend_init::run_persistence_migrations(&persistence).await?;

    let result = ServiceCommandResult {
        status: "ok",
        database_url: redact_database_url(&config.database_url),
    };
    output(output_format, &result, || {
        format!("Database migrations completed ({})", result.database_url)
    })
}

pub(crate) async fn init(args: InitArgs, output_format: OutputFormat) -> Result<()> {
    load_dotenv();
    let _observability = observability::init("extrittio", "extrittio=info,extrittio_backend=info")?;

    let mut config = AppConfig::from_env()?;
    apply_database_args(&mut config, args.database);
    if let Some(certs_dir) = args.certs_dir {
        config.certs_dir = certs_dir;
    }
    let pool = backend_init::create_db_pool(&config.database_url, config.db_pool_size)?;
    let persistence = extrittio_backend::persistence::postgres::create_persistence(pool);
    backend_init::run_persistence_migrations(&persistence).await?;
    backend_init::seed_persistence_device_types(&persistence).await?;
    backend_init::init_persistence_jwt_secret(&persistence).await?;
    backend_init::seed_persistence_admin_user(&persistence).await?;
    backend_init::init_persistence_ca_certificate(&persistence).await?;
    backend_init::write_persistence_tls_certs(&persistence, &config.certs_dir).await?;

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

fn app_config(args: ServiceConfigArgs) -> Result<AppConfig> {
    let mut config = AppConfig::from_env()?;

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
        config.max_firmware_size_bytes = max_firmware_size_mb
            .checked_mul(1024 * 1024)
            .context("--max-firmware-size-mb is too large")?;
    }
    if let Some(alert_retention_days) = args.alert_retention_days {
        config.alert_retention_days = alert_retention_days;
    }
    if let Some(telemetry_retention_days) = args.telemetry_retention_days {
        config.telemetry_retention_days = telemetry_retention_days;
    }

    config.validate()?;
    Ok(config)
}

fn apply_database_args(config: &mut AppConfig, args: DatabaseArgs) {
    if let Some(database_url) = args.database_url {
        config.database_url = database_url;
    }
    if let Some(db_pool_size) = args.db_pool_size {
        config.db_pool_size = db_pool_size;
    }
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
