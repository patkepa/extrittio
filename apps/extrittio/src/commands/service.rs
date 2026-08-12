use anyhow::{Context, Result};
use extrittio_backend::{
    app,
    config::{AppConfig, DatabaseConfig, DeploymentProfile, FirmwareStorageConfig},
    init as backend_init, observability,
};
use serde::Serialize;
use tracing::{info, warn};

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

pub(crate) async fn run_hobby(args: RunArgs) -> Result<()> {
    #[cfg(not(feature = "hobby"))]
    {
        let _ = args;
        anyhow::bail!(
            "`extrittio run` requires the standalone hobby build; install it with: cargo install --path apps/extrittio --locked --no-default-features --features hobby"
        )
    }
    #[cfg(feature = "hobby")]
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
        let mut thread_runtime = start_hobby_thread_router(&args)?;
        let zenoh_listen_host = args
            .zenoh_listen_host
            .clone()
            .or_else(|| thread_runtime.as_ref().map(|_| "::".to_string()));
        let mut config = app_config(ServiceConfigArgs {
            database: DatabaseArgs {
                database_backend: Some("turso".into()),
                deployment_profile: Some("hobby".into()),
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
        })?;
        config.serve_ui = true;
        config.ui_dir = None;

        let persistence = extrittio_backend::persistence::factory::create(&config.database).await?;
        backend_init::run_persistence_migrations(&persistence).await?;
        if !persistence.bootstrap.users_exist().await? {
            backend_init::seed_persistence_local_owner(
                &persistence,
                args.admin_username.clone(),
                args.admin_password.clone(),
            )
            .await?;
            eprintln!("\nExtrittio owner account created");
            eprintln!("  Username: {}", args.admin_username);
            eprintln!("  Password: {}", args.admin_password);
            eprintln!("  Change this password after signing in.");
            eprintln!();
        }
        drop(persistence);

        eprintln!("Extrittio web UI: {public_url}");
        eprintln!(
            "Data directory: {}\n",
            match &config.database {
                DatabaseConfig::Turso { data_dir, .. } => data_dir.display().to_string(),
                DatabaseConfig::Postgres { .. } => unreachable!(),
            }
        );
        let thread_controller = thread_runtime
            .as_ref()
            .and_then(|runtime| runtime.controller.clone());
        let result = serve_config(config, thread_controller).await;
        if let Some(runtime) = thread_runtime.as_mut() {
            let router = &mut runtime.router;
            if let Err(error) = router.shutdown() {
                warn!(%error, "OpenThread border-router shutdown failed");
            }
        }
        result
    }
}

#[cfg(feature = "hobby")]
struct HobbyThreadRuntime {
    router: extrittio_openthread_runtime::BorderRouter,
    controller: Option<std::sync::Arc<extrittio_openthread_runtime::ThreadController>>,
}

#[cfg(feature = "hobby")]
fn start_hobby_thread_router(args: &RunArgs) -> Result<Option<HobbyThreadRuntime>> {
    use extrittio_openthread_runtime::{
        BorderRouter, BorderRouterConfig, ThreadController, default_infrastructure_interface,
        discover_agent, discover_rcp,
    };

    if !args.thread_enabled {
        info!("OpenThread hobby runtime disabled");
        return Ok(None);
    }

    let rcp_discovery = match &args.thread_rcp {
        Some(device) => Ok(Some(device.clone())),
        None => discover_rcp(),
    };
    let rcp_device = match rcp_discovery {
        Ok(Some(device)) => device,
        Ok(None) if args.thread_required => anyhow::bail!(
            "OpenThread is required but no unique RCP was found; pass --thread-rcp <serial-device>"
        ),
        Ok(None) => {
            info!("No OpenThread RCP detected; continuing in Wi-Fi-only hobby mode");
            return Ok(None);
        }
        Err(error) if args.thread_required || args.thread_rcp.is_some() => {
            return Err(error.context("Failed to discover the OpenThread RCP"));
        }
        Err(error) => {
            warn!(%error, "Unable to discover an OpenThread RCP; continuing in Wi-Fi-only hobby mode");
            return Ok(None);
        }
    };

    let agent_path = match discover_agent(args.thread_otbr_agent.as_deref())? {
        Some(path) => path,
        None if args.thread_required || args.thread_rcp.is_some() => anyhow::bail!(
            "OpenThread RCP found at {} but bundled otbr-agent is unavailable. Set --thread-otbr-agent or EXTRITTIO_OTBR_AGENT while developing from Cargo.",
            rcp_device.display()
        ),
        None => {
            warn!(
                rcp = %rcp_device.display(),
                "OpenThread RCP detected but otbr-agent is unavailable; continuing in Wi-Fi-only hobby mode"
            );
            return Ok(None);
        }
    };

    let config = BorderRouterConfig {
        agent_path: agent_path.clone(),
        rcp_device,
        baud_rate: args.thread_rcp_baud,
        thread_interface: "wpan0".to_string(),
        infrastructure_interface: args
            .thread_infra_interface
            .clone()
            .unwrap_or_else(|| default_infrastructure_interface().to_string()),
    };
    let controller = ThreadController::discover(&agent_path, None)?;
    if controller.is_none() {
        warn!(
            "OpenThread controller tool is unavailable; rebuild with `make hobby` to enable Thread settings"
        );
    }
    let router = BorderRouter::start(config)?;
    Ok(Some(HobbyThreadRuntime {
        router,
        controller: controller.map(std::sync::Arc::new),
    }))
}

async fn serve_config(
    config: AppConfig,
    thread_controller: Option<std::sync::Arc<extrittio_openthread_runtime::ThreadController>>,
) -> Result<()> {
    let observability = observability::init("extrittio", "extrittio=info,extrittio_backend=info")?;
    info!("Starting extrittio on port {}", config.port);

    let state = app::boot::initialize_state(&config, thread_controller).await?;
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
        zenoh_listen_host: None,
        offline_timeout_secs: None,
        command_timeout_secs: None,
        max_firmware_size_mb: None,
        alert_retention_days: None,
        telemetry_retention_days: None,
    })?;

    let persistence = extrittio_backend::persistence::factory::create(&config.database).await?;
    backend_init::run_persistence_migrations(&persistence).await?;

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
    let persistence = extrittio_backend::persistence::factory::create(&config.database).await?;
    backend_init::run_persistence_migrations(&persistence).await?;
    backend_init::seed_persistence_device_types(&persistence).await?;
    backend_init::init_persistence_jwt_secret(&persistence).await?;
    backend_init::seed_persistence_admin_user(&persistence).await?;
    backend_init::init_persistence_ca_certificate(&persistence).await?;
    backend_init::write_persistence_tls_certs(&persistence, &config.certs_dir).await?;

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
    })?;
    let DatabaseConfig::Turso {
        data_dir,
        database_path,
        busy_timeout,
        ..
    } = config.database
    else {
        anyhow::bail!("database maintenance commands require --database-backend turso")
    };

    #[cfg(feature = "turso")]
    {
        use extrittio_backend::persistence::turso::TursoDatabase;

        if let DatabaseSubcommand::VerifyBackup { backup } = &args.command {
            let result = TursoDatabase::verify_backup(backup).await?;
            return output(output_format, &result, || {
                format!("Backup verified ({})", result.path.display())
            });
        }
        if let DatabaseSubcommand::Restore { backup, force } = &args.command {
            let result =
                TursoDatabase::restore_backup(&data_dir, &database_path, backup, *force).await?;
            return output(output_format, &result, || {
                format!("Database restored ({})", result.path.display())
            });
        }

        let database = TursoDatabase::open(&data_dir, &database_path, busy_timeout).await?;
        database.migrate().await?;
        match args.command {
            DatabaseSubcommand::Info => {
                let result = database.info().await?;
                output(output_format, &result, || {
                    format!(
                        "Turso database: {} ({} bytes, schema {}, integrity {})",
                        result.path.display(),
                        result.size_bytes,
                        result.schema_version,
                        result.integrity
                    )
                })
            }
            DatabaseSubcommand::Integrity => {
                database.integrity_check().await?;
                output(output_format, &StatusResult { status: "ok" }, || {
                    "Database integrity check passed".to_string()
                })
            }
            DatabaseSubcommand::Checkpoint => {
                database.checkpoint().await?;
                output(output_format, &StatusResult { status: "ok" }, || {
                    "Database checkpoint completed".to_string()
                })
            }
            DatabaseSubcommand::Backup { path } => {
                let result = database.backup(&path).await?;
                output(output_format, &result, || {
                    format!("Database backup created ({})", result.path.display())
                })
            }
            DatabaseSubcommand::Export { path } => {
                let result = database.export_logical(&path).await?;
                output(output_format, &result, || {
                    format!("Logical archive exported ({})", result.path.display())
                })
            }
            DatabaseSubcommand::Import { path, dry_run } => {
                let result = database.import_logical(&path, dry_run).await?;
                output(output_format, &result, || {
                    if dry_run {
                        format!("Logical archive validated ({})", result.path.display())
                    } else {
                        format!("Logical archive imported ({})", result.path.display())
                    }
                })
            }
            DatabaseSubcommand::VerifyBackup { .. } => unreachable!(),
            DatabaseSubcommand::Restore { .. } => unreachable!(),
        }
    }
    #[cfg(not(feature = "turso"))]
    {
        let _ = (
            data_dir,
            database_path,
            busy_timeout,
            args.command,
            output_format,
        );
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
            "hobby" => DeploymentProfile::Hobby,
            _ => anyhow::bail!("--deployment-profile must be production, development, or hobby"),
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
