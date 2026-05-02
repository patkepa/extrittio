use std::{
    env,
    fmt::Write as _,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const DEFAULT_URL: &str = "http://localhost:8080";
const DEFAULT_ZENOH_CONNECT: &str = "tcp/127.0.0.1:7447";
const DEFAULT_ESP32_CHIP: &str = "esp32c6";
const DEFAULT_ESP32_BAUD: u32 = 460_800;
const DEFAULT_ESP32_NVS_OFFSET: &str = "0x9000";
const DEFAULT_ESP32_NVS_SIZE: &str = "0x4000";

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
    device_type_id: Option<i32>,

    #[arg(long, value_name = "NAME")]
    device_type: Option<String>,

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

    /// Generate and flash an ESP-IDF NVS image with this device's runtime config.
    #[arg(long)]
    flash_esp32_nvs: bool,

    /// ESP serial port. Auto-detected when omitted and exactly one USB serial device exists.
    #[arg(long)]
    port: Option<PathBuf>,

    /// ESP chip passed to esptool.py.
    #[arg(long, default_value = DEFAULT_ESP32_CHIP)]
    chip: String,

    /// ESP serial baud rate passed to esptool.py.
    #[arg(long, default_value_t = DEFAULT_ESP32_BAUD)]
    baud: u32,

    /// NVS partition offset for the target firmware partition table.
    #[arg(long, default_value = DEFAULT_ESP32_NVS_OFFSET)]
    nvs_offset: String,

    /// NVS partition size for the generated image.
    #[arg(long, default_value = DEFAULT_ESP32_NVS_SIZE)]
    nvs_size: String,

    /// Wi-Fi SSID to write into ESP NVS. Defaults to EXTRITTIO_WIFI_SSID.
    #[arg(long, env = "EXTRITTIO_WIFI_SSID")]
    wifi_ssid: Option<String>,

    /// Wi-Fi password to write into ESP NVS. Defaults to EXTRITTIO_WIFI_PASSWORD.
    #[arg(long, env = "EXTRITTIO_WIFI_PASSWORD")]
    wifi_password: Option<String>,

    /// Firmware version to write into ESP NVS. Defaults to the created device firmware.
    #[arg(long)]
    esp32_firmware_version: Option<String>,

    /// ESP-IDF path used to locate nvs_partition_gen.py and esptool.py.
    #[arg(long, env = "IDF_PATH")]
    idf_path: Option<PathBuf>,

    /// Python interpreter for ESP-IDF Python tools.
    #[arg(long, env = "EXTRITTIO_IDF_PYTHON")]
    idf_python: Option<PathBuf>,

    /// Override path to nvs_partition_gen.py.
    #[arg(long, env = "EXTRITTIO_NVS_PARTITION_GEN")]
    nvs_partition_gen: Option<PathBuf>,

    /// Override path to esptool.py.
    #[arg(long, env = "EXTRITTIO_ESPTOOL")]
    esptool: Option<PathBuf>,

    /// Keep generated NVS CSV and binary files for inspection.
    #[arg(long)]
    keep_nvs_artifacts: bool,
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

#[derive(Debug, Serialize)]
struct Esp32NvsFlashResult {
    port: String,
    chip: String,
    baud: u32,
    nvs_offset: String,
    nvs_size: String,
    csv_path: Option<PathBuf>,
    bin_path: Option<PathBuf>,
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
            let esp32_nvs = if args.flash_esp32_nvs {
                let result = flash_esp32_nvs(&device, &args)?;
                Some(result)
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
                "esp32_nvs": esp32_nvs,
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
    let device_type_id =
        resolve_device_type_id(client, args.device_type_id, args.device_type.as_deref()).await?;
    client
        .request(
            Method::POST,
            "/api/v1/devices",
            Some(json!({
                "name": args.name,
                "device_type_id": device_type_id,
                "fleet_id": args.fleet_id,
                "firmware": args.firmware,
            })),
            true,
        )
        .await
}

async fn resolve_device_type_id(
    client: &ApiClient,
    device_type_id: Option<i32>,
    device_type_name: Option<&str>,
) -> Result<i32> {
    if device_type_id.is_some() && device_type_name.is_some() {
        bail!("pass either --device-type-id or --device-type, not both");
    }
    if let Some(device_type_id) = device_type_id {
        return Ok(device_type_id);
    }

    let device_type_name = device_type_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("pass --device-type-id or --device-type"))?;
    let existing: Paginated<DeviceTypeResponse> = client
        .get("/api/v1/device-types?limit=1000&offset=0")
        .await?;
    if let Some(device_type) = existing
        .data
        .iter()
        .find(|device_type| device_type.name.eq_ignore_ascii_case(device_type_name))
    {
        return Ok(device_type.id);
    }

    let created: DeviceTypeResponse = client
        .request(
            Method::POST,
            "/api/v1/device-types",
            Some(json!({ "name": device_type_name })),
            true,
        )
        .await?;
    Ok(created.id)
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

fn flash_esp32_nvs(device: &DeviceResponse, args: &ProvisionArgs) -> Result<Esp32NvsFlashResult> {
    let wifi_ssid = args
        .wifi_ssid
        .as_deref()
        .filter(|ssid| !ssid.is_empty())
        .ok_or_else(|| {
            anyhow!("--wifi-ssid or EXTRITTIO_WIFI_SSID is required with --flash-esp32-nvs")
        })?;
    let wifi_password = args.wifi_password.as_deref().unwrap_or("");
    let firmware_version = args
        .esp32_firmware_version
        .as_deref()
        .unwrap_or(device.firmware.as_str());
    let port = args
        .port
        .clone()
        .map(Ok)
        .unwrap_or_else(detect_esp_serial_port)?;
    let nvs_gen = resolve_esp_tool(
        args.nvs_partition_gen.as_deref(),
        args.idf_path.as_deref(),
        &[
            "components",
            "nvs_flash",
            "nvs_partition_generator",
            "nvs_partition_gen.py",
        ],
        "nvs_partition_gen.py",
    );
    let esptool = resolve_esp_tool(
        args.esptool.as_deref(),
        args.idf_path.as_deref(),
        &["components", "esptool_py", "esptool", "esptool.py"],
        "esptool.py",
    );
    let idf_python = resolve_idf_python(args.idf_python.as_deref());
    let (csv_path, bin_path) = nvs_artifact_paths(&device.id)?;

    write_esp32_nvs_csv(
        &csv_path,
        &[
            ("device_id", device.id.as_str()),
            ("wifi_ssid", wifi_ssid),
            ("wifi_pass", wifi_password),
            ("zenoh", args.zenoh_connect.as_str()),
            ("fw_version", firmware_version),
        ],
    )?;

    run_process(
        ProcessCommand::new(&idf_python)
            .arg(&nvs_gen)
            .arg("generate")
            .arg(&csv_path)
            .arg(&bin_path)
            .arg(&args.nvs_size),
        "failed to generate ESP32 NVS image",
    )?;

    run_process(
        ProcessCommand::new(&idf_python)
            .arg(&esptool)
            .arg("--chip")
            .arg(&args.chip)
            .arg("-p")
            .arg(&port)
            .arg("-b")
            .arg(args.baud.to_string())
            .arg("--before")
            .arg("default_reset")
            .arg("--after")
            .arg("hard_reset")
            .arg("write_flash")
            .arg(&args.nvs_offset)
            .arg(&bin_path),
        "failed to flash ESP32 NVS image",
    )?;

    let result = Esp32NvsFlashResult {
        port: port.display().to_string(),
        chip: args.chip.clone(),
        baud: args.baud,
        nvs_offset: args.nvs_offset.clone(),
        nvs_size: args.nvs_size.clone(),
        csv_path: args.keep_nvs_artifacts.then(|| csv_path.clone()),
        bin_path: args.keep_nvs_artifacts.then(|| bin_path.clone()),
    };

    if !args.keep_nvs_artifacts {
        let _ = fs::remove_file(&csv_path);
        let _ = fs::remove_file(&bin_path);
    }

    Ok(result)
}

fn resolve_esp_tool(
    explicit: Option<&Path>,
    idf_path: Option<&Path>,
    idf_relative: &[&str],
    fallback: &str,
) -> PathBuf {
    if let Some(explicit) = explicit {
        return explicit.to_path_buf();
    }
    if let Some(idf_path) = idf_path {
        let mut path = idf_path.to_path_buf();
        for segment in idf_relative {
            path.push(segment);
        }
        return path;
    }
    PathBuf::from(fallback)
}

fn resolve_idf_python(explicit: Option<&Path>) -> PathBuf {
    if let Some(explicit) = explicit {
        return explicit.to_path_buf();
    }
    if let Ok(env_path) = env::var("IDF_PYTHON_ENV_PATH") {
        return PathBuf::from(env_path).join("bin").join("python");
    }
    PathBuf::from("python")
}

fn nvs_artifact_paths(device_id: &str) -> Result<(PathBuf, PathBuf)> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis();
    let safe_id = device_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let base = env::temp_dir().join(format!(
        "extrittio-{safe_id}-{}-{nonce}",
        std::process::id()
    ));
    Ok((base.with_extension("csv"), base.with_extension("bin")))
}

fn write_esp32_nvs_csv(path: &Path, entries: &[(&str, &str)]) -> Result<()> {
    let mut csv = String::from("key,type,encoding,value\nextrittio,namespace,,\n");
    for (key, value) in entries {
        let _ = writeln!(csv, "{key},data,string,{}", nvs_csv_escape(value));
    }
    fs::write(path, csv).with_context(|| format!("failed to write {}", path.display()))
}

fn nvs_csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn detect_esp_serial_port() -> Result<PathBuf> {
    let mut ports = Vec::new();
    let mut callout_ports = Vec::new();
    let dev = Path::new("/dev");
    for entry in fs::read_dir(dev).context("failed to read /dev for ESP serial ports")? {
        let entry = entry.context("failed to read /dev entry")?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_likely_esp_serial_port(&name) {
            let path = dev.join(&name);
            if name.starts_with("cu.") {
                callout_ports.push(path);
            } else {
                ports.push(path);
            }
        }
    }
    if !callout_ports.is_empty() {
        ports = callout_ports;
    }
    ports.sort();
    if ports.is_empty() {
        bail!("no ESP serial port detected; pass --port /dev/<device>");
    }
    if ports.len() > 1 {
        let mut message =
            String::from("multiple ESP serial ports detected; pass --port explicitly:");
        for port in ports {
            let _ = write!(message, "\n  {}", port.display());
        }
        bail!("{message}");
    }
    Ok(ports.remove(0))
}

fn is_likely_esp_serial_port(name: &str) -> bool {
    name.starts_with("cu.usbmodem")
        || name.starts_with("tty.usbmodem")
        || name.starts_with("cu.usbserial")
        || name.starts_with("tty.usbserial")
        || name.starts_with("cu.SLAB_USBtoUART")
        || name.starts_with("tty.SLAB_USBtoUART")
        || name.starts_with("cu.wchusbserial")
        || name.starts_with("tty.wchusbserial")
        || name.starts_with("ttyUSB")
        || name.starts_with("ttyACM")
}

fn run_process(command: &mut ProcessCommand, context: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("{context}: failed to start process"))?;
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "{context}: exit status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout.trim(),
        stderr.trim()
    )
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
    if let Some(esp32_nvs) = value["esp32_nvs"].as_object() {
        let _ = writeln!(
            out,
            "esp32_nvs=flashed port={} offset={} size={}",
            esp32_nvs
                .get("port")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            esp32_nvs
                .get("nvs_offset")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_ESP32_NVS_OFFSET),
            esp32_nvs
                .get("nvs_size")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_ESP32_NVS_SIZE),
        );
        if let Some(csv_path) = esp32_nvs.get("csv_path").and_then(Value::as_str) {
            let _ = writeln!(out, "nvs_csv={csv_path}");
        }
        if let Some(bin_path) = esp32_nvs.get("bin_path").and_then(Value::as_str) {
            let _ = writeln!(out, "nvs_bin={bin_path}");
        }
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
