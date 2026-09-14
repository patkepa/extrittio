use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::rate_limit::{ApiKeyRateLimiter, RateLimiter, TrustedProxy};

const MAX_LATENCY_SAMPLES_PER_FLUSH: usize = 10_000;

#[derive(Debug, Clone)]
pub struct ReadinessSnapshot {
    pub migrations_ready: bool,
    pub zenoh_ready: bool,
    pub workers: BTreeMap<String, bool>,
}

/// Shared readiness state for dependencies that cannot be checked solely by
/// opening a database connection at probe time.
pub struct ReadinessRegistry {
    migrations_ready: std::sync::atomic::AtomicBool,
    zenoh_ready: std::sync::atomic::AtomicBool,
    workers: RwLock<BTreeMap<String, bool>>,
}

impl ReadinessRegistry {
    pub fn new(migrations_ready: bool, zenoh_ready: bool) -> Self {
        Self {
            migrations_ready: std::sync::atomic::AtomicBool::new(migrations_ready),
            zenoh_ready: std::sync::atomic::AtomicBool::new(zenoh_ready),
            workers: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn register_worker(&self, name: &str) {
        self.set_worker(name, true);
    }

    pub fn set_worker(&self, name: &str, ready: bool) {
        let mut workers = self
            .workers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        workers.insert(name.to_string(), ready);
    }

    #[must_use]
    pub fn snapshot(&self) -> ReadinessSnapshot {
        let workers = self
            .workers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        ReadinessSnapshot {
            migrations_ready: self.migrations_ready.load(Ordering::Acquire),
            zenoh_ready: self.zenoh_ready.load(Ordering::Acquire),
            workers,
        }
    }
}

/// Tracks Zenoh message counts. Shared between subscriber, services, and metrics flush.
pub struct ZenohMetrics {
    pub messages_in: AtomicU64,
    pub messages_out: AtomicU64,
}

impl ZenohMetrics {
    pub fn new() -> Self {
        Self {
            messages_in: AtomicU64::new(0),
            messages_out: AtomicU64::new(0),
        }
    }
}

impl Default for ZenohMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulates HTTP request metrics between flush intervals.
pub struct MetricsAccumulator {
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub latency_sum_micros: AtomicU64,
    pub latency_samples: Mutex<Vec<u64>>,
}

impl MetricsAccumulator {
    pub fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            latency_sum_micros: AtomicU64::new(0),
            latency_samples: Mutex::new(Vec::new()),
        }
    }

    /// Record a completed request. Called from the metrics middleware.
    pub fn record(&self, latency_micros: u64, is_error: bool) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        self.latency_sum_micros
            .fetch_add(latency_micros, Ordering::Relaxed);
        if is_error {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut samples) = self.latency_samples.try_lock()
            && samples.len() < MAX_LATENCY_SAMPLES_PER_FLUSH
        {
            samples.push(latency_micros);
        }
    }

    /// Drain all accumulated metrics. Mutex first, then atomics for consistency.
    pub fn drain(&self) -> (u64, u64, u64, Vec<u64>) {
        let samples = match self.latency_samples.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(poisoned) => {
                tracing::warn!("Metrics latency mutex poisoned; recovering samples");
                std::mem::take(&mut *poisoned.into_inner())
            }
        };
        let req = self.request_count.swap(0, Ordering::Relaxed);
        let err = self.error_count.swap(0, Ordering::Relaxed);
        let lat = self.latency_sum_micros.swap(0, Ordering::Relaxed);
        (req, err, lat, samples)
    }
}

impl Default for MetricsAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Explicit construction boundary for the host's shared runtime state.
///
/// Runtime consumers receive [`AppState`], whose fields are not part of the
/// public API. Boot code and external integration tests use this input instead
/// of coupling themselves to the state container's internal layout.
pub struct AppStateInput {
    /// Sole persistence composition source. `AppState::new` derives both the
    /// temporary legacy repository bridge and the core application ports from
    /// this runtime, preventing mismatched handles.
    pub database: crate::persistence::DatabaseRuntime,
    pub zenoh_session: Arc<zenoh::Session>,
    pub zenoh_tls_enabled: bool,
    pub zenoh_port: u16,
    pub jwt_secret: String,
    pub public_url: String,
    pub cookie_secure: bool,
    pub health_token: Option<String>,
    pub api_rate_limiter: RateLimiter,
    pub login_rate_limiter: RateLimiter,
    pub trusted_proxies: Vec<TrustedProxy>,
    pub ci_rate_limiter: ApiKeyRateLimiter,
    pub metrics_accumulator: MetricsAccumulator,
    pub zenoh_metrics: Arc<ZenohMetrics>,
    pub rule_cache: Arc<crate::rule_snapshots::RuleSnapshotStore>,
    pub http_client: reqwest::Client,
    pub firmware_store: crate::domains::firmware_store::FirmwareObjectStore,
    pub readiness: Arc<ReadinessRegistry>,
    /// Host-local Thread runtime. It can rediscover an RCP connected after
    /// Extrittio Edge has started.
    pub thread_runtime: Option<Arc<extrittio_openthread_runtime::ThreadRuntime>>,
}

pub struct AppState {
    application: extrittio_backend_core::Application,
    /// Temporary legacy repository escape hatch. Existing host handlers use
    /// it while their vertical slices move behind `Application`.
    pub(crate) persistence: crate::persistence::RepositorySet,
    pub(crate) database: crate::persistence::DatabaseRuntime,
    pub(crate) zenoh_session: Arc<zenoh::Session>,
    pub(crate) zenoh_tls_enabled: bool,
    pub(crate) zenoh_port: u16,
    pub(crate) jwt_secret: String,
    pub(crate) public_url: String,
    pub(crate) cookie_secure: bool,
    pub(crate) health_token: Option<String>,
    pub(crate) api_rate_limiter: RateLimiter,
    pub(crate) login_rate_limiter: RateLimiter,
    pub(crate) trusted_proxies: Vec<TrustedProxy>,
    pub(crate) ci_rate_limiter: ApiKeyRateLimiter,
    pub(crate) metrics_accumulator: MetricsAccumulator,
    pub(crate) zenoh_metrics: Arc<ZenohMetrics>,
    pub(crate) rule_cache: Arc<crate::rule_snapshots::RuleSnapshotStore>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) firmware_store: crate::domains::firmware_store::FirmwareObjectStore,
    pub(crate) readiness: Arc<ReadinessRegistry>,
    pub(crate) thread_runtime: Option<Arc<extrittio_openthread_runtime::ThreadRuntime>>,
}

impl AppState {
    #[must_use]
    pub fn new(input: AppStateInput) -> Self {
        let persistence = input.database.repositories().clone();
        let crypto = Arc::new(crate::outbound::certificates::CertificateCrypto::new(
            crate::config::certificate_encryption_secret(),
        ));
        let application = extrittio_backend_core::Application::new(
            extrittio_backend_core::RepositorySet::new(
                extrittio_backend_core::RepositorySetInput {
                    audit: persistence.audit.clone(),
                    analytics: persistence.analytics.clone(),
                    dashboard: persistence.dashboard.clone(),
                    activity: persistence.activity.clone(),
                    firmware: persistence.firmware.clone(),
                    telemetry: persistence.telemetry.clone(),
                    events: persistence.events.clone(),
                    logs: persistence.logs.clone(),
                    commands: persistence.commands.clone(),
                    configuration: persistence.configuration.clone(),
                    shadows: persistence.shadows.clone(),
                    alerts: persistence.alerts.clone(),
                    outbox: persistence.outbox.clone(),
                    rules: persistence.rules.clone(),
                    devices: persistence.devices.clone(),
                    device_blueprints: persistence.device_blueprints.clone(),
                    api_keys: persistence.api_keys.clone(),
                    fleets: persistence.fleets.clone(),
                    device_types: persistence.device_types.clone(),
                    ci_ingest: persistence.ci_ingest.clone(),
                    certificates: persistence.certificates.clone(),
                    roles: persistence.roles.clone(),
                    users: persistence.users.clone(),
                    zones: persistence.zones.clone(),
                },
            ),
            extrittio_backend_core::ApplicationDependencies::new(
                Arc::new(crate::auth::Argon2PasswordHasher),
                Arc::new(crate::auth::SystemClock),
                Arc::new(crate::api_key_util::RandomApiKeyGenerator),
                crypto.clone(),
                crypto,
                Arc::new(crate::security::PublicWebhookUrlPolicy),
                input.rule_cache.clone(),
                Arc::new(crate::outbound::device_bus::ZenohDeviceBus::new(
                    input.zenoh_session.clone(),
                    input.zenoh_metrics.clone(),
                )),
            ),
        );

        Self {
            application,
            persistence,
            database: input.database,
            zenoh_session: input.zenoh_session,
            zenoh_tls_enabled: input.zenoh_tls_enabled,
            zenoh_port: input.zenoh_port,
            jwt_secret: input.jwt_secret,
            public_url: input.public_url,
            cookie_secure: input.cookie_secure,
            health_token: input.health_token,
            api_rate_limiter: input.api_rate_limiter,
            login_rate_limiter: input.login_rate_limiter,
            trusted_proxies: input.trusted_proxies,
            ci_rate_limiter: input.ci_rate_limiter,
            metrics_accumulator: input.metrics_accumulator,
            zenoh_metrics: input.zenoh_metrics,
            rule_cache: input.rule_cache,
            http_client: input.http_client,
            firmware_store: input.firmware_store,
            readiness: input.readiness,
            thread_runtime: input.thread_runtime,
        }
    }

    /// Curated application boundary for transport handlers. More use cases are
    /// added here as their vertical slices leave the legacy persistence host.
    #[must_use]
    pub fn application(&self) -> &extrittio_backend_core::Application {
        &self.application
    }
}
