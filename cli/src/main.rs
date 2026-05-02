use std::{
    env,
    fmt::Write as _,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const DEFAULT_URL: &str = "http://localhost:8080";
const DEFAULT_ZENOH_CONNECT: &str = "tcp/127.0.0.1:7447";

#[derive(Debug, Parser)]
#[command(name = "extrittio")]
#[command(about = "Command line tooling for Extrittio IoT Hub")]
struct Cli {
    /// Extrittio backend URL.
    #[arg(long, env = "EXTRITTIO_URL", global = true)]
    url: Option<String>,

    /// JWT token. Defaults to EXTRITTIO_TOKEN or the saved CLI config.
    #[arg(long, env = "EXTRITTIO_TOKEN", global = true)]
    token: Option<String>,

    /// CLI config file path.
    #[arg(long, env = "EXTRITTIO_CLI_CONFIG", global = true)]
    config: Option<PathBuf>,

    /// Output format.
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table, global = true)]
    output: OutputFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Authenticate and manage the local CLI session.
    Auth(AuthCommand),
    /// Show or update local CLI configuration.
    Config(ConfigCommand),
    /// Check backend health.
    Health,
    /// Manage devices.
    Devices(DevicesCommand),
    /// Manage device types.
    DeviceTypes(DeviceTypesCommand),
    /// Manage fleets.
    Fleets(FleetsCommand),
    /// Manage API keys for CI and automation.
    ApiKeys(ApiKeysCommand),
    /// Download or regenerate device certificates.
    Certs(CertsCommand),
    /// Create a device and emit client provisioning material.
    Provision(ProvisionArgs),
}

#[derive(Debug, Args)]
struct AuthCommand {
    #[command(subcommand)]
    command: AuthSubcommand,
}

#[derive(Debug, Subcommand)]
enum AuthSubcommand {
    /// Login and save the returned JWT token.
    Login(LoginArgs),
    /// Show the current authenticated user.
    Me,
    /// Remove the saved JWT token from the CLI config.
    Logout,
}

#[derive(Debug, Args)]
struct LoginArgs {
    #[arg(short, long)]
    username: String,

    #[arg(short, long, env = "EXTRITTIO_PASSWORD")]
    password: Option<String>,

    /// Do not save the token to the CLI config file.
    #[arg(long)]
    no_save: bool,
}

#[derive(Debug, Args)]
struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
enum ConfigSubcommand {
    /// Print the active CLI configuration.
    Show,
    /// Save a default backend URL.
    SetUrl { url: String },
}

#[derive(Debug, Args)]
struct DevicesCommand {
    #[command(subcommand)]
    command: DevicesSubcommand,
}

#[derive(Debug, Subcommand)]
enum DevicesSubcommand {
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
struct ListDevicesArgs {
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    search: Option<String>,
    #[arg(long)]
    fleet_id: Option<i32>,
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
}

#[derive(Debug, Args, Clone)]
struct CreateDeviceArgs {
    #[arg(short, long)]
    name: String,

    #[arg(long)]
    device_type_id: i32,

    #[arg(long)]
    fleet_id: Option<i32>,

    #[arg(long)]
    firmware: Option<String>,
}

#[derive(Debug, Args)]
struct DeviceTypesCommand {
    #[command(subcommand)]
    command: DeviceTypesSubcommand,
}

#[derive(Debug, Subcommand)]
enum DeviceTypesSubcommand {
    /// List device types.
    List(PageArgs),
    /// Create a device type.
    Create { name: String },
    /// Delete a device type.
    Delete { id: i32 },
}

#[derive(Debug, Args)]
struct FleetsCommand {
    #[command(subcommand)]
    command: FleetsSubcommand,
}

#[derive(Debug, Subcommand)]
enum FleetsSubcommand {
    /// List fleets.
    List(PageArgs),
    /// Create a fleet.
    Create { name: String },
    /// Delete a fleet.
    Delete { id: i32 },
}

#[derive(Debug, Args)]
struct ApiKeysCommand {
    #[command(subcommand)]
    command: ApiKeysSubcommand,
}

#[derive(Debug, Subcommand)]
enum ApiKeysSubcommand {
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
struct CertsCommand {
    #[command(subcommand)]
    command: CertsSubcommand,
}

#[derive(Debug, Subcommand)]
enum CertsSubcommand {
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
struct CertDownloadArgs {
    device_id: String,

    /// Directory where ca.pem, device.pem, and device-key.pem will be written.
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

#[derive(Debug, Args, Clone)]
struct PageArgs {
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
}

#[derive(Debug, Args)]
struct ProvisionArgs {
    #[command(flatten)]
    device: CreateDeviceArgs,

    /// Zenoh endpoint to include in generated client config.
    #[arg(long, default_value = DEFAULT_ZENOH_CONNECT)]
    zenoh_connect: String,

    /// Download and write the device certificate bundle into this directory.
    #[arg(long)]
    cert_dir: Option<PathBuf>,

    /// Regenerate the device certificate before downloading it.
    #[arg(long)]
    regenerate_cert: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CliConfig {
    url: Option<String>,
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    token: String,
    user: UserResponse,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserResponse {
    id: i32,
    username: String,
    role: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Paginated<T> {
    data: Vec<T>,
    total: i64,
    limit: i64,
    offset: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeviceResponse {
    id: String,
    name: String,
    device_type_id: i32,
    device_type_name: String,
    fleet_id: Option<i32>,
    fleet_name: Option<String>,
    status: String,
    last_seen: String,
    last_seen_at: Option<String>,
    firmware: String,
    uptime: String,
    uptime_seconds: i32,
    latest_latitude: Option<f64>,
    latest_longitude: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeviceTypeResponse {
    id: i32,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FleetResponse {
    id: i32,
    name: String,
    device_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiKeyResponse {
    id: i32,
    name: String,
    key_prefix: String,
    device_type_id: Option<i32>,
    device_type_name: Option<String>,
    created_at: String,
    last_used_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreatedApiKeyResponse {
    id: i32,
    name: String,
    key: String,
    key_prefix: String,
    device_type_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CertificateBundle {
    certificate_pem: String,
    private_key_pem: String,
    ca_pem: String,
    fingerprint: String,
    expires_at: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CaCertificate {
    fingerprint: String,
    certificate_pem: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CertificateStatus {
    fingerprint: String,
    expires_at: String,
    created_at: String,
}

#[derive(Clone)]
struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = config_path(cli.config.as_deref())?;
    let mut config = load_config(&config_path)?;
    let base_url = cli
        .url
        .clone()
        .or_else(|| config.url.clone())
        .unwrap_or_else(|| DEFAULT_URL.to_string());
    let token = cli.token.clone().or_else(|| config.token.clone());

    let client = ApiClient {
        http: reqwest::Client::new(),
        base_url: normalize_url(&base_url),
        token,
    };

    match cli.command {
        Command::Auth(auth) => match auth.command {
            AuthSubcommand::Login(args) => {
                let password = match args.password {
                    Some(password) => password,
                    None => prompt_password()?,
                };
                let response: LoginResponse = client
                    .request(
                        Method::POST,
                        "/api/v1/auth/login",
                        Some(json!({
                            "username": args.username,
                            "password": password,
                        })),
                        false,
                    )
                    .await?;

                if !args.no_save {
                    config.url = Some(client.base_url.clone());
                    config.token = Some(response.token.clone());
                    save_config(&config_path, &config)?;
                }

                output(cli.output, &response, || {
                    format!(
                        "Logged in as {} ({})",
                        response.user.username, response.user.role
                    )
                })?;
            }
            AuthSubcommand::Me => {
                let user: UserResponse = client.get("/api/v1/auth/me").await?;
                output(cli.output, &user, || {
                    format!("{} ({}) id={}", user.username, user.role, user.id)
                })?;
            }
            AuthSubcommand::Logout => {
                config.token = None;
                save_config(&config_path, &config)?;
                let value = json!({ "logged_out": true });
                output(cli.output, &value, || "Logged out".to_string())?;
            }
        },
        Command::Config(config_cmd) => match config_cmd.command {
            ConfigSubcommand::Show => {
                let value = json!({
                    "config_path": config_path,
                    "url": client.base_url,
                    "token_saved": config.token.is_some(),
                    "token_active": client.token.is_some(),
                });
                output(cli.output, &value, || {
                    format!(
                        "config: {}\nurl: {}\ntoken_saved: {}\ntoken_active: {}",
                        value["config_path"].as_str().unwrap_or_default(),
                        value["url"].as_str().unwrap_or_default(),
                        value["token_saved"].as_bool().unwrap_or(false),
                        value["token_active"].as_bool().unwrap_or(false),
                    )
                })?;
            }
            ConfigSubcommand::SetUrl { url } => {
                config.url = Some(normalize_url(&url));
                save_config(&config_path, &config)?;
                let value = json!({ "url": config.url.as_deref().unwrap_or(DEFAULT_URL) });
                output(cli.output, &value, || {
                    format!(
                        "Saved backend URL: {}",
                        config.url.as_deref().unwrap_or(DEFAULT_URL)
                    )
                })?;
            }
        },
        Command::Health => {
            let value: Value = client.request(Method::GET, "/health", None, false).await?;
            output(cli.output, &value, || "Backend is healthy".to_string())?;
        }
        Command::Devices(devices) => match devices.command {
            DevicesSubcommand::List(args) => {
                let query = device_list_query(&args);
                let devices: Paginated<DeviceResponse> =
                    client.get(&format!("/api/v1/devices{query}")).await?;
                output(cli.output, &devices, || format_devices(&devices))?;
            }
            DevicesSubcommand::Get { id } => {
                let device: DeviceResponse = client.get(&format!("/api/v1/devices/{id}")).await?;
                output(cli.output, &device, || format_device(&device))?;
            }
            DevicesSubcommand::Create(args) => {
                let device = create_device(&client, args).await?;
                output(cli.output, &device, || format_device(&device))?;
            }
            DevicesSubcommand::Delete { id } => {
                client
                    .request_empty(Method::DELETE, &format!("/api/v1/devices/{id}"))
                    .await?;
                let value = json!({ "deleted": true, "id": id });
                output(cli.output, &value, || {
                    format!(
                        "Deleted device {}",
                        value["id"].as_str().unwrap_or_default()
                    )
                })?;
            }
        },
        Command::DeviceTypes(device_types) => match device_types.command {
            DeviceTypesSubcommand::List(args) => {
                let result: Paginated<DeviceTypeResponse> = client
                    .get(&format!("/api/v1/device-types{}", page_query(&args)))
                    .await?;
                output(cli.output, &result, || format_device_types(&result))?;
            }
            DeviceTypesSubcommand::Create { name } => {
                let result: DeviceTypeResponse = client
                    .request(
                        Method::POST,
                        "/api/v1/device-types",
                        Some(json!({ "name": name })),
                        true,
                    )
                    .await?;
                output(cli.output, &result, || {
                    format!("{} {}", result.id, result.name)
                })?;
            }
            DeviceTypesSubcommand::Delete { id } => {
                client
                    .request_empty(Method::DELETE, &format!("/api/v1/device-types/{id}"))
                    .await?;
                let value = json!({ "deleted": true, "id": id });
                output(cli.output, &value, || {
                    format!(
                        "Deleted device type {}",
                        value["id"].as_i64().unwrap_or_default()
                    )
                })?;
            }
        },
        Command::Fleets(fleets) => match fleets.command {
            FleetsSubcommand::List(args) => {
                let result: Paginated<FleetResponse> = client
                    .get(&format!("/api/v1/fleets{}", page_query(&args)))
                    .await?;
                output(cli.output, &result, || format_fleets(&result))?;
            }
            FleetsSubcommand::Create { name } => {
                let result: FleetResponse = client
                    .request(
                        Method::POST,
                        "/api/v1/fleets",
                        Some(json!({ "name": name })),
                        true,
                    )
                    .await?;
                output(cli.output, &result, || {
                    format!(
                        "{} {} devices={}",
                        result.id, result.name, result.device_count
                    )
                })?;
            }
            FleetsSubcommand::Delete { id } => {
                client
                    .request_empty(Method::DELETE, &format!("/api/v1/fleets/{id}"))
                    .await?;
                let value = json!({ "deleted": true, "id": id });
                output(cli.output, &value, || {
                    format!("Deleted fleet {}", value["id"].as_i64().unwrap_or_default())
                })?;
            }
        },
        Command::ApiKeys(api_keys) => match api_keys.command {
            ApiKeysSubcommand::List => {
                let result: Vec<ApiKeyResponse> = client.get("/api/v1/api-keys").await?;
                output(cli.output, &result, || format_api_keys(&result))?;
            }
            ApiKeysSubcommand::Create {
                name,
                device_type_id,
            } => {
                let result: CreatedApiKeyResponse = client
                    .request(
                        Method::POST,
                        "/api/v1/api-keys",
                        Some(json!({ "name": name, "device_type_id": device_type_id })),
                        true,
                    )
                    .await?;
                output(cli.output, &result, || {
                    format!(
                        "id={}\nname={}\nkey={}\nkey_prefix={}",
                        result.id, result.name, result.key, result.key_prefix
                    )
                })?;
            }
            ApiKeysSubcommand::Delete { id } => {
                client
                    .request_empty(Method::DELETE, &format!("/api/v1/api-keys/{id}"))
                    .await?;
                let value = json!({ "deleted": true, "id": id });
                output(cli.output, &value, || {
                    format!(
                        "Deleted API key {}",
                        value["id"].as_i64().unwrap_or_default()
                    )
                })?;
            }
        },
        Command::Certs(certs) => match certs.command {
            CertsSubcommand::Ca => {
                let cert: CaCertificate = client.get("/api/v1/ca/certificate").await?;
                output(cli.output, &cert, || cert.certificate_pem.clone())?;
            }
            CertsSubcommand::Download(args) => {
                let bundle = download_device_cert(&client, &args.device_id, false).await?;
                handle_cert_bundle(cli.output, bundle, args.out_dir.as_deref())?;
            }
            CertsSubcommand::Regenerate(args) => {
                let bundle = download_device_cert(&client, &args.device_id, true).await?;
                handle_cert_bundle(cli.output, bundle, args.out_dir.as_deref())?;
            }
            CertsSubcommand::Status { device_id } => {
                let status: Option<CertificateStatus> = client
                    .get(&format!("/api/v1/devices/{device_id}/certificate/status"))
                    .await?;
                output(cli.output, &status, || match status {
                    Some(ref s) => format!(
                        "fingerprint={}\nexpires_at={}\ncreated_at={}",
                        s.fingerprint, s.expires_at, s.created_at
                    ),
                    None => "No certificate".to_string(),
                })?;
            }
        },
        Command::Provision(args) => {
            let device = create_device(&client, args.device.clone()).await?;
            let cert_written = if let Some(cert_dir) = args.cert_dir.as_deref() {
                let bundle =
                    download_device_cert(&client, &device.id, args.regenerate_cert).await?;
                write_cert_bundle(cert_dir, &bundle)?;
                Some(cert_dir.to_path_buf())
            } else {
                None
            };
            let value = json!({
                "device_id": device.id,
                "name": device.name,
                "device_type_id": device.device_type_id,
                "device_type_name": device.device_type_name,
                "fleet_id": device.fleet_id,
                "fleet_name": device.fleet_name,
                "firmware": device.firmware,
                "zenoh_connect": args.zenoh_connect,
                "certificate_dir": cert_written,
            });
            output(cli.output, &value, || format_provisioning(&value))?;
        }
    }

    Ok(())
}

impl ApiClient {
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.request(Method::GET, path, None, true).await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        auth: bool,
    ) -> Result<T> {
        let response = self.send(method, path, body, auth).await?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("{}", api_error(status, &bytes));
        }

        serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "failed to parse response from {} as JSON",
                path.trim_start_matches('/')
            )
        })
    }

    async fn request_empty(&self, method: Method, path: &str) -> Result<()> {
        let response = self.send(method, path, None, true).await?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read response body")?;

        if !status.is_success() {
            bail!("{}", api_error(status, &bytes));
        }

        Ok(())
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        auth: bool,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.request(method, url);

        if auth {
            let token = self
                .token
                .as_deref()
                .ok_or_else(|| anyhow!("not authenticated; run `extrittio auth login` first"))?;
            request = request.bearer_auth(token);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        request.send().await.context("request failed")
    }
}

async fn create_device(client: &ApiClient, args: CreateDeviceArgs) -> Result<DeviceResponse> {
    client
        .request(
            Method::POST,
            "/api/v1/devices",
            Some(json!({
                "name": args.name,
                "device_type_id": args.device_type_id,
                "fleet_id": args.fleet_id,
                "firmware": args.firmware,
            })),
            true,
        )
        .await
}

async fn download_device_cert(
    client: &ApiClient,
    device_id: &str,
    regenerate: bool,
) -> Result<CertificateBundle> {
    let path = if regenerate {
        format!("/api/v1/devices/{device_id}/certificate/regenerate")
    } else {
        format!("/api/v1/devices/{device_id}/certificate")
    };
    let method = if regenerate {
        Method::POST
    } else {
        Method::GET
    };
    client.request(method, &path, None, true).await
}

fn handle_cert_bundle(
    output_format: OutputFormat,
    bundle: CertificateBundle,
    out_dir: Option<&Path>,
) -> Result<()> {
    if let Some(out_dir) = out_dir {
        write_cert_bundle(out_dir, &bundle)?;
        let value = json!({
            "fingerprint": bundle.fingerprint,
            "expires_at": bundle.expires_at,
            "created_at": bundle.created_at,
            "files": {
                "ca": out_dir.join("ca.pem"),
                "certificate": out_dir.join("device.pem"),
                "private_key": out_dir.join("device-key.pem"),
            }
        });
        output(output_format, &value, || {
            format!(
                "Wrote certificate bundle to {}\nfingerprint={}\nexpires_at={}",
                out_dir.display(),
                value["fingerprint"].as_str().unwrap_or_default(),
                value["expires_at"].as_str().unwrap_or_default()
            )
        })?;
    } else {
        output(output_format, &bundle, || {
            format!(
                "{}\n{}\n{}",
                bundle.certificate_pem, bundle.private_key_pem, bundle.ca_pem
            )
        })?;
    }
    Ok(())
}

fn write_cert_bundle(out_dir: &Path, bundle: &CertificateBundle) -> Result<()> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    fs::write(out_dir.join("ca.pem"), &bundle.ca_pem)
        .with_context(|| format!("failed to write {}", out_dir.join("ca.pem").display()))?;
    fs::write(out_dir.join("device.pem"), &bundle.certificate_pem)
        .with_context(|| format!("failed to write {}", out_dir.join("device.pem").display()))?;
    let key_path = out_dir.join("device-key.pem");
    fs::write(&key_path, &bundle.private_key_pem)
        .with_context(|| format!("failed to write {}", key_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", key_path.display()))?;
    }

    Ok(())
}

fn prompt_password() -> Result<String> {
    print!("Password: ");
    io::stdout().flush().context("failed to flush stdout")?;
    let mut password = String::new();
    io::stdin()
        .read_line(&mut password)
        .context("failed to read password")?;
    Ok(password.trim_end_matches(['\r', '\n']).to_string())
}

fn config_path(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    if let Ok(path) = env::var("EXTRITTIO_CLI_CONFIG") {
        return Ok(PathBuf::from(path));
    }

    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(dir).join("extrittio").join("cli.json"));
    }

    let home = env::var("HOME").context("HOME is not set; pass --config explicitly")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("extrittio")
        .join("cli.json"))
}

fn load_config(path: &Path) -> Result<CliConfig> {
    if !path.exists() {
        return Ok(CliConfig::default());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn save_config(path: &Path, config: &CliConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, format!("{contents}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn output<T: Serialize, F: FnOnce() -> String>(
    output_format: OutputFormat,
    value: &T,
    table: F,
) -> Result<()> {
    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).context("failed to serialize JSON output")?
            );
        }
        OutputFormat::Table => println!("{}", table()),
    }
    Ok(())
}

fn api_error(status: StatusCode, bytes: &[u8]) -> String {
    let message = serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .or_else(|| String::from_utf8(bytes.to_vec()).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| status.to_string());

    format!("API error {status}: {message}")
}

fn page_query(args: &PageArgs) -> String {
    format!("?limit={}&offset={}", args.limit, args.offset)
}

fn device_list_query(args: &ListDevicesArgs) -> String {
    let mut params = vec![
        format!("limit={}", args.limit),
        format!("offset={}", args.offset),
    ];
    if let Some(status) = &args.status {
        params.push(format!("status={}", percent_encode(status)));
    }
    if let Some(search) = &args.search {
        params.push(format!("search={}", percent_encode(search)));
    }
    if let Some(fleet_id) = args.fleet_id {
        params.push(format!("fleet_id={fleet_id}"));
    }
    format!("?{}", params.join("&"))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn format_devices(devices: &Paginated<DeviceResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<38} {:<24} {:<14} {:<12} {:<16} {}",
        "ID", "NAME", "TYPE", "STATUS", "FLEET", "FIRMWARE"
    );
    for device in &devices.data {
        let _ = writeln!(
            out,
            "{:<38} {:<24} {:<14} {:<12} {:<16} {}",
            truncate(&device.id, 38),
            truncate(&device.name, 24),
            truncate(&device.device_type_name, 14),
            truncate(&device.status, 12),
            truncate(device.fleet_name.as_deref().unwrap_or("-"), 16),
            device.firmware
        );
    }
    let _ = write!(
        out,
        "\nshowing {} of {} (limit={}, offset={})",
        devices.data.len(),
        devices.total,
        devices.limit,
        devices.offset
    );
    out
}

fn format_device(device: &DeviceResponse) -> String {
    format!(
        "id={}\nname={}\ntype={} ({})\nfleet={}\nstatus={}\nlast_seen={}\nfirmware={}\nuptime={}",
        device.id,
        device.name,
        device.device_type_name,
        device.device_type_id,
        device.fleet_name.as_deref().unwrap_or("-"),
        device.status,
        device.last_seen,
        device.firmware,
        device.uptime,
    )
}

fn format_device_types(device_types: &Paginated<DeviceTypeResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<8} NAME", "ID");
    for device_type in &device_types.data {
        let _ = writeln!(out, "{:<8} {}", device_type.id, device_type.name);
    }
    let _ = write!(
        out,
        "\nshowing {} of {}",
        device_types.data.len(),
        device_types.total
    );
    out
}

fn format_fleets(fleets: &Paginated<FleetResponse>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<8} {:<28} DEVICES", "ID", "NAME");
    for fleet in &fleets.data {
        let _ = writeln!(
            out,
            "{:<8} {:<28} {}",
            fleet.id,
            truncate(&fleet.name, 28),
            fleet.device_count
        );
    }
    let _ = write!(out, "\nshowing {} of {}", fleets.data.len(), fleets.total);
    out
}

fn format_api_keys(api_keys: &[ApiKeyResponse]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<8} {:<24} {:<16} {:<18} LAST_USED",
        "ID", "NAME", "PREFIX", "DEVICE_TYPE"
    );
    for key in api_keys {
        let _ = writeln!(
            out,
            "{:<8} {:<24} {:<16} {:<18} {}",
            key.id,
            truncate(&key.name, 24),
            key.key_prefix,
            truncate(key.device_type_name.as_deref().unwrap_or("-"), 18),
            key.last_used_at.as_deref().unwrap_or("-")
        );
    }
    out
}

fn format_provisioning(value: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "device_id={}",
        value["device_id"].as_str().unwrap_or_default()
    );
    let _ = writeln!(out, "name={}", value["name"].as_str().unwrap_or_default());
    let _ = writeln!(
        out,
        "device_type={} ({})",
        value["device_type_name"].as_str().unwrap_or_default(),
        value["device_type_id"].as_i64().unwrap_or_default(),
    );
    if let Some(fleet_name) = value["fleet_name"].as_str() {
        let _ = writeln!(out, "fleet={fleet_name}");
    }
    let _ = writeln!(
        out,
        "firmware={}",
        value["firmware"].as_str().unwrap_or_default()
    );
    let _ = writeln!(
        out,
        "zenoh_connect={}",
        value["zenoh_connect"]
            .as_str()
            .unwrap_or(DEFAULT_ZENOH_CONNECT)
    );
    if let Some(cert_dir) = value["certificate_dir"].as_str() {
        let _ = writeln!(out, "certificate_dir={cert_dir}");
    }
    out
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut result = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}
