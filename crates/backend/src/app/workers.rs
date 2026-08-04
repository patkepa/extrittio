use std::future::Future;
use std::sync::Arc;

use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::state::{AppState, ReadinessRegistry};
use crate::{background, services, zenoh_handler};

struct WorkerExit {
    name: &'static str,
    result: Result<(), String>,
}

/// Owns the process-wide worker tree and coordinates it with HTTP shutdown.
pub struct WorkerSupervisor {
    cancellation: CancellationToken,
    monitor: JoinHandle<anyhow::Result<()>>,
}

impl WorkerSupervisor {
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        self.monitor
            .await
            .map_err(|error| anyhow::anyhow!("worker supervisor task panicked: {error}"))?
    }
}

fn spawn_worker<F>(
    workers: &mut JoinSet<WorkerExit>,
    cancellation: &CancellationToken,
    readiness: &Arc<ReadinessRegistry>,
    name: &'static str,
    worker: F,
) where
    F: Future<Output = Result<(), String>> + Send + 'static,
{
    readiness.register_worker(name);
    let cancellation = cancellation.clone();
    workers.spawn(async move {
        tokio::select! {
            () = cancellation.cancelled() => WorkerExit { name, result: Ok(()) },
            result = worker => WorkerExit { name, result },
        }
    });
}

/// Start all long-running tasks under one monitored worker tree.
pub fn spawn_background_tasks(config: &AppConfig, state: Arc<AppState>) -> WorkerSupervisor {
    let cancellation = CancellationToken::new();
    let mut workers = JoinSet::new();

    let subscriber_pool = state.db_pool.clone();
    let subscriber_session = state.zenoh_session.clone();
    let subscriber_metrics = state.zenoh_metrics.clone();
    let sub_cache = state.rule_cache.clone();
    let max_zenoh_payload_size_bytes = config.max_zenoh_payload_size_bytes;
    let auto_register_devices = config.auto_register_devices;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "zenoh-subscriber",
        async move {
            zenoh_handler::subscriber::run_subscriber(
                subscriber_session,
                subscriber_pool,
                subscriber_metrics,
                sub_cache,
                max_zenoh_payload_size_bytes,
                auto_register_devices,
            )
            .await
            .map_err(|error| error.to_string())
        },
    );

    let firmware_readiness_store = state.firmware_store.clone();
    let firmware_readiness = state.readiness.clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "firmware-object-store",
        async move {
            crate::domains::firmware_store::run_readiness_monitor(
                firmware_readiness_store,
                firmware_readiness,
            )
            .await;
            Ok(())
        },
    );

    let firmware_migration_pool = state.db_pool.clone();
    let firmware_store = state.firmware_store.clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "firmware-object-migrator",
        async move {
            crate::domains::firmware_store::run_legacy_blob_migrator(
                firmware_migration_pool,
                firmware_store,
            )
            .await;
            Ok(())
        },
    );

    let outbox_pool = state.db_pool.clone();
    let outbox_cache = state.rule_cache.clone();
    let outbox_client = state.http_client.clone();
    let outbox_session = state.zenoh_session.clone();
    let outbox_metrics = state.zenoh_metrics.clone();
    let outbox_config = crate::rule_engine::actions::OutboxWorkerConfig {
        batch_size: config.outbox_batch_size,
        concurrency: config.outbox_concurrency,
        idle_interval: std::time::Duration::from_millis(config.outbox_idle_interval_ms),
        lease_timeout: std::time::Duration::from_secs(config.outbox_lease_timeout_secs),
    };
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "rule-action-outbox",
        async move {
            crate::rule_engine::actions::run_rule_action_outbox_worker(
                outbox_pool,
                outbox_cache,
                outbox_client,
                outbox_session,
                outbox_metrics,
                outbox_config,
            )
            .await;
            Ok(())
        },
    );

    let metrics_pool = state.db_pool.clone();
    let metrics_interval = config.system_metrics_interval_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "system-metrics",
        async move {
            services::server_metrics::run_system_metrics_collector(metrics_pool, metrics_interval)
                .await;
            Ok(())
        },
    );

    let app_metrics_state = state.clone();
    let app_metrics_interval = config.app_metrics_flush_interval_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "app-metrics",
        async move {
            services::server_metrics::run_app_metrics_flusher(
                app_metrics_state,
                app_metrics_interval,
            )
            .await;
            Ok(())
        },
    );

    let metrics_retention_pool = state.db_pool.clone();
    let metrics_retention_hours = config.metrics_retention_hours;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "metrics-retention",
        async move {
            services::server_metrics::run_metrics_retention(
                metrics_retention_pool,
                metrics_retention_hours,
            )
            .await;
            Ok(())
        },
    );

    let checker_pool = state.db_pool.clone();
    let checker_cache = state.rule_cache.clone();
    let offline_timeout = config.offline_timeout_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "offline-checker",
        async move {
            background::run_offline_checker(checker_pool, offline_timeout, checker_cache).await;
            Ok(())
        },
    );

    let retention_pool = state.db_pool.clone();
    let retention_days = config.alert_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "alert-retention",
        async move {
            background::run_alert_retention(retention_pool, retention_days).await;
            Ok(())
        },
    );

    let log_retention_pool = state.db_pool.clone();
    let log_retention_days = config.log_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "log-retention",
        async move {
            background::run_log_retention(log_retention_pool, log_retention_days).await;
            Ok(())
        },
    );

    let command_pool = state.db_pool.clone();
    let command_timeout = config.command_timeout_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "command-timeout",
        async move {
            background::run_command_timeout_checker(command_pool, command_timeout).await;
            Ok(())
        },
    );

    let telemetry_pool = state.db_pool.clone();
    let telemetry_retention_days = config.telemetry_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "telemetry-retention",
        async move {
            background::run_telemetry_rollup_and_retention(
                telemetry_pool,
                telemetry_retention_days,
            )
            .await;
            Ok(())
        },
    );

    let rate_limit_state = state.clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "rate-limit-cleanup",
        async move {
            crate::rate_limit::run_cleanup_worker(rate_limit_state).await;
            Ok(())
        },
    );

    let monitor_cancellation = cancellation.clone();
    let readiness = state.readiness.clone();
    let worker_names = readiness
        .snapshot()
        .workers
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let monitor = tokio::spawn(async move {
        let outcome = tokio::select! {
            () = monitor_cancellation.cancelled() => Ok(()),
            exit = workers.join_next() => {
                match exit {
                    Some(Ok(exit)) => {
                        readiness.set_worker(exit.name, false);
                        match exit.result {
                            Ok(()) => Err(anyhow::anyhow!("worker '{}' exited unexpectedly", exit.name)),
                            Err(error) => Err(anyhow::anyhow!("worker '{}' failed: {error}", exit.name)),
                        }
                    }
                    Some(Err(error)) => Err(anyhow::anyhow!("worker task panicked: {error}")),
                    None => Err(anyhow::anyhow!("worker tree exited unexpectedly")),
                }
            }
        };

        monitor_cancellation.cancel();
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        for name in worker_names {
            readiness.set_worker(&name, false);
        }
        outcome
    });

    WorkerSupervisor {
        cancellation,
        monitor,
    }
}
