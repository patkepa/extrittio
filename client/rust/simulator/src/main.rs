use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use extrittio_common::extrittio::{DeviceHeartbeat, DeviceTelemetry};
use extrittio_common::{device_status, topics};
use extrittio_sdk::sensor::SensorState;
use prost::Message;
use rand::Rng;
use tokio::task::JoinSet;
use tracing::{info, warn};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "extrittio-simulator",
    about = "Simulate many Extrittio devices over the real Zenoh protocol"
)]
struct Args {
    /// Number of logical devices to simulate.
    #[arg(long, default_value_t = 100)]
    count: u32,

    /// Device ID prefix. IDs are generated as <prefix>-000001.
    #[arg(long, default_value = "sim-device")]
    prefix: String,

    /// Backend Zenoh endpoint, for example tcp/127.0.0.1:7447 or tls/127.0.0.1:7447.
    #[arg(long)]
    connect: Option<String>,

    /// Seconds between telemetry messages per simulated device.
    #[arg(long, default_value_t = 5)]
    telemetry_interval: u64,

    /// Seconds between heartbeat messages per simulated device.
    #[arg(long, default_value_t = 30)]
    heartbeat_interval: u64,

    /// Spread initial device startup over this many milliseconds.
    #[arg(long, default_value_t = 2_000)]
    startup_spread_ms: u64,

    /// Randomize each send interval by +/- this percent.
    #[arg(long, default_value_t = 20)]
    jitter_percent: u8,

    /// Give each device a persistent +/- interval variance so they do not share the same cadence.
    #[arg(long, default_value_t = 15)]
    device_interval_variance_percent: u8,

    /// Add random measurement noise to emitted telemetry values.
    #[arg(long, default_value_t = 5)]
    sensor_noise_percent: u8,

    /// Seconds between aggregate simulator stats logs.
    #[arg(long, default_value_t = 5)]
    stats_interval: u64,

    /// Firmware string sent in heartbeats. Backend uses this for device-type inference.
    #[arg(long, default_value = "simulator-v1.0.0")]
    firmware: String,

    /// Fake telemetry scenario.
    #[arg(long, value_enum, default_value_t = Scenario::Normal)]
    scenario: Scenario,

    /// Initial location distribution for mobile simulated devices.
    #[arg(long, value_enum, default_value_t = LocationArea::Poland)]
    location_area: LocationArea,

    /// Use one shared Zenoh session, or one session per logical device.
    #[arg(long, value_enum, default_value_t = SessionMode::Shared)]
    session_mode: SessionMode,

    /// Optional run duration. Examples: 30s, 10m, 2h.
    #[arg(long, value_parser = parse_duration)]
    duration: Option<Duration>,

    /// Path to CA certificate PEM file when connecting over TLS.
    #[arg(long)]
    ca_cert: Option<String>,

    /// Path to client certificate PEM file when connecting over mTLS.
    #[arg(long)]
    client_cert: Option<String>,

    /// Path to client private key PEM file when connecting over mTLS.
    #[arg(long)]
    client_key: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Scenario {
    Normal,
    Hot,
    LowBattery,
    Mobile,
    Flaky,
    Burst,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LocationArea {
    Poland,
    Warsaw,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SessionMode {
    Shared,
    PerDevice,
}

struct DeviceRuntime {
    id: String,
    firmware: String,
    sensor: SensorState,
    start: Instant,
    telemetry_period: Duration,
    heartbeat_period: Duration,
    battery_drain_multiplier: f32,
    latitude: f64,
    longitude: f64,
    heading: f32,
    speed: f32,
    altitude: f32,
}

#[derive(Default)]
struct SimulatorMetrics {
    devices_started: AtomicU64,
    heartbeats_sent: AtomicU64,
    heartbeats_failed: AtomicU64,
    telemetry_sent: AtomicU64,
    telemetry_failed: AtomicU64,
    flaky_heartbeats_skipped: AtomicU64,
}

#[derive(Clone, Copy, Default)]
struct MetricsSnapshot {
    devices_started: u64,
    heartbeats_sent: u64,
    heartbeats_failed: u64,
    telemetry_sent: u64,
    telemetry_failed: u64,
    flaky_heartbeats_skipped: u64,
}

const POLISH_CITY_CENTERS: &[(f64, f64)] = &[
    (52.2297, 21.0122), // Warsaw
    (50.0647, 19.9450), // Krakow
    (51.7592, 19.4560), // Lodz
    (51.1079, 17.0385), // Wroclaw
    (52.4064, 16.9252), // Poznan
    (54.3520, 18.6466), // Gdansk
    (53.4285, 14.5528), // Szczecin
    (53.1235, 18.0084), // Bydgoszcz
    (51.2465, 22.5684), // Lublin
    (53.1325, 23.1688), // Bialystok
    (50.2649, 19.0238), // Katowice
    (50.0412, 21.9991), // Rzeszow
    (53.7784, 20.4801), // Olsztyn
    (54.5189, 18.5305), // Gdynia
    (50.8661, 20.6286), // Kielce
    (52.7325, 15.2369), // Gorzow Wielkopolski
];

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "extrittio_simulator=info".into()),
        )
        .init();

    let args = Args::parse();
    validate_args(&args);

    let estimated_telemetry_rate = args.count as f64 / args.telemetry_interval as f64;
    let estimated_heartbeat_rate = args.count as f64 / args.heartbeat_interval as f64;
    info!(
        "Starting {} simulated devices ({:?}, {:?}); nominal inbound rate: {:.1} telemetry/s + {:.1} heartbeat/s; jitter={}%, device variance={}%, sensor noise={}%",
        args.count,
        args.scenario,
        args.session_mode,
        estimated_telemetry_rate,
        estimated_heartbeat_rate,
        args.jitter_percent,
        args.device_interval_variance_percent,
        args.sensor_noise_percent,
    );

    match args.session_mode {
        SessionMode::Shared => run_shared(args).await,
        SessionMode::PerDevice => run_per_device(args).await,
    }
}

async fn run_shared(args: Args) {
    let session = Arc::new(open_session(&args).await);
    info!("Opened shared Zenoh session");

    let metrics = Arc::new(SimulatorMetrics::default());
    let mut tasks = JoinSet::new();
    tasks.spawn(report_stats(
        metrics.clone(),
        args.count,
        Duration::from_secs(args.stats_interval),
    ));
    for index in 0..args.count {
        let device = build_device(&args, index);
        let device_args = args.clone();
        let device_session = session.clone();
        let device_metrics = metrics.clone();
        tasks.spawn(async move {
            run_device(device, device_args, device_session, device_metrics).await;
        });
    }

    wait_for_shutdown(args.duration, tasks).await;
}

async fn run_per_device(args: Args) {
    let metrics = Arc::new(SimulatorMetrics::default());
    let mut tasks = JoinSet::new();
    tasks.spawn(report_stats(
        metrics.clone(),
        args.count,
        Duration::from_secs(args.stats_interval),
    ));
    for index in 0..args.count {
        let device = build_device(&args, index);
        let device_args = args.clone();
        let device_metrics = metrics.clone();
        tasks.spawn(async move {
            let session = Arc::new(open_session(&device_args).await);
            run_device(device, device_args, session, device_metrics).await;
        });
    }

    wait_for_shutdown(args.duration, tasks).await;
}

async fn wait_for_shutdown(duration: Option<Duration>, mut tasks: JoinSet<()>) {
    match duration {
        Some(duration) => {
            tokio::select! {
                _ = tokio::time::sleep(duration) => info!("Simulation duration elapsed"),
                _ = shutdown_signal() => info!("Shutdown requested"),
            }
        }
        None => shutdown_signal().await,
    }

    tasks.abort_all();
    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result
            && !e.is_cancelled()
        {
            warn!("Device task ended unexpectedly: {e}");
        }
    }
}

async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        warn!("Failed to listen for Ctrl-C: {e}");
    }
}

async fn run_device(
    mut device: DeviceRuntime,
    args: Args,
    session: Arc<zenoh::Session>,
    metrics: Arc<SimulatorMetrics>,
) {
    sleep_random(Duration::from_millis(args.startup_spread_ms)).await;

    if let Err(e) = publish_heartbeat(&device, &session, device_status::ONLINE).await {
        metrics.heartbeats_failed.fetch_add(1, Ordering::Relaxed);
        warn!("{} failed initial heartbeat: {e}", device.id);
        return;
    }
    metrics.heartbeats_sent.fetch_add(1, Ordering::Relaxed);
    metrics.devices_started.fetch_add(1, Ordering::Relaxed);

    let mut next_heartbeat = Box::pin(tokio::time::sleep(jittered_delay(
        device.heartbeat_period,
        args.jitter_percent,
    )));
    let mut next_telemetry = Box::pin(tokio::time::sleep(jittered_delay(
        device.telemetry_period,
        args.jitter_percent,
    )));

    loop {
        tokio::select! {
            _ = &mut next_heartbeat => {
                if should_skip_flaky_heartbeat(args.scenario) {
                    metrics.flaky_heartbeats_skipped.fetch_add(1, Ordering::Relaxed);
                    warn!("{} skipped heartbeat due to flaky scenario", device.id);
                } else if let Err(e) = publish_heartbeat(&device, &session, device_status::ONLINE).await {
                    metrics.heartbeats_failed.fetch_add(1, Ordering::Relaxed);
                    warn!("{} failed heartbeat: {e}", device.id);
                } else {
                    metrics.heartbeats_sent.fetch_add(1, Ordering::Relaxed);
                }
                next_heartbeat.as_mut().reset(
                    tokio::time::Instant::now() + jittered_delay(device.heartbeat_period, args.jitter_percent),
                );
            }
            _ = &mut next_telemetry => {
                let samples = if matches!(args.scenario, Scenario::Burst) { 5 } else { 1 };
                for _ in 0..samples {
                    advance_device(&mut device, args.scenario);
                    if let Err(e) = publish_telemetry(&device, args.scenario, args.sensor_noise_percent, &session).await {
                        metrics.telemetry_failed.fetch_add(1, Ordering::Relaxed);
                        warn!("{} failed telemetry: {e}", device.id);
                    } else {
                        metrics.telemetry_sent.fetch_add(1, Ordering::Relaxed);
                    }
                    if samples > 1 {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                }
                next_telemetry.as_mut().reset(
                    tokio::time::Instant::now() + jittered_delay(device.telemetry_period, args.jitter_percent),
                );
            }
        }
    }
}

async fn publish_heartbeat(
    device: &DeviceRuntime,
    session: &zenoh::Session,
    status: &str,
) -> Result<(), zenoh::Error> {
    let message = DeviceHeartbeat {
        device_id: device.id.clone(),
        timestamp: extrittio_sdk::time::now_millis(),
        status: status.to_string(),
        firmware: device.firmware.clone(),
        #[allow(clippy::cast_possible_wrap)]
        uptime_seconds: device.start.elapsed().as_secs() as i64,
    };

    session
        .put(topics::heartbeat(&device.id), message.encode_to_vec())
        .await
}

async fn publish_telemetry(
    device: &DeviceRuntime,
    scenario: Scenario,
    sensor_noise_percent: u8,
    session: &zenoh::Session,
) -> Result<(), zenoh::Error> {
    let mut metadata = HashMap::new();
    metadata.insert("simulated".to_string(), "true".to_string());
    metadata.insert(
        "scenario".to_string(),
        format!("{scenario:?}").to_lowercase(),
    );

    let has_location = matches!(scenario, Scenario::Mobile);
    let message = DeviceTelemetry {
        device_id: device.id.clone(),
        timestamp: extrittio_sdk::time::now_millis(),
        temperature: noisy_reading(
            device.sensor.temperature,
            sensor_noise_percent,
            0.3,
            15.0,
            95.0,
        ),
        humidity: noisy_reading(
            device.sensor.humidity,
            sensor_noise_percent,
            0.6,
            20.0,
            80.0,
        ),
        battery_level: noisy_reading(device.sensor.battery, sensor_noise_percent, 0.2, 0.0, 100.0),
        metadata,
        latitude: if has_location { device.latitude } else { 0.0 },
        longitude: if has_location { device.longitude } else { 0.0 },
        speed: if has_location {
            noisy_reading(device.speed, sensor_noise_percent, 0.4, 0.0, 60.0)
        } else {
            0.0
        },
        altitude: if has_location {
            noisy_reading(device.altitude, sensor_noise_percent, 0.8, -100.0, 5_000.0)
        } else {
            0.0
        },
        heading: if has_location { device.heading } else { 0.0 },
    };

    session
        .put(topics::telemetry(&device.id), message.encode_to_vec())
        .await
}

fn advance_device(device: &mut DeviceRuntime, scenario: Scenario) {
    match scenario {
        Scenario::Normal | Scenario::Flaky | Scenario::Burst => {
            let mut rng = rand::rng();
            device.sensor.step(
                rng.random_range(-0.5..=0.5),
                rng.random_range(-1.0..=1.0),
                rng.random_range(0.05..=0.15) * device.battery_drain_multiplier,
            );
        }
        Scenario::Hot => {
            let mut rng = rand::rng();
            device.sensor.step(
                0.0,
                rng.random_range(-1.0..=1.0),
                rng.random_range(0.05..=0.15) * device.battery_drain_multiplier,
            );
            device.sensor.temperature =
                (device.sensor.temperature + rng.random_range(0.2..=0.8)).clamp(15.0, 95.0);
        }
        Scenario::LowBattery => {
            let mut rng = rand::rng();
            device.sensor.step(
                rng.random_range(-0.5..=0.5),
                rng.random_range(-1.0..=1.0),
                rng.random_range(0.8..=2.0) * device.battery_drain_multiplier,
            );
        }
        Scenario::Mobile => {
            let mut rng = rand::rng();
            device.sensor.step(
                rng.random_range(-0.5..=0.5),
                rng.random_range(-1.0..=1.0),
                rng.random_range(0.05..=0.15) * device.battery_drain_multiplier,
            );
            device.heading = (device.heading + rng.random_range(-8.0..=8.0)).rem_euclid(360.0);
            let radians = f64::from(device.heading).to_radians();
            let movement = f64::from(device.speed) * 0.000006;
            device.latitude += radians.cos() * movement;
            device.longitude += radians.sin() * movement;
            device.altitude =
                (device.altitude + rng.random_range(-0.5..=0.5)).clamp(-100.0, 5_000.0);
        }
    }
}

fn build_device(args: &Args, index: u32) -> DeviceRuntime {
    let ordinal = index + 1;
    let mut rng = rand::rng();
    let mut sensor = SensorState::new();
    let offset = (index % 20) as f32 * 0.1;
    sensor.temperature =
        (sensor.temperature + offset + rng.random_range(-2.0..=2.0)).clamp(15.0, 30.0);
    sensor.humidity = (sensor.humidity + offset + rng.random_range(-5.0..=5.0)).clamp(20.0, 80.0);
    sensor.battery = rng.random_range(55.0..=100.0);

    let (latitude, longitude) = initial_location(args.location_area, index, &mut rng);

    DeviceRuntime {
        id: format!("{}-{ordinal:06}", args.prefix),
        firmware: args.firmware.clone(),
        sensor,
        start: Instant::now(),
        telemetry_period: varied_period(
            Duration::from_secs(args.telemetry_interval),
            args.device_interval_variance_percent,
        ),
        heartbeat_period: varied_period(
            Duration::from_secs(args.heartbeat_interval),
            args.device_interval_variance_percent,
        ),
        battery_drain_multiplier: rng.random_range(0.6..=1.6),
        latitude,
        longitude,
        heading: ((index % 360) as f32 + rng.random_range(-15.0..=15.0)).rem_euclid(360.0),
        speed: rng.random_range(4.0..=24.0),
        altitude: rng.random_range(60.0..=120.0),
    }
}

fn initial_location(area: LocationArea, index: u32, rng: &mut impl Rng) -> (f64, f64) {
    match area {
        LocationArea::Poland => {
            let center = POLISH_CITY_CENTERS[index as usize % POLISH_CITY_CENTERS.len()];
            (
                center.0 + rng.random_range(-0.28..=0.28),
                center.1 + rng.random_range(-0.38..=0.38),
            )
        }
        LocationArea::Warsaw => (
            52.2297 + f64::from(index % 50) * 0.001 + rng.random_range(-0.02..=0.02),
            21.0122 + f64::from(index / 50) * 0.001 + rng.random_range(-0.02..=0.02),
        ),
    }
}

async fn open_session(args: &Args) -> zenoh::Session {
    let mut config = zenoh::Config::default();

    if let Some(ref endpoint) = args.connect {
        config
            .insert_json5("connect/endpoints", &format!("[\"{endpoint}\"]"))
            .expect("failed to set Zenoh connect endpoint");
        config
            .insert_json5("scouting/multicast/enabled", "false")
            .expect("failed to disable Zenoh multicast scouting");
    }

    if let Some(ref ca_cert) = args.ca_cert {
        let ca_path = std::fs::canonicalize(ca_cert)
            .unwrap_or_else(|e| panic!("CA cert not found at '{ca_cert}': {e}"));
        config
            .insert_json5(
                "transport/link/tls/root_ca_certificate",
                &format!("\"{}\"", ca_path.display()),
            )
            .expect("failed to set TLS root CA");
    }

    if let Some(ref client_cert) = args.client_cert {
        let cert_path = std::fs::canonicalize(client_cert)
            .unwrap_or_else(|e| panic!("Client cert not found at '{client_cert}': {e}"));
        config
            .insert_json5(
                "transport/link/tls/connect_certificate",
                &format!("\"{}\"", cert_path.display()),
            )
            .expect("failed to set TLS client certificate");
    }

    if let Some(ref client_key) = args.client_key {
        let key_path = std::fs::canonicalize(client_key)
            .unwrap_or_else(|e| panic!("Client key not found at '{client_key}': {e}"));
        config
            .insert_json5(
                "transport/link/tls/connect_private_key",
                &format!("\"{}\"", key_path.display()),
            )
            .expect("failed to set TLS client private key");
    }

    zenoh::open(config)
        .await
        .expect("failed to open Zenoh session")
}

fn validate_args(args: &Args) {
    if args.count == 0 {
        eprintln!("--count must be greater than 0");
        std::process::exit(2);
    }
    if args.telemetry_interval == 0 {
        eprintln!("--telemetry-interval must be greater than 0");
        std::process::exit(2);
    }
    if args.heartbeat_interval == 0 {
        eprintln!("--heartbeat-interval must be greater than 0");
        std::process::exit(2);
    }
    if args.jitter_percent > 100 {
        eprintln!("--jitter-percent must be between 0 and 100");
        std::process::exit(2);
    }
    if args.device_interval_variance_percent > 90 {
        eprintln!("--device-interval-variance-percent must be between 0 and 90");
        std::process::exit(2);
    }
    if args.sensor_noise_percent > 100 {
        eprintln!("--sensor-noise-percent must be between 0 and 100");
        std::process::exit(2);
    }
    if args.stats_interval == 0 {
        eprintln!("--stats-interval must be greater than 0");
        std::process::exit(2);
    }
}

async fn report_stats(metrics: Arc<SimulatorMetrics>, device_count: u32, interval: Duration) {
    let mut previous = MetricsSnapshot::default();

    loop {
        tokio::time::sleep(interval).await;
        let current = metrics.snapshot();
        let seconds = interval.as_secs_f64();
        let telemetry_rate = (current.telemetry_sent - previous.telemetry_sent) as f64 / seconds;
        let heartbeat_rate = (current.heartbeats_sent - previous.heartbeats_sent) as f64 / seconds;

        info!(
            "stats: devices_started={}/{} telemetry_sent={} ({:.1}/s) heartbeats_sent={} ({:.1}/s) telemetry_failed={} heartbeats_failed={} flaky_heartbeats_skipped={}",
            current.devices_started,
            device_count,
            current.telemetry_sent,
            telemetry_rate,
            current.heartbeats_sent,
            heartbeat_rate,
            current.telemetry_failed,
            current.heartbeats_failed,
            current.flaky_heartbeats_skipped,
        );

        previous = current;
    }
}

impl SimulatorMetrics {
    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            devices_started: self.devices_started.load(Ordering::Relaxed),
            heartbeats_sent: self.heartbeats_sent.load(Ordering::Relaxed),
            heartbeats_failed: self.heartbeats_failed.load(Ordering::Relaxed),
            telemetry_sent: self.telemetry_sent.load(Ordering::Relaxed),
            telemetry_failed: self.telemetry_failed.load(Ordering::Relaxed),
            flaky_heartbeats_skipped: self.flaky_heartbeats_skipped.load(Ordering::Relaxed),
        }
    }
}

async fn sleep_random(max: Duration) {
    if max.is_zero() {
        return;
    }
    let max_ms = max.as_millis();
    let Ok(max_ms) = u64::try_from(max_ms) else {
        return;
    };
    let delay = {
        let mut rng = rand::rng();
        rng.random_range(0..=max_ms)
    };
    tokio::time::sleep(Duration::from_millis(delay)).await;
}

fn should_skip_flaky_heartbeat(scenario: Scenario) -> bool {
    matches!(scenario, Scenario::Flaky) && {
        let mut rng = rand::rng();
        rng.random_bool(0.2)
    }
}

fn varied_period(base: Duration, percent: u8) -> Duration {
    if percent == 0 {
        return base;
    }
    let factor = random_factor(percent);
    duration_mul(base, factor).max(Duration::from_millis(100))
}

fn jittered_delay(base: Duration, percent: u8) -> Duration {
    if percent == 0 {
        return base;
    }
    let factor = random_factor(percent);
    duration_mul(base, factor).max(Duration::from_millis(25))
}

fn random_factor(percent: u8) -> f64 {
    let spread = f64::from(percent) / 100.0;
    let mut rng = rand::rng();
    rng.random_range(1.0 - spread..=1.0 + spread)
}

fn duration_mul(duration: Duration, factor: f64) -> Duration {
    Duration::from_secs_f64(duration.as_secs_f64() * factor.max(0.01))
}

fn noisy_reading(value: f32, percent: u8, floor_noise: f32, min: f32, max: f32) -> f32 {
    if percent == 0 {
        return value.clamp(min, max);
    }
    let mut rng = rand::rng();
    let spread = (value.abs() * f32::from(percent) / 100.0).max(floor_noise);
    (value + rng.random_range(-spread..=spread)).clamp(min, max)
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let Some(unit) = value.chars().last() else {
        return Err("duration cannot be empty".to_string());
    };
    let number = &value[..value.len() - unit.len_utf8()];
    let amount = number
        .parse::<u64>()
        .map_err(|_| "duration amount must be a positive integer".to_string())?;

    match unit {
        's' | 'S' => Ok(Duration::from_secs(amount)),
        'm' | 'M' => Ok(Duration::from_secs(amount.saturating_mul(60))),
        'h' | 'H' => Ok(Duration::from_secs(amount.saturating_mul(60 * 60))),
        _ => Err("duration must end with s, m, or h".to_string()),
    }
}
