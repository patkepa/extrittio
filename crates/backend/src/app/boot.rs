use std::sync::Arc;

use anyhow::Context;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::domains::firmware_store::FirmwareObjectStore;
use crate::init;
use crate::rate_limit::{ApiKeyRateLimiter, RateLimiter, parse_trusted_proxies};
use crate::state::{AppState, AppStateInput, MetricsAccumulator, ReadinessRegistry, ZenohMetrics};

/// Initialize infrastructure and shared application state.
pub async fn initialize_state(
    config: &AppConfig,
    thread_runtime: Option<crate::service::ThreadHandle>,
) -> anyhow::Result<Arc<AppState>> {
    let thread_runtime = thread_runtime.map(|handle| handle.0);
    init::install_crypto_provider();
    let database = crate::persistence::factory::create(&config.database).await?;
    let persistence = database.repositories().clone();
    info!(
        backend = database.runtime().descriptor().kind.as_str(),
        "Database opened"
    );
    if database.runtime().descriptor().kind == crate::persistence::BackendKind::Turso {
        let database_path_label = database
            .runtime()
            .descriptor()
            .local_file
            .as_deref()
            .and_then(std::path::Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("local database");
        info!(
            database = database_path_label,
            "Embedded Turso backend enabled: single-node Extrittio Edge deployment"
        );
    }

    init::run_database_migrations(database.runtime()).await?;
    if let crate::config::DatabaseConfig::Turso {
        database_path,
        size_warning_bytes,
        ..
    } = &config.database
        && let Ok(metadata) = std::fs::metadata(database_path)
        && metadata.len() >= *size_warning_bytes
    {
        warn!(
            database_size_bytes = metadata.len(),
            warning_threshold_bytes = size_warning_bytes,
            "Embedded Turso database has reached its configured size warning threshold"
        );
    }
    let jwt_secret = init::init_persistence_jwt_secret(&persistence).await?;
    init::seed_persistence_admin_user(&persistence).await?;
    init::init_persistence_ca_certificate(&persistence).await?;
    init::certificate_system(&persistence)
        .encrypt_stored_private_keys()
        .await
        .context("Failed to encrypt stored certificate private keys")?;
    init::write_persistence_tls_certs(&persistence, &config.certs_dir).await?;
    let device_certificate_ids = init::certificate_system(&persistence)
        .active_device_ids(chrono::Utc::now())
        .await
        .context("Failed to load active device certificate IDs for Zenoh ACL")?;

    let rule_cache = crate::rule_snapshots::RuleSnapshotStore::initialize(
        persistence.rules.clone(),
        std::time::Duration::from_secs(config.rule_snapshot_refresh_interval_secs),
    )
    .await
    .context("Failed to build initial rule snapshot")?;

    let firmware_store = FirmwareObjectStore::from_config(&config.firmware_storage)
        .context("Failed to initialize firmware object storage")?;
    firmware_store
        .verify()
        .await
        .context("Firmware object storage readiness probe failed")?;
    info!(
        backend = firmware_store.backend(),
        "Firmware object store ready"
    );

    let zenoh_session = Arc::new(
        init::open_zenoh_session(
            config.zenoh_tls_enabled,
            config.zenoh_tls_port,
            &config.zenoh_listen_host,
            &config.certs_dir,
            config.zenoh_cert_acl_enabled,
            &device_certificate_ids,
        )
        .await?,
    );
    info!("Zenoh session opened");

    let zenoh_metrics = Arc::new(ZenohMetrics::new());

    Ok(Arc::new(AppState::new(AppStateInput {
        database,
        zenoh_session,
        zenoh_tls_enabled: config.zenoh_tls_enabled,
        zenoh_port: config.zenoh_tls_port,
        jwt_secret,
        public_url: config.public_url.clone(),
        cookie_secure: config.cookie_secure,
        health_token: config.health_token.clone(),
        api_rate_limiter: RateLimiter::new(100, 60),
        login_rate_limiter: RateLimiter::new(5, 60),
        trusted_proxies: parse_trusted_proxies(&config.trusted_proxies),
        ci_rate_limiter: ApiKeyRateLimiter::new(60, 60),
        metrics_accumulator: MetricsAccumulator::new(),
        zenoh_metrics,
        rule_cache,
        firmware_store,
        readiness: Arc::new(ReadinessRegistry::new(true, true)),
        thread_runtime,
    })))
}
