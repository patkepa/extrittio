use std::sync::{Arc, RwLock};

use anyhow::Context;
use tracing::info;

use crate::config::AppConfig;
use crate::init;
use crate::rate_limit::{ApiKeyRateLimiter, RateLimiter, parse_trusted_proxies};
use crate::services;
use crate::state::{AppState, MetricsAccumulator, ZenohMetrics};

/// Initialize infrastructure and shared application state.
pub async fn initialize_state(config: &AppConfig) -> anyhow::Result<Arc<AppState>> {
    let db_pool = init::create_db_pool(&config.database_url, config.db_pool_size)?;
    info!("DB connection pool: max_size={}", config.db_pool_size);

    let jwt_secret = {
        let mut conn = db_pool
            .get()
            .context("Failed to get DB connection for initialization")?;
        init::run_migrations(&mut conn)?;
        init::seed_default_device_types(&mut conn)?;
        let secret = init::init_jwt_secret(&mut conn)?;
        init::seed_admin_user(&mut conn)?;
        init::init_ca_certificate(&mut conn)?;
        services::cert_service::encrypt_stored_private_keys(&mut conn)
            .context("Failed to encrypt stored certificate private keys")?;
        init::write_tls_certs(&mut conn, &config.certs_dir)?;
        secret
    };

    let rule_cache = {
        let mut conn = db_pool
            .get()
            .context("Failed to get DB connection for rule cache")?;
        services::rule_service::build_cache(&mut conn)
            .context("Failed to build initial rule cache")?
    };
    let rule_cache = Arc::new(RwLock::new(rule_cache));

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("Failed to create HTTP client")?;

    let zenoh_session = Arc::new(
        init::open_zenoh_session(
            config.zenoh_tls_enabled,
            config.zenoh_tls_port,
            &config.zenoh_listen_host,
            &config.certs_dir,
        )
        .await?,
    );
    info!("Zenoh session opened");

    let zenoh_metrics = Arc::new(ZenohMetrics::new());

    Ok(Arc::new(AppState {
        db_pool,
        zenoh_session,
        jwt_secret,
        public_url: config.public_url.clone(),
        cookie_secure: config.cookie_secure,
        api_rate_limiter: RateLimiter::new(100, 60),
        login_rate_limiter: RateLimiter::new(5, 60),
        trusted_proxies: parse_trusted_proxies(&config.trusted_proxies),
        ci_rate_limiter: ApiKeyRateLimiter::new(60, 60),
        metrics_accumulator: MetricsAccumulator::new(),
        zenoh_metrics,
        rule_cache,
        http_client,
    }))
}
