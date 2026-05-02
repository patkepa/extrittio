use diesel::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::AppError;
use crate::rate_limit::{ApiKeyRateLimiter, RateLimiter};
use crate::rule_engine::cache::RuleCache;

pub type DbPool = Pool<ConnectionManager<PgConnection>>;

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
        if let Ok(mut samples) = self.latency_samples.lock() {
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

pub struct AppState {
    pub db_pool: DbPool,
    pub zenoh_session: Arc<zenoh::Session>,
    pub jwt_secret: String,
    pub public_url: String,
    pub api_rate_limiter: RateLimiter,
    pub login_rate_limiter: RateLimiter,
    pub ci_rate_limiter: ApiKeyRateLimiter,
    pub metrics_accumulator: MetricsAccumulator,
    pub zenoh_metrics: Arc<ZenohMetrics>,
    pub rule_cache: Arc<RwLock<RuleCache>>,
    pub http_client: reqwest::Client,
}

/// Run a synchronous DB operation on a blocking thread to avoid starving the
/// Tokio runtime. Acquires a pooled connection, passes it to the closure, and
/// returns the result.
pub async fn run_db<F, T>(pool: &DbPool, f: F) -> Result<T, AppError>
where
    F: FnOnce(&mut PgConnection) -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        f(&mut conn)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task join error: {e}")))?
}
