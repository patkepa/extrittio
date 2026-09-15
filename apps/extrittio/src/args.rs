use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::output::OutputFormat;

#[derive(Debug, Parser)]
#[command(name = "extrittio")]
#[command(about = "Command line tooling for Extrittio IoT Hub")]
#[command(version)]
pub(crate) struct Cli {
    /// Extrittio backend URL for client and admin commands.
    #[arg(long, env = "EXTRITTIO_URL", global = true)]
    pub(crate) url: Option<String>,

    /// JWT token for client and admin commands. Defaults to EXTRITTIO_TOKEN or the saved CLI config.
    #[arg(long, env = "EXTRITTIO_TOKEN", global = true)]
    pub(crate) token: Option<String>,

    /// CLI config file path.
    #[arg(long, env = "EXTRITTIO_CLI_CONFIG", global = true)]
    pub(crate) config: Option<PathBuf>,

    /// Output format.
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    pub(crate) output: OutputFormat,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run the complete single-node Extrittio Edge stack with embedded Turso.
    Run(RunArgs),
    /// Run the Extrittio backend service.
    #[command(alias = "server")]
    Serve(ServeArgs),
    /// Run pending database migrations and exit.
    Migrate(DatabaseArgs),
    /// Initialize database seed data and service certificates, then exit.
    Init(InitArgs),
    /// Inspect and maintain an embedded Turso database.
    Database(DatabaseCommand),
    /// Authenticate and manage the local CLI session.
    Auth(AuthCommand),
    /// Show or update local CLI configuration.
    Config(ConfigCommand),
    /// Check backend liveness.
    Health,
    /// Check backend readiness, including database reachability.
    Ready,
    /// Manage devices.
    Devices(DevicesCommand),
    /// Manage device types.
    DeviceTypes(DeviceTypesCommand),
    /// Manage fleets.
    Fleets(FleetsCommand),
    /// Publish and list firmware artifacts.
    Firmware(FirmwareCommand),
    /// Deploy and inspect firmware over-the-air updates.
    Ota(OtaCommand),
    /// Manage API keys for CI and automation.
    ApiKeys(ApiKeysCommand),
    /// Download or regenerate device certificates.
    Certs(CertsCommand),
}

#[derive(Debug, Args)]
pub(crate) struct RunArgs {
    /// Directory containing the database, certificates, firmware, and backups.
    #[arg(long, env = "EXTRITTIO_DATA_DIR")]
    pub(crate) data_dir: Option<PathBuf>,

    /// Disable the embedded or on-disk web UI.
    #[arg(long)]
    pub(crate) no_ui: bool,

    /// HTTP port for the web UI and REST API.
    #[arg(long, env = "PORT", default_value_t = 8080)]
    pub(crate) port: u16,

    /// Zenoh TCP/TLS listen port used by devices.
    #[arg(long, env = "ZENOH_TLS_PORT", default_value_t = 7447)]
    pub(crate) zenoh_port: u16,

    /// Host address for the Zenoh listener. Use :: to accept Thread IPv6 clients.
    #[arg(long, env = "ZENOH_LISTEN_HOST")]
    pub(crate) zenoh_listen_host: Option<String>,

    /// Start the bundled OpenThread border-router runtime when an RCP is connected.
    #[arg(long, env = "EXTRITTIO_THREAD_ENABLED", default_value_t = true, action = clap::ArgAction::Set)]
    pub(crate) thread_enabled: bool,

    /// Require a local OpenThread RCP and border-router runtime instead of falling back to Wi-Fi-only mode.
    #[arg(long, env = "EXTRITTIO_THREAD_REQUIRED")]
    pub(crate) thread_required: bool,

    /// Serial device for the OpenThread RCP. Auto-detected when omitted.
    #[arg(long, env = "EXTRITTIO_THREAD_RCP")]
    pub(crate) thread_rcp: Option<PathBuf>,

    /// Path to the packaged or locally installed otbr-agent executable.
    #[arg(long, env = "EXTRITTIO_OTBR_AGENT")]
    pub(crate) thread_otbr_agent: Option<PathBuf>,

    /// Adjacent Ethernet or Wi-Fi interface used by the Thread border router.
    #[arg(long, env = "EXTRITTIO_THREAD_INFRA_INTERFACE")]
    pub(crate) thread_infra_interface: Option<String>,

    /// UART baud rate used by the OpenThread RCP firmware.
    #[arg(long, env = "EXTRITTIO_THREAD_RCP_BAUD", default_value_t = 460_800)]
    pub(crate) thread_rcp_baud: u32,

    /// Seed the built-in development Thread dataset when a ready RCP has no active dataset.
    /// Existing Thread datasets are never replaced.
    #[arg(
        long,
        env = "EXTRITTIO_THREAD_SEED_DEFAULT_DATASET",
        default_value_t = true,
        action = clap::ArgAction::Set
    )]
    pub(crate) thread_seed_default_dataset: bool,

    /// Initial owner username, used only when the database has no users.
    #[arg(
        long,
        env = "EXTRITTIO_BOOTSTRAP_ADMIN_USERNAME",
        default_value = "admin"
    )]
    pub(crate) admin_username: String,

    /// Initial owner password, used only when the database has no users.
    #[arg(
        long,
        env = "EXTRITTIO_BOOTSTRAP_ADMIN_PASSWORD",
        default_value = "admin",
        hide_env_values = true
    )]
    pub(crate) admin_password: String,

    /// Public URL advertised for firmware downloads.
    #[arg(long, env = "EXTRITTIO_PUBLIC_URL")]
    pub(crate) public_url: Option<String>,
}

#[derive(Debug, Args, Default)]
pub(crate) struct ServeArgs {
    #[command(flatten)]
    pub(crate) config: ServiceConfigArgs,

    /// Directory containing the built frontend assets.
    #[arg(long, env = "EXTRITTIO_UI_DIR")]
    pub(crate) ui_dir: Option<PathBuf>,

    /// Disable serving the frontend UI from this process.
    #[arg(long)]
    pub(crate) no_ui: bool,
}

#[derive(Debug, Args, Default)]
pub(crate) struct InitArgs {
    #[command(flatten)]
    pub(crate) database: DatabaseArgs,

    /// Directory where CA and server TLS certificates are stored.
    #[arg(long, env = "EXTRITTIO_CERTS_DIR")]
    pub(crate) certs_dir: Option<String>,
}

#[derive(Debug, Args, Clone, Default)]
pub(crate) struct DatabaseArgs {
    /// Database backend: postgres or turso (single-node Extrittio Edge support).
    #[arg(long, env = "EXTRITTIO_DATABASE_BACKEND")]
    pub(crate) database_backend: Option<String>,

    /// Deployment profile: production, development, or edge.
    #[arg(long, env = "EXTRITTIO_DEPLOYMENT_PROFILE")]
    pub(crate) deployment_profile: Option<String>,

    /// PostgreSQL connection URL.
    #[arg(long, env = "DATABASE_URL")]
    pub(crate) database_url: Option<String>,

    /// Maximum PostgreSQL pool size.
    #[arg(long, env = "DB_POOL_SIZE")]
    pub(crate) db_pool_size: Option<u32>,

    /// Data directory for the local Turso database and lock file.
    #[arg(long, env = "EXTRITTIO_DATA_DIR")]
    pub(crate) data_dir: Option<PathBuf>,

    /// Turso database file path; must be inside --data-dir.
    #[arg(long, env = "EXTRITTIO_TURSO_DATABASE_PATH")]
    pub(crate) turso_database_path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct DatabaseCommand {
    #[command(flatten)]
    pub(crate) database: DatabaseArgs,

    #[command(subcommand)]
    pub(crate) command: DatabaseSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DatabaseSubcommand {
    /// Show the database path, size, schema version, and integrity status.
    Info,
    /// Run the database integrity check.
    Integrity,
    /// Checkpoint and truncate the write-ahead log.
    Checkpoint,
    /// Create a checkpointed, verified backup and SHA-256 manifest.
    Backup {
        /// New backup file path. Existing files are never overwritten.
        path: PathBuf,
    },
    /// Verify a backup's integrity, schema version, and checksum manifest.
    VerifyBackup { backup: PathBuf },
    /// Restore a verified backup. The replaced database is retained beside it.
    Restore {
        backup: PathBuf,
        /// Required when the configured database already exists.
        #[arg(long)]
        force: bool,
    },
    /// Export a versioned, backend-neutral logical JSON archive.
    Export { path: PathBuf },
    /// Validate or replace database contents from a logical JSON archive.
    Import {
        path: PathBuf,
        /// Validate the complete archive without writing records.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Args, Clone, Default)]
pub(crate) struct ServiceConfigArgs {
    #[command(flatten)]
    pub(crate) database: DatabaseArgs,

    /// HTTP port for the REST API.
    #[arg(long, env = "PORT")]
    pub(crate) port: Option<u16>,

    /// Public URL advertised for downloadable artifacts.
    #[arg(long, env = "EXTRITTIO_PUBLIC_URL")]
    pub(crate) public_url: Option<String>,

    /// Allowed browser origin for CORS.
    #[arg(long, env = "CORS_ORIGIN")]
    pub(crate) cors_origin: Option<String>,

    /// Directory where CA and server TLS certificates are stored.
    #[arg(long, env = "EXTRITTIO_CERTS_DIR")]
    pub(crate) certs_dir: Option<String>,

    /// Enable Zenoh TLS/mTLS listener.
    #[arg(long, env = "ZENOH_TLS_ENABLED")]
    pub(crate) zenoh_tls_enabled: Option<bool>,

    /// Zenoh TCP/TLS listen port.
    #[arg(long, env = "ZENOH_TLS_PORT")]
    pub(crate) zenoh_tls_port: Option<u16>,

    /// Host address for the Zenoh listener. Use :: to accept Thread IPv6 clients.
    #[arg(long, env = "ZENOH_LISTEN_HOST")]
    pub(crate) zenoh_listen_host: Option<String>,

    /// Seconds before a device is marked offline.
    #[arg(long, env = "OFFLINE_TIMEOUT_SECS")]
    pub(crate) offline_timeout_secs: Option<u64>,

    /// Seconds before a pending command times out.
    #[arg(long, env = "COMMAND_TIMEOUT_SECS")]
    pub(crate) command_timeout_secs: Option<u64>,

    /// Maximum uploaded firmware size in MiB.
    #[arg(long, env = "MAX_FIRMWARE_SIZE_MB")]
    pub(crate) max_firmware_size_mb: Option<usize>,

    /// Days to keep resolved alerts.
    #[arg(long, env = "ALERT_RETENTION_DAYS")]
    pub(crate) alert_retention_days: Option<u64>,

    /// Days to keep raw telemetry.
    #[arg(long, env = "TELEMETRY_RETENTION_DAYS")]
    pub(crate) telemetry_retention_days: Option<u64>,
}

#[derive(Debug, Args)]
pub(crate) struct AuthCommand {
    #[command(subcommand)]
    pub(crate) command: AuthSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthSubcommand {
    /// Login and save the returned JWT token.
    Login(LoginArgs),
    /// Show the current authenticated user.
    Me,
    /// Remove the saved JWT token from the CLI config.
    Logout,
}

#[derive(Debug, Args)]
pub(crate) struct LoginArgs {
    #[arg(short, long)]
    pub(crate) username: String,

    #[arg(short, long, env = "EXTRITTIO_PASSWORD")]
    pub(crate) password: Option<String>,

    /// Do not save the token to the CLI config file.
    #[arg(long)]
    pub(crate) no_save: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ConfigCommand {
    #[command(subcommand)]
    pub(crate) command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigSubcommand {
    /// Print the active CLI configuration.
    Show,
    /// Save a default backend URL.
    SetUrl { url: String },
}

#[derive(Debug, Args)]
pub(crate) struct DevicesCommand {
    #[command(subcommand)]
    pub(crate) command: DevicesSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DevicesSubcommand {
    /// List devices.
    List(ListDevicesArgs),
    /// Get one device.
    Get { id: String },
    /// Create a device.
    Create(CreateDeviceArgs),
    /// Delete a device.
    Delete { id: String },
}

#[derive(Debug, Args)]
pub(crate) struct ListDevicesArgs {
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long)]
    pub(crate) search: Option<String>,
    #[arg(long)]
    pub(crate) fleet_id: Option<i32>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: i64,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: i64,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct CreateDeviceArgs {
    #[arg(short, long)]
    pub(crate) name: String,

    /// Published immutable blueprint revision assigned to the device.
    #[arg(long, value_parser = parse_non_empty_string)]
    pub(crate) blueprint_revision_id: String,

    #[arg(long)]
    pub(crate) device_type_id: Option<i32>,

    #[arg(long, value_name = "NAME")]
    pub(crate) device_type: Option<String>,

    #[arg(long)]
    pub(crate) fleet_id: Option<i32>,

    #[arg(long)]
    pub(crate) firmware: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct DeviceTypesCommand {
    #[command(subcommand)]
    pub(crate) command: DeviceTypesSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DeviceTypesSubcommand {
    /// List device types.
    List(PageArgs),
    /// Create a device type.
    Create { name: String },
    /// Delete a device type.
    Delete { id: i32 },
}

#[derive(Debug, Args)]
pub(crate) struct FleetsCommand {
    #[command(subcommand)]
    pub(crate) command: FleetsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FleetsSubcommand {
    /// List fleets.
    List(PageArgs),
    /// Create a fleet.
    Create { name: String },
    /// Delete a fleet.
    Delete { id: i32 },
}

#[derive(Debug, Args)]
pub(crate) struct FirmwareCommand {
    #[command(subcommand)]
    pub(crate) command: FirmwareSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FirmwareSubcommand {
    /// List firmware artifacts.
    List(ListFirmwareArgs),
    /// Upload a firmware binary artifact.
    Upload(UploadFirmwareArgs),
    /// Delete a firmware artifact.
    Delete { id: i32 },
}

#[derive(Debug, Args)]
pub(crate) struct ListFirmwareArgs {
    #[arg(long)]
    pub(crate) device_type_id: Option<i32>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: i64,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: i64,
}

#[derive(Debug, Args)]
pub(crate) struct UploadFirmwareArgs {
    #[arg(long)]
    pub(crate) device_type_id: i32,
    /// Published immutable blueprint revision associated with the firmware.
    #[arg(long, value_parser = parse_non_empty_string)]
    pub(crate) blueprint_revision_id: String,
    #[arg(long)]
    pub(crate) version: Option<String>,
    #[arg(long)]
    pub(crate) description: Option<String>,
    #[arg(long, value_name = "PATH")]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct OtaCommand {
    #[command(subcommand)]
    pub(crate) command: OtaSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum OtaSubcommand {
    /// Trigger OTA for one device or a filtered device set.
    Deploy(DeployOtaArgs),
    /// List OTA deployment history for a device.
    Status(OtaStatusArgs),
    /// Wait for an OTA deployment to reach a terminal state.
    Wait(OtaWaitArgs),
}

#[derive(Debug, Args)]
pub(crate) struct DeployOtaArgs {
    #[arg(long)]
    pub(crate) firmware_id: i32,
    #[arg(long)]
    pub(crate) device: Option<String>,
    #[arg(long)]
    pub(crate) fleet_id: Option<i32>,
    #[arg(long)]
    pub(crate) status: Option<String>,
    #[arg(long)]
    pub(crate) search: Option<String>,
    #[arg(long)]
    pub(crate) all: bool,
}

#[derive(Debug, Args)]
pub(crate) struct OtaStatusArgs {
    #[arg(long)]
    pub(crate) device: String,
    #[arg(long, default_value_t = 20)]
    pub(crate) limit: i64,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: i64,
}

#[derive(Debug, Args)]
pub(crate) struct OtaWaitArgs {
    #[arg(long)]
    pub(crate) device: String,
    #[arg(long)]
    pub(crate) firmware_id: i32,
    #[arg(long, default_value_t = 600)]
    pub(crate) timeout_secs: u64,
    #[arg(long, default_value_t = 2)]
    pub(crate) interval_secs: u64,
}

#[derive(Debug, Args)]
pub(crate) struct ApiKeysCommand {
    #[command(subcommand)]
    pub(crate) command: ApiKeysSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ApiKeysSubcommand {
    /// List API keys.
    List,
    /// Create an API key. The plaintext key is returned once.
    Create {
        name: String,
        #[arg(long)]
        device_type_id: Option<i32>,
    },
    /// Delete an API key.
    Delete { id: i32 },
}

#[derive(Debug, Args)]
pub(crate) struct CertsCommand {
    #[command(subcommand)]
    pub(crate) command: CertsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CertsSubcommand {
    /// Print the root CA certificate.
    Ca,
    /// Download a device certificate bundle.
    Download(CertDownloadArgs),
    /// Regenerate and download a device certificate bundle.
    Regenerate(CertDownloadArgs),
    /// Show device certificate status.
    Status { device_id: String },
}

#[derive(Debug, Args)]
pub(crate) struct CertDownloadArgs {
    pub(crate) device_id: String,

    /// Directory where ca.pem, device.pem, and device-key.pem will be written.
    #[arg(long)]
    pub(crate) out_dir: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct PageArgs {
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: i64,
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: i64,
}

fn parse_non_empty_string(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err("value cannot be blank".to_string())
    } else {
        Ok(value.to_string())
    }
}
