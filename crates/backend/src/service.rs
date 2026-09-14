//! Host operations for command-line provisioning and database initialization.
//! The process shell supplies configuration and formats results; it never sees ports.
use crate::config::{AppConfig, DatabaseConfig};

/// Prepared host runtime; owns HTTP configuration, state, and supervised workers.
pub struct Service {
    config: AppConfig,
    state: std::sync::Arc<crate::state::AppState>,
    supervisor: crate::app::workers::WorkerSupervisor,
}

impl Service {
    pub async fn start(config: AppConfig, thread: Option<ThreadHandle>) -> anyhow::Result<Self> {
        let state = crate::app::boot::initialize_state(&config, thread).await?;
        let supervisor = crate::app::workers::spawn_background_tasks(&config, state.clone());
        Ok(Self {
            config,
            state,
            supervisor,
        })
    }

    /// Drain workers and checkpoint storage even when HTTP serving fails.
    pub async fn run(self) -> anyhow::Result<()> {
        let server_result = crate::app::http::serve(
            &self.config,
            self.state,
            self.supervisor.cancellation_token(),
        )
        .await;
        let worker_result = self.supervisor.shutdown().await;
        server_result?;
        worker_result
    }
}

pub async fn migrate(config: &DatabaseConfig) -> anyhow::Result<()> {
    let database = crate::persistence::factory::create(config).await?;
    crate::init::run_database_migrations(database.runtime()).await
}

pub async fn initialize(config: &AppConfig) -> anyhow::Result<()> {
    let database = crate::persistence::factory::create(&config.database).await?;
    crate::init::run_database_migrations(database.runtime()).await?;
    let repositories = database.repositories();
    crate::init::seed_persistence_device_types(repositories).await?;
    crate::init::init_persistence_jwt_secret(repositories).await?;
    crate::init::seed_persistence_admin_user(repositories).await?;
    crate::init::init_persistence_ca_certificate(repositories).await?;
    crate::init::write_persistence_tls_certs(repositories, &config.certs_dir).await
}

/// Preserve the edge command's existing check-then-seed flow and output decision.
/// The bootstrap application retains responsibility for concurrent creation.
pub async fn provision_local_owner(
    config: &DatabaseConfig,
    username: String,
    password: String,
) -> anyhow::Result<bool> {
    let database = crate::persistence::factory::create(config).await?;
    crate::init::run_database_migrations(database.runtime()).await?;
    let repositories = database.repositories();
    if crate::init::bootstrap_application(repositories)
        .users_exist()
        .await?
    {
        return Ok(false);
    }
    crate::init::seed_persistence_local_owner(repositories, username, password).await?;
    Ok(true)
}

#[cfg(feature = "turso")]
pub mod maintenance {
    use crate::config::DatabaseConfig;
    use crate::database::{LogicalArchiveInfo, TursoBackupInfo, TursoDatabase, TursoDatabaseInfo};
    use std::path::PathBuf;

    pub enum Action {
        Info,
        Integrity,
        Checkpoint,
        Backup(PathBuf),
        VerifyBackup(PathBuf),
        Restore { backup: PathBuf, force: bool },
        Export(PathBuf),
        Import { path: PathBuf, dry_run: bool },
    }
    pub enum Outcome {
        Info(TursoDatabaseInfo),
        Integrity,
        Checkpoint,
        Backup(TursoBackupInfo),
        VerifyBackup(TursoBackupInfo),
        Restore(TursoBackupInfo),
        Export(LogicalArchiveInfo),
        Import {
            info: LogicalArchiveInfo,
            dry_run: bool,
        },
    }
    pub async fn execute(config: &DatabaseConfig, action: Action) -> anyhow::Result<Outcome> {
        let DatabaseConfig::Turso {
            data_dir,
            database_path,
            busy_timeout,
            ..
        } = config
        else {
            anyhow::bail!("database maintenance commands require --database-backend turso")
        };
        // Verification and restore must not open/migrate the target beforehand.
        match action {
            Action::VerifyBackup(path) => {
                return Ok(Outcome::VerifyBackup(
                    TursoDatabase::verify_backup(&path).await?,
                ));
            }
            Action::Restore { backup, force } => {
                return Ok(Outcome::Restore(
                    TursoDatabase::restore_backup(data_dir, database_path, &backup, force).await?,
                ));
            }
            _ => {}
        }
        let database = TursoDatabase::open(data_dir, database_path, *busy_timeout).await?;
        database.migrate().await?;
        Ok(match action {
            Action::Info => Outcome::Info(database.info().await?),
            Action::Integrity => {
                database.integrity_check().await?;
                Outcome::Integrity
            }
            Action::Checkpoint => {
                database.checkpoint().await?;
                Outcome::Checkpoint
            }
            Action::Backup(path) => Outcome::Backup(database.backup(&path).await?),
            Action::Export(path) => Outcome::Export(database.export_logical(&path).await?),
            Action::Import { path, dry_run } => Outcome::Import {
                info: database.import_logical(&path, dry_run).await?,
                dry_run,
            },
            Action::VerifyBackup(_) | Action::Restore { .. } => unreachable!(),
        })
    }
}

/// Opaque host-owned Thread runtime passed from edge startup into server boot.
pub struct ThreadHandle(pub(crate) std::sync::Arc<extrittio_openthread_runtime::ThreadRuntime>);

pub struct EdgeThreadOptions {
    pub enabled: bool,
    pub required: bool,
    pub rcp_device: Option<std::path::PathBuf>,
    pub agent_path: Option<std::path::PathBuf>,
    pub baud_rate: u32,
    pub infrastructure_interface: Option<String>,
    pub data_dir: std::path::PathBuf,
}

pub fn start_edge_thread(options: EdgeThreadOptions) -> anyhow::Result<Option<ThreadHandle>> {
    use extrittio_openthread_runtime::{
        ThreadRuntime, ThreadRuntimeConfig, default_infrastructure_interface,
    };
    if !options.enabled {
        tracing::info!("Extrittio Edge OpenThread runtime disabled");
        return Ok(None);
    }
    let runtime = std::sync::Arc::new(ThreadRuntime::new(ThreadRuntimeConfig {
        rcp_device: options.rcp_device,
        agent_path: options.agent_path,
        baud_rate: options.baud_rate,
        thread_interface: "wpan0".to_string(),
        infrastructure_interface: options
            .infrastructure_interface
            .unwrap_or_else(default_infrastructure_interface),
        data_path: options.data_dir.join("thread"),
    }));
    let snapshot = runtime.refresh();
    if !snapshot.available {
        let message = snapshot
            .message
            .as_deref()
            .unwrap_or("Thread runtime is unavailable");
        if options.required {
            anyhow::bail!("OpenThread is required but unavailable: {message}");
        }
        tracing::warn!(%message, "OpenThread runtime is waiting for an RCP");
    }
    Ok(Some(ThreadHandle(runtime)))
}

pub async fn seed_default_thread_dataset(
    handle: Option<&ThreadHandle>,
    enabled: bool,
) -> anyhow::Result<bool> {
    use anyhow::Context;
    let Some(handle) = handle.filter(|handle| enabled && handle.0.snapshot().available) else {
        return Ok(false);
    };
    let runtime = handle.0.clone();
    Ok(
        tokio::task::spawn_blocking(move || runtime.ensure_default_development_network())
            .await
            .context("Thread default-dataset task failed")??,
    )
}
