use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

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
    persistence: crate::persistence::Persistence,
}

impl WorkerSupervisor {
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.cancellation.cancel();
        let result = self
            .monitor
            .await
            .map_err(|error| anyhow::anyhow!("worker supervisor task panicked: {error}"))?;
        self.persistence
            .bootstrap
            .maintenance_checkpoint()
            .await
            .map_err(|error| anyhow::anyhow!("database shutdown checkpoint failed: {error}"))?;
        result
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

fn spawn_shutdown_worker<F, Fut>(
    workers: &mut JoinSet<WorkerExit>,
    cancellation: &CancellationToken,
    readiness: &Arc<ReadinessRegistry>,
    name: &'static str,
    worker: F,
) where
    F: FnOnce(CancellationToken) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), String>> + Send + 'static,
{
    readiness.register_worker(name);
    let cancellation = cancellation.clone();
    workers.spawn(async move {
        WorkerExit {
            name,
            result: worker(cancellation).await,
        }
    });
}

/// Start all long-running tasks under one monitored worker tree.
pub fn spawn_background_tasks(config: &AppConfig, state: Arc<AppState>) -> WorkerSupervisor {
    let cancellation = CancellationToken::new();
    let mut workers = JoinSet::new();

    if let Some(thread_runtime) = state.thread_runtime.clone() {
        let service = ThreadDnsSdService {
            instance_name: config.thread_zenoh_service_instance.clone(),
            service_name: config.thread_zenoh_service_name.clone(),
            port: config.zenoh_tls_port,
            tls_enabled: config.zenoh_tls_enabled,
        };
        let listen_host = config.zenoh_listen_host.clone();
        spawn_shutdown_worker(
            &mut workers,
            &cancellation,
            &state.readiness,
            "thread-dns-sd",
            move |cancellation| async move {
                run_thread_dns_sd_advertiser(thread_runtime, service, listen_host, cancellation)
                    .await
            },
        );
    }

    let subscriber_persistence = state.persistence.clone();
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
                subscriber_persistence,
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

    let firmware_migration_persistence = state.persistence.clone();
    let firmware_store = state.firmware_store.clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "firmware-object-migrator",
        async move {
            crate::domains::firmware_store::run_legacy_blob_migrator(
                firmware_migration_persistence,
                firmware_store,
            )
            .await;
            Ok(())
        },
    );

    let outbox_persistence = state.persistence.clone();
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
                outbox_persistence,
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

    let metrics_persistence = state.persistence.clone();
    let metrics_interval = config.system_metrics_interval_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "system-metrics",
        async move {
            services::server_metrics::run_system_metrics_collector(
                metrics_persistence,
                metrics_interval,
            )
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

    let metrics_retention_persistence = state.persistence.clone();
    let metrics_retention_hours = config.metrics_retention_hours;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "metrics-retention",
        async move {
            services::server_metrics::run_metrics_retention(
                metrics_retention_persistence,
                metrics_retention_hours,
            )
            .await;
            Ok(())
        },
    );

    let checker_persistence = state.persistence.clone();
    let checker_cache = state.rule_cache.clone();
    let offline_timeout = config.offline_timeout_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "offline-checker",
        async move {
            background::run_offline_checker(checker_persistence, offline_timeout, checker_cache)
                .await;
            Ok(())
        },
    );

    let retention_persistence = state.persistence.clone();
    let retention_days = config.alert_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "alert-retention",
        async move {
            background::run_alert_retention(retention_persistence, retention_days).await;
            Ok(())
        },
    );

    let log_retention_persistence = state.persistence.clone();
    let log_retention_days = config.log_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "log-retention",
        async move {
            background::run_log_retention(log_retention_persistence, log_retention_days).await;
            Ok(())
        },
    );

    let command_persistence = state.persistence.clone();
    let command_timeout = config.command_timeout_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "command-timeout",
        async move {
            background::run_command_timeout_checker(command_persistence, command_timeout).await;
            Ok(())
        },
    );

    let telemetry_persistence = state.persistence.clone();
    let telemetry_retention_days = config.telemetry_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        &state.readiness,
        "telemetry-retention",
        async move {
            background::run_telemetry_rollup_and_retention(
                telemetry_persistence,
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

    if !state.persistence.backend.capabilities.partitioned_telemetry {
        let maintenance = state.persistence.clone();
        spawn_worker(
            &mut workers,
            &cancellation,
            &state.readiness,
            "database-checkpoint",
            async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(15 * 60));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    maintenance
                        .bootstrap
                        .maintenance_checkpoint()
                        .await
                        .map_err(|error| error.to_string())?;
                }
            },
        );
    }

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
        persistence: state.persistence.clone(),
    }
}

async fn run_thread_dns_sd_advertiser(
    runtime: Arc<extrittio_openthread_runtime::ThreadRuntime>,
    service: ThreadDnsSdService,
    listen_host: String,
    cancellation: CancellationToken,
) -> Result<(), String> {
    let mut registration = None;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

    loop {
        tokio::select! {
            () = cancellation.cancelled() => {
                if let Some(registration) = registration.take() {
                    withdraw_thread_service(registration).await;
                }
                return Ok(());
            }
            _ = interval.tick() => {
                let refresh_runtime = runtime.clone();
                let snapshot = tokio::task::spawn_blocking(move || refresh_runtime.refresh())
                    .await
                    .map_err(|error| format!("Thread DNS-SD refresh task failed: {error}"))?;
                if !snapshot.available {
                    if let Some(registration) = registration.take() {
                        withdraw_thread_service(registration).await;
                        warn!("Thread DNS-SD service withdrawn because OTBR is unavailable");
                    }
                    continue;
                }

                let generation = runtime.network_generation();
                if registration.as_ref().is_some_and(|registration: &ThreadDnsSdRegistration| registration.generation == generation) {
                    continue;
                }

                let address_runtime = runtime.clone();
                let address = match tokio::task::spawn_blocking(move || address_runtime.thread_ipv6_address()).await {
                    Ok(Ok(address)) => address,
                    Ok(Err(error)) => {
                        warn!(%error, "Thread DNS-SD service is waiting for a mesh-reachable IPv6 address");
                        continue;
                    }
                    Err(error) => return Err(format!("Thread DNS-SD address task failed: {error}")),
                };
                if !zenoh_listener_accepts(&listen_host, address) {
                    warn!(
                        listen_host,
                        address = %address,
                        "Thread DNS-SD service is not advertised because the Zenoh listener does not accept the Thread IPv6 address"
                    );
                    continue;
                }

                let advertise_service = service.clone();
                if let Some(registration) = registration.take() {
                    withdraw_thread_service(registration).await;
                }
                match tokio::task::spawn_blocking(move || register_thread_dns_sd_service(advertise_service, address, generation)).await {
                    Ok(Ok(new_registration)) => {
                        registration = Some(new_registration);
                        info!(
                            service = %service.service_name,
                            instance = %service.instance_name,
                            address = %address,
                            port = service.port,
                            "Advertised Zenoh DNS-SD service on the Thread mesh"
                        );
                    }
                    Ok(Err(error)) => warn!(%error, "Failed to advertise Zenoh DNS-SD service on the Thread mesh; will retry"),
                    Err(error) => return Err(format!("Thread DNS-SD advertisement task failed: {error}")),
                }
            }
        }
    }
}

async fn withdraw_thread_service(registration: ThreadDnsSdRegistration) {
    match tokio::task::spawn_blocking(move || registration.withdraw()).await {
        Ok(Ok(())) => info!("Withdrew Zenoh DNS-SD service from the Thread mesh"),
        Ok(Err(error)) => {
            warn!(%error, "Failed to withdraw Zenoh DNS-SD service from the Thread mesh")
        }
        Err(error) => warn!(%error, "Thread DNS-SD withdrawal task failed"),
    }
}

#[derive(Clone)]
struct ThreadDnsSdService {
    instance_name: String,
    service_name: String,
    port: u16,
    tls_enabled: bool,
}

struct ThreadDnsSdRegistration {
    daemon: ServiceDaemon,
    fullname: String,
    generation: u64,
}

impl ThreadDnsSdRegistration {
    fn withdraw(self) -> Result<(), String> {
        let receiver = self
            .daemon
            .unregister(&self.fullname)
            .map_err(|error| error.to_string())?;
        let _ = receiver.recv_timeout(Duration::from_secs(1));
        self.daemon
            .shutdown()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn register_thread_dns_sd_service(
    service: ThreadDnsSdService,
    address: std::net::Ipv6Addr,
    generation: u64,
) -> Result<ThreadDnsSdRegistration, String> {
    validate_thread_dns_sd_service(&service)?;
    let service_type = format!("{}.local.", service.service_name);
    let host_name = format!("{}.local.", service.instance_name);
    let properties = [
        ("role", "server"),
        ("proto", "zenoh"),
        ("version", "1"),
        ("tls", if service.tls_enabled { "1" } else { "0" }),
    ];
    let info = ServiceInfo::new(
        &service_type,
        &service.instance_name,
        &host_name,
        address.to_string(),
        service.port,
        &properties[..],
    )
    .map_err(|error| error.to_string())?;
    let daemon = ServiceDaemon::new().map_err(|error| error.to_string())?;
    daemon.register(info).map_err(|error| error.to_string())?;
    Ok(ThreadDnsSdRegistration {
        daemon,
        fullname: format!("{}.{}", service.instance_name, service_type),
        generation,
    })
}

fn validate_thread_dns_sd_service(service: &ThreadDnsSdService) -> Result<(), String> {
    if service.port == 0 {
        return Err("DNS-SD service port must be greater than zero".to_string());
    }
    if !valid_dns_sd_label(&service.instance_name) {
        return Err("DNS-SD service instance must be a non-empty DNS label".to_string());
    }
    if !service.service_name.starts_with('_')
        || !service.service_name.ends_with("._tcp")
        || service.service_name.split('.').count() != 2
        || service.service_name.len() > 63
    {
        return Err("DNS-SD service name must be a _service._tcp value".to_string());
    }
    Ok(())
}

fn valid_dns_sd_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn zenoh_listener_accepts(listen_host: &str, address: std::net::Ipv6Addr) -> bool {
    let listen_host = listen_host.trim();
    listen_host == "::" || listen_host.parse::<std::net::Ipv6Addr>() == Ok(address)
}
