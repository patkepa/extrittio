use anyhow::{Context, Result};
use extrittio_backend::{
    config::{AppConfig, DatabaseConfig, DeploymentProfile, FirmwareStorageConfig},
    observability,
};
use serde::Serialize;
use tracing::info;

use crate::{
    args::{DatabaseArgs, DatabaseCommand, InitArgs, RunArgs, ServeArgs, ServiceConfigArgs},
    output::{OutputFormat, output},
};

#[cfg(feature = "turso")]
use crate::args::DatabaseSubcommand;

pub(crate) async fn serve(args: ServeArgs) -> Result<()> {
    load_dotenv();

    let mut config = app_config(args.config)?;
    if let Some(ui_dir) = args.ui_dir {
        config.ui_dir = Some(ui_dir);
    }
    if args.no_ui {
        config.serve_ui = false;
    }
    serve_config(config, None).await
}

pub(crate) async fn run_edge(args: RunArgs) -> Result<()> {
    #[cfg(not(feature = "edge-runtime"))]
    {
        let _ = args;
        anyhow::bail!(
            "`extrittio run` requires an Extrittio Edge build; use `cargo xtask edge run dev` or install it with: cargo install --path apps/extrittio --locked --no-default-features --features edge"
        )
    }
    #[cfg(feature = "edge-runtime")]
    {
        load_dotenv();
        let data_dir = args.data_dir.clone().unwrap_or_else(|| {
            dirs::data_local_dir()
                .map(|path| path.join("extrittio"))
                .unwrap_or_else(|| std::path::PathBuf::from("./extrittio-data"))
        });
        let public_url = args
            .public_url
            .clone()
            .unwrap_or_else(|| format!("http://localhost:{}", args.port));
        let thread_runtime = extrittio_backend::service::start_edge_thread(
            extrittio_backend::service::EdgeThreadOptions {
                enabled: args.thread_enabled,
                required: args.thread_required,
                rcp_device: args.thread_rcp.clone(),
                agent_path: args.thread_otbr_agent.clone(),
                baud_rate: args.thread_rcp_baud,
                infrastructure_interface: args.thread_infra_interface.clone(),
                data_dir: data_dir.clone(),
            },
        )?;
        let zenoh_listen_host = args
            .zenoh_listen_host
            .clone()
            .or_else(|| thread_runtime.as_ref().map(|_| "::".to_string()));
        let mut config = app_config(ServiceConfigArgs {
            database: DatabaseArgs {
                database_backend: Some("turso".into()),
                deployment_profile: Some("edge".into()),
                data_dir: Some(data_dir),
                ..Default::default()
            },
            port: Some(args.port),
            public_url: Some(public_url.clone()),
            cors_origin: Some(public_url.clone()),
            certs_dir: None,
            zenoh_tls_enabled: None,
            zenoh_tls_port: Some(args.zenoh_port),
            zenoh_listen_host,
            offline_timeout_secs: None,
            command_timeout_secs: None,
            max_firmware_size_mb: None,
            alert_retention_days: None,
            telemetry_retention_days: None,
            metric_rollup_retention_days: None,
        })?;
        config.serve_ui = !args.no_ui;
        config.ui_dir = None;

        if extrittio_backend::service::provision_local_owner(
            &config.database,
            args.admin_username.clone(),
            args.admin_password.clone(),
        )
        .await?
        {
            eprintln!("\nExtrittio owner account created");
            eprintln!("  Username: {}", args.admin_username);
            eprintln!("  Password: {}", args.admin_password);
            eprintln!("  Change this password after signing in.");
            eprintln!();
        }

        if extrittio_backend::service::seed_default_thread_dataset(
            thread_runtime.as_ref(),
            args.thread_seed_default_dataset,
        )
        .await?
        {
            eprintln!(
                "Created the built-in Thread development network (extrittio-c6-dev). \\\n+                     Use --thread-seed-default-dataset false to disable this on future empty RCPs.\n"
            );
        }

        if args.no_ui {
            eprintln!("Extrittio API: {public_url}");
        } else {
            eprintln!("Extrittio web UI: {public_url}");
        }
        eprintln!(
            "Data directory: {}\n",
            match &config.database {
                DatabaseConfig::Turso { data_dir, .. } => data_dir.display().to_string(),
                DatabaseConfig::Postgres { .. } => unreachable!(),
            }
        );
        serve_config(config, thread_runtime).await
    }
}

async fn serve_config(
    config: AppConfig,
    thread_runtime: Option<extrittio_backend::service::ThreadHandle>,
) -> Result<()> {
    let observability = observability::init("extrittio", "extrittio=info,extrittio_backend=info")?;
    info!("Starting extrittio on port {}", config.port);

    let service = extrittio_backend::service::Service::start(config, thread_runtime).await?;
    let service_result = service.run().await;
    let observability_result = observability.shutdown();
    service_result?;
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
        zenoh_listen_host: None,
        offline_timeout_secs: None,
        command_timeout_secs: None,
        max_firmware_size_mb: None,
        alert_retention_days: None,
        telemetry_retention_days: None,
        metric_rollup_retention_days: None,
    })?;

    extrittio_backend::service::migrate(&config.database).await?;

    let result = ServiceCommandResult {
        status: "ok",
        database_url: database_location(&config.database),
    };
    output(output_format, &result, || {
        format!("Database migrations completed ({})", result.database_url)
    })
}

pub(crate) async fn init(args: InitArgs, output_format: OutputFormat) -> Result<()> {
    load_dotenv();
    let _observability = observability::init("extrittio", "extrittio=info,extrittio_backend=info")?;

    let mut config = AppConfig::from_env()?;
    apply_database_args(&mut config, args.database)?;
    if let Some(certs_dir) = args.certs_dir {
        config.certs_dir = certs_dir;
    }
    config.validate()?;
    extrittio_backend::service::initialize(&config).await?;

    let result = InitCommandResult {
        status: "ok",
        database_url: database_location(&config.database),
        certs_dir: config.certs_dir,
    };
    output(output_format, &result, || {
        format!(
            "Service initialized ({}, certs_dir={})",
            result.database_url, result.certs_dir
        )
    })
}

pub(crate) async fn database(args: DatabaseCommand, output_format: OutputFormat) -> Result<()> {
    load_dotenv();
    let config = app_config(ServiceConfigArgs {
        database: args.database,
        port: None,
        public_url: None,
        cors_origin: None,
        certs_dir: None,
        zenoh_tls_enabled: None,
        zenoh_tls_port: None,
        zenoh_listen_host: None,
        offline_timeout_secs: None,
        command_timeout_secs: None,
        max_firmware_size_mb: None,
        alert_retention_days: None,
        telemetry_retention_days: None,
        metric_rollup_retention_days: None,
    })?;
    if !matches!(config.database, DatabaseConfig::Turso { .. }) {
        anyhow::bail!("database maintenance commands require --database-backend turso")
    }
    #[cfg(feature = "turso")]
    {
        use extrittio_backend::service::maintenance::{Action, Outcome};
        let action = match args.command {
            DatabaseSubcommand::Info => Action::Info,
            DatabaseSubcommand::Integrity => Action::Integrity,
            DatabaseSubcommand::Checkpoint => Action::Checkpoint,
            DatabaseSubcommand::Backup { path } => Action::Backup(path),
            DatabaseSubcommand::VerifyBackup { backup } => Action::VerifyBackup(backup),
            DatabaseSubcommand::Restore { backup, force } => Action::Restore { backup, force },
            DatabaseSubcommand::Export { path } => Action::Export(path),
            DatabaseSubcommand::Import { path, dry_run } => Action::Import { path, dry_run },
        };
        match extrittio_backend::service::maintenance::execute(&config.database, action).await? {
            Outcome::Info(result) => output(output_format, &result, || {
                format!(
                    "Turso database: {} ({} bytes, schema {}, integrity {})",
                    result.path.display(),
                    result.size_bytes,
                    result.schema_version,
                    result.integrity
                )
            }),
            Outcome::Integrity => output(output_format, &StatusResult { status: "ok" }, || {
                "Database integrity check passed".to_string()
            }),
            Outcome::Checkpoint => output(output_format, &StatusResult { status: "ok" }, || {
                "Database checkpoint completed".to_string()
            }),
            Outcome::Backup(result) => output(output_format, &result, || {
                format!("Database backup created ({})", result.path.display())
            }),
            Outcome::VerifyBackup(result) => output(output_format, &result, || {
                format!("Backup verified ({})", result.path.display())
            }),
            Outcome::Restore(result) => output(output_format, &result, || {
                format!("Database restored ({})", result.path.display())
            }),
            Outcome::Export(result) => output(output_format, &result, || {
                format!("Logical archive exported ({})", result.path.display())
            }),
            Outcome::Import {
                info: result,
                dry_run,
            } => output(output_format, &result, || {
                if dry_run {
                    format!("Logical archive validated ({})", result.path.display())
                } else {
                    format!("Logical archive imported ({})", result.path.display())
                }
            }),
        }
    }
    #[cfg(not(feature = "turso"))]
    {
        let _ = (args.command, output_format);
        anyhow::bail!("database maintenance requires a binary built with the `turso` feature")
    }
}

fn app_config(args: ServiceConfigArgs) -> Result<AppConfig> {
    let mut config = AppConfig::from_env()?;

    apply_database_args(&mut config, args.database)?;
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
    if let Some(zenoh_listen_host) = args.zenoh_listen_host {
        config.zenoh_listen_host = zenoh_listen_host;
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
    if let Some(metric_rollup_retention_days) = args.metric_rollup_retention_days {
        config.metric_rollup_retention_days = metric_rollup_retention_days;
    }

    config.validate()?;
    Ok(config)
}

fn apply_database_args(config: &mut AppConfig, args: DatabaseArgs) -> Result<()> {
    let previous_data_dir = match &config.database {
        DatabaseConfig::Turso { data_dir, .. } => Some(data_dir.clone()),
        DatabaseConfig::Postgres { .. } => None,
    };
    if let Some(profile) = args.deployment_profile {
        config.deployment_profile = match profile.trim().to_ascii_lowercase().as_str() {
            "production" => DeploymentProfile::Production,
            "development" => DeploymentProfile::Development,
            "edge" => DeploymentProfile::Edge,
            _ => anyhow::bail!("--deployment-profile must be production, development, or edge"),
        };
    }
    if let Some(database_url) = args.database_url {
        config.database_url = database_url;
    }
    if let Some(db_pool_size) = args.db_pool_size {
        config.db_pool_size = db_pool_size;
    }
    let selected_backend = args
        .database_backend
        .as_deref()
        .map(str::trim)
        .unwrap_or_else(|| config.database.kind().as_str());
    config.database = match selected_backend {
        "postgres" => DatabaseConfig::Postgres {
            url: config.database_url.clone(),
            pool_size: config.db_pool_size,
        },
        "turso" => {
            let (existing_data_dir, existing_path, busy_timeout, size_warning_bytes) =
                match &config.database {
                    DatabaseConfig::Turso {
                        data_dir,
                        database_path,
                        busy_timeout,
                        size_warning_bytes,
                    } => (
                        data_dir.clone(),
                        database_path.clone(),
                        *busy_timeout,
                        *size_warning_bytes,
                    ),
                    DatabaseConfig::Postgres { .. } => (
                        "./data".into(),
                        "./data/extrittio.db".into(),
                        std::time::Duration::from_secs(5),
                        5 * 1024 * 1024 * 1024,
                    ),
                };
            let data_dir_overridden = args.data_dir.is_some();
            let data_dir = args.data_dir.unwrap_or(existing_data_dir);
            let database_path = args.turso_database_path.unwrap_or_else(|| {
                if data_dir_overridden {
                    data_dir.join("extrittio.db")
                } else {
                    existing_path
                }
            });
            DatabaseConfig::Turso {
                data_dir,
                database_path,
                busy_timeout,
                size_warning_bytes,
            }
        }
        _ => anyhow::bail!("--database-backend must be postgres or turso"),
    };
    if let DatabaseConfig::Turso { data_dir, .. } = &config.database {
        let previous_default_certs = previous_data_dir
            .as_ref()
            .map(|path| path.join("certs").display().to_string());
        if config.certs_dir == "./certs"
            || previous_default_certs.as_deref() == Some(config.certs_dir.as_str())
        {
            config.certs_dir = data_dir.join("certs").display().to_string();
        }
        let previous_default_firmware =
            previous_data_dir.as_ref().map(|path| path.join("firmware"));
        if matches!(
            &config.firmware_storage,
            FirmwareStorageConfig::Local { path }
                if path == &std::path::PathBuf::from("./data/firmware")
                    || previous_default_firmware.as_ref() == Some(path)
        ) {
            config.firmware_storage = FirmwareStorageConfig::Local {
                path: data_dir.join("firmware"),
            };
        }
    }
    Ok(())
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

fn database_location(database: &DatabaseConfig) -> String {
    match database {
        DatabaseConfig::Postgres { url, .. } => redact_database_url(url),
        DatabaseConfig::Turso { database_path, .. } => database_path.display().to_string(),
    }
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

#[derive(Serialize)]
#[cfg(feature = "turso")]
struct StatusResult {
    status: &'static str,
}
