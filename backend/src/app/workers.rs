use std::sync::Arc;

use crate::config::AppConfig;
use crate::state::AppState;
use crate::{background, services, zenoh_handler};

/// Spawn long-running background workers that share application state.
pub fn spawn_background_tasks(config: &AppConfig, state: Arc<AppState>) {
    let subscriber_pool = state.db_pool.clone();
    let subscriber_session = state.zenoh_session.clone();
    let subscriber_metrics = state.zenoh_metrics.clone();
    let sub_cache = state.rule_cache.clone();
    let sub_client = state.http_client.clone();
    tokio::spawn(async move {
        if let Err(e) = zenoh_handler::subscriber::run_subscriber(
            subscriber_session,
            subscriber_pool,
            subscriber_metrics,
            sub_cache,
            sub_client,
        )
        .await
        {
            tracing::error!("Zenoh subscriber failed: {}. Shutting down.", e);
            std::process::exit(1);
        }
    });

    tokio::spawn(services::server_metrics::run_system_metrics_collector(
        state.db_pool.clone(),
    ));
    tokio::spawn(services::server_metrics::run_app_metrics_flusher(
        state.clone(),
    ));
    tokio::spawn(services::server_metrics::run_metrics_retention(
        state.db_pool.clone(),
    ));

    let checker_pool = state.db_pool.clone();
    let checker_cache = state.rule_cache.clone();
    let checker_client = state.http_client.clone();
    let checker_session = state.zenoh_session.clone();
    let checker_metrics = state.zenoh_metrics.clone();
    let offline_timeout = config.offline_timeout_secs;
    tokio::spawn(async move {
        background::run_offline_checker(
            checker_pool,
            offline_timeout,
            checker_cache,
            checker_client,
            checker_session,
            checker_metrics,
        )
        .await;
    });

    let retention_pool = state.db_pool.clone();
    let retention_days = config.alert_retention_days;
    tokio::spawn(async move {
        background::run_alert_retention(retention_pool, retention_days).await;
    });

    let cmd_timeout_pool = state.db_pool.clone();
    let cmd_timeout = config.command_timeout_secs;
    tokio::spawn(async move {
        background::run_command_timeout_checker(cmd_timeout_pool, cmd_timeout).await;
    });
}
