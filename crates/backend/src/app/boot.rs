use std::sync::{Arc, RwLock};

use anyhow::Context;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::domains::firmware_store::FirmwareObjectStore;
use crate::init;
use crate::rate_limit::{ApiKeyRateLimiter, RateLimiter, parse_trusted_proxies};
use crate::services;
use crate::state::{AppState, AppStateInput, MetricsAccumulator, ReadinessRegistry, ZenohMetrics};

/// Initialize infrastructure and shared application state.
pub async fn initialize_state(
    config: &AppConfig,
    thread_runtime: Option<Arc<extrittio_openthread_runtime::ThreadRuntime>>,
) -> anyhow::Result<Arc<AppState>> {
    init::install_crypto_provider();
    let database = crate::persistence::factory::create(&config.database).await?;
    let persistence = database.repositories().clone();
    info!(
        backend = database.descriptor().kind.as_str(),
        "Database opened"
    );
    if database.descriptor().kind == crate::persistence::BackendKind::Turso {
        let database_path_label = database
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

    init::run_database_migrations(&database).await?;
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
    init::seed_persistence_device_types(&persistence).await?;
    let jwt_secret = init::init_persistence_jwt_secret(&persistence).await?;
    init::seed_persistence_admin_user(&persistence).await?;
    init::init_persistence_ca_certificate(&persistence).await?;
    services::cert_service::encrypt_stored_private_keys(persistence.certificates.as_ref())
        .await
        .context("Failed to encrypt stored certificate private keys")?;
    init::write_persistence_tls_certs(&persistence, &config.certs_dir).await?;
    let device_certificate_ids = persistence
        .certificates
        .list_active_device_ids(chrono::Utc::now())
        .await
        .context("Failed to load active device certificate IDs for Zenoh ACL")?;

    let rule_cache = services::rule_service::build_cache_with_repositories(
        persistence.rules.as_ref(),
        persistence.rule_zone_snapshots.as_ref(),
    )
    .await
    .context("Failed to build initial rule cache")?;
    let rule_cache = Arc::new(RwLock::new(rule_cache));

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("Failed to create HTTP client")?;

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
        http_client,
        firmware_store,
        readiness: Arc::new(ReadinessRegistry::new(true, true)),
        thread_runtime,
    })))
}
