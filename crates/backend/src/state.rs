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
    /// Paired repository/lifecycle construction result, consumed once into
    /// applications and operational state without retaining business ports.
    pub database: crate::persistence::DatabaseComposition,
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
    pub firmware_store: crate::domains::firmware_store::FirmwareObjectStore,
    pub readiness: Arc<ReadinessRegistry>,
    /// Host-local Thread runtime. It can rediscover an RCP connected after
    /// Extrittio Edge has started.
    pub thread_runtime: Option<Arc<extrittio_openthread_runtime::ThreadRuntime>>,
}

/// Lifecycle and diagnostics shared by operational routes and the supervisor.
/// Business repository ports are never retained here.
pub(crate) struct OperationalState {
    database: crate::persistence::DatabaseRuntime,
    health_token: Option<String>,
    readiness: Arc<ReadinessRegistry>,
    thread_runtime: Option<Arc<extrittio_openthread_runtime::ThreadRuntime>>,
}

impl OperationalState {
    pub(crate) fn database(&self) -> &crate::persistence::DatabaseRuntime {
        &self.database
    }
    pub(crate) fn readiness(&self) -> &Arc<ReadinessRegistry> {
        &self.readiness
    }
    pub(crate) fn thread_runtime(
        &self,
    ) -> &Option<Arc<extrittio_openthread_runtime::ThreadRuntime>> {
        &self.thread_runtime
    }
    pub(crate) fn health_token(&self) -> Option<&str> {
        self.health_token.as_deref()
    }
}

pub(crate) struct HttpState {
    jwt_secret: String,
    public_url: String,
    cookie_secure: bool,
    api_rate_limiter: RateLimiter,
    login_rate_limiter: RateLimiter,
    trusted_proxies: Vec<TrustedProxy>,
    ci_rate_limiter: ApiKeyRateLimiter,
}

impl HttpState {
    pub(crate) fn jwt_secret(&self) -> &String {
        &self.jwt_secret
    }
    pub(crate) fn public_url(&self) -> &String {
        &self.public_url
    }
    pub(crate) fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }
    pub(crate) fn api_rate_limiter(&self) -> &RateLimiter {
        &self.api_rate_limiter
    }
    pub(crate) fn login_rate_limiter(&self) -> &RateLimiter {
        &self.login_rate_limiter
    }
    pub(crate) fn trusted_proxies(&self) -> &Vec<TrustedProxy> {
        &self.trusted_proxies
    }
    pub(crate) fn ci_rate_limiter(&self) -> &ApiKeyRateLimiter {
        &self.ci_rate_limiter
    }
}

pub(crate) struct MessagingState {
    zenoh_session: Arc<zenoh::Session>,
    zenoh_tls_enabled: bool,
    zenoh_port: u16,
    zenoh_metrics: Arc<ZenohMetrics>,
}

impl MessagingState {
    pub(crate) fn zenoh_session(&self) -> &Arc<zenoh::Session> {
        &self.zenoh_session
    }
    pub(crate) fn zenoh_tls_enabled(&self) -> bool {
        self.zenoh_tls_enabled
    }
    pub(crate) fn zenoh_port(&self) -> u16 {
        self.zenoh_port
    }
    pub(crate) fn zenoh_metrics(&self) -> &Arc<ZenohMetrics> {
        &self.zenoh_metrics
    }
}

pub(crate) struct ObservabilityState {
    metrics_accumulator: MetricsAccumulator,
}

impl ObservabilityState {
    pub(crate) fn metrics_accumulator(&self) -> &MetricsAccumulator {
        &self.metrics_accumulator
    }
}

pub struct AppState {
    observability: ObservabilityState,
    messaging: MessagingState,
    http: HttpState,
    runtime: OperationalState,
    application: extrittio_backend_core::Application,
    workers: crate::app::workers::WorkerApplications,
    rule_cache: Arc<crate::rule_snapshots::RuleSnapshotStore>,
    firmware_store: crate::domains::firmware_store::FirmwareObjectStore,
}

impl AppState {
    #[must_use]
    pub fn new(input: AppStateInput) -> Self {
        let (persistence, database) = input.database.into_parts();
        let crypto = Arc::new(crate::outbound::certificates::CertificateCrypto::new(
            crate::config::certificate_encryption_secret(),
        ));
        let application = extrittio_backend_core::Application::new(
            extrittio_backend_core::RepositorySet::new(persistence.clone()),
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

        let workers = crate::app::workers::WorkerApplications::new(
            &persistence,
            &input.zenoh_session,
            &input.zenoh_metrics,
        );
        Self {
            messaging: MessagingState {
                zenoh_session: input.zenoh_session,
                zenoh_tls_enabled: input.zenoh_tls_enabled,
                zenoh_port: input.zenoh_port,
                zenoh_metrics: input.zenoh_metrics,
            },
            observability: ObservabilityState {
                metrics_accumulator: input.metrics_accumulator,
            },
            http: HttpState {
                jwt_secret: input.jwt_secret,
                public_url: input.public_url,
                cookie_secure: input.cookie_secure,
                api_rate_limiter: input.api_rate_limiter,
                login_rate_limiter: input.login_rate_limiter,
                trusted_proxies: input.trusted_proxies,
                ci_rate_limiter: input.ci_rate_limiter,
            },
            application,
            workers,
            runtime: OperationalState {
                database,
                readiness: input.readiness,
                thread_runtime: input.thread_runtime,
                health_token: input.health_token,
            },
            rule_cache: input.rule_cache,
            firmware_store: input.firmware_store,
        }
    }

    pub(crate) fn http(&self) -> &HttpState {
        &self.http
    }

    pub(crate) fn messaging(&self) -> &MessagingState {
        &self.messaging
    }

    pub(crate) fn observability(&self) -> &ObservabilityState {
        &self.observability
    }

    pub(crate) fn workers(&self) -> &crate::app::workers::WorkerApplications {
        &self.workers
    }

    pub(crate) fn rule_cache(&self) -> &Arc<crate::rule_snapshots::RuleSnapshotStore> {
        &self.rule_cache
    }

    pub(crate) fn firmware_store(&self) -> &crate::domains::firmware_store::FirmwareObjectStore {
        &self.firmware_store
    }

    pub(crate) fn runtime(&self) -> &OperationalState {
        &self.runtime
    }

    /// Application boundary for transport handlers.
    #[must_use]
    pub fn application(&self) -> &extrittio_backend_core::Application {
        &self.application
    }
}
