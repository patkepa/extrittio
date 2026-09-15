use std::future::Future;
use std::sync::Arc;
#[cfg(all(feature = "mdns", not(target_os = "macos")))]
use std::time::Duration;

#[cfg(all(feature = "mdns", not(target_os = "macos")))]
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
#[cfg(feature = "mdns")]
use tracing::info;
use tracing::warn;

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
    database: crate::persistence::DatabaseRuntime,
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
        self.database
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
    let snapshots = state.rule_cache().clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "rule-snapshots",
        async move { snapshots.run().await },
    );

    if let Some(thread_runtime) = state.runtime().thread_runtime().clone() {
        let scan_runtime = thread_runtime.clone();
        spawn_shutdown_worker(
            &mut workers,
            &cancellation,
            state.runtime().readiness(),
            "thread-scanner",
            move |cancellation| async move { run_thread_scanner(scan_runtime, cancellation).await },
        );

        #[cfg(feature = "mdns")]
        {
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
                state.runtime().readiness(),
                "thread-dns-sd",
                move |cancellation| async move {
                    run_thread_dns_sd_advertiser(thread_runtime, service, listen_host, cancellation)
                        .await
                },
            );
        }
    }

    let subscriber_applications = state.workers().subscriber_applications.clone();
    let subscriber_session = state.messaging().zenoh_session().clone();
    let subscriber_metrics = state.messaging().zenoh_metrics().clone();
    let sub_cache = state.rule_cache().clone();
    let max_zenoh_payload_size_bytes = config.max_zenoh_payload_size_bytes;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "zenoh-subscriber",
        async move {
            zenoh_handler::subscriber::run_subscriber(
                subscriber_session,
                subscriber_applications,
                subscriber_metrics,
                sub_cache,
                max_zenoh_payload_size_bytes,
            )
            .await
            .map_err(|error| error.to_string())
        },
    );

    let firmware_readiness_store = state.firmware_store().clone();
    let firmware_readiness = state.runtime().readiness().clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
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

    let firmware_migration_application = state.workers().firmware_migration_application.clone();
    let firmware_store = state.firmware_store().clone();
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "firmware-object-migrator",
        async move {
            crate::domains::firmware_store::run_legacy_blob_migrator(
                firmware_migration_application,
                firmware_store,
            )
            .await;
            Ok(())
        },
    );

    let outbox_application = state.workers().outbox_application.clone();
    let rule_delivery = state.workers().rule_delivery.clone();
    let outbox_config = crate::rule_engine::actions::OutboxWorkerConfig {
        batch_size: config.outbox_batch_size,
        concurrency: config.outbox_concurrency,
        idle_interval: std::time::Duration::from_millis(config.outbox_idle_interval_ms),
        lease_timeout: std::time::Duration::from_secs(config.outbox_lease_timeout_secs),
    };
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "rule-action-outbox",
        async move {
            crate::rule_engine::actions::run_rule_action_outbox_worker(
                outbox_application,
                rule_delivery,
                outbox_config,
            )
            .await;
            Ok(())
        },
    );

    let metrics_worker = state.workers().metrics_worker.clone();
    let system_metrics_application = metrics_worker.clone();
    let metrics_interval = config.system_metrics_interval_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "system-metrics",
        async move {
            services::server_metrics::run_system_metrics_collector(
                system_metrics_application,
                metrics_interval,
            )
            .await;
            Ok(())
        },
    );

    let app_metrics_state = state.clone();
    let app_metrics_application = metrics_worker.clone();
    let app_metrics_interval = config.app_metrics_flush_interval_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "app-metrics",
        async move {
            services::server_metrics::run_app_metrics_flusher(
                app_metrics_state,
                app_metrics_application,
                app_metrics_interval,
            )
            .await;
            Ok(())
        },
    );

    let metrics_retention_application = metrics_worker;
    let metrics_retention_hours = config.metrics_retention_hours;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "metrics-retention",
        async move {
            services::server_metrics::run_metrics_retention(
                metrics_retention_application,
                metrics_retention_hours,
            )
            .await;
            Ok(())
        },
    );

    let checker_application = state.workers().checker_application.clone();
    let checker_cache = state.rule_cache().clone();
    let offline_timeout = config.offline_timeout_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "offline-checker",
        async move {
            background::run_offline_checker(checker_application, offline_timeout, checker_cache)
                .await;
            Ok(())
        },
    );

    let retention_application = state.workers().retention_application.clone();
    let retention_days = config.alert_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "alert-retention",
        async move {
            background::run_alert_retention(retention_application, retention_days).await;
            Ok(())
        },
    );

    let log_retention_application = state.workers().log_retention_application.clone();
    let log_retention_days = config.log_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "log-retention",
        async move {
            background::run_log_retention(log_retention_application, log_retention_days).await;
            Ok(())
        },
    );

    let command_application = state.workers().command_application.clone();
    let command_timeout = config.command_timeout_secs;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "command-timeout",
        async move {
            background::run_command_timeout_checker(command_application, command_timeout).await;
            Ok(())
        },
    );

    let telemetry_application = state.workers().telemetry_application.clone();
    let telemetry_retention_days = config.telemetry_retention_days;
    spawn_worker(
        &mut workers,
        &cancellation,
        state.runtime().readiness(),
        "telemetry-retention",
        async move {
            background::run_telemetry_rollup_and_retention(
                telemetry_application,
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
        state.runtime().readiness(),
        "rate-limit-cleanup",
        async move {
            crate::rate_limit::run_cleanup_worker(rate_limit_state).await;
            Ok(())
        },
    );

    if !state
        .runtime()
        .database()
        .descriptor()
        .capabilities
        .partitioned_telemetry
    {
        let maintenance = state.runtime().database().clone();
        spawn_worker(
            &mut workers,
            &cancellation,
            state.runtime().readiness(),
            "database-checkpoint",
            async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(15 * 60));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    maintenance
                        .maintenance_checkpoint()
                        .await
                        .map_err(|error| error.to_string())?;
                }
            },
        );
    }

    let monitor_cancellation = cancellation.clone();
    let readiness = state.runtime().readiness().clone();
    let worker_names = readiness
        .snapshot()
        .workers
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let monitor = tokio::spawn(async move {
        let outcome = tokio::select! {
            biased;
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
        database: state.runtime().database().clone(),
    }
}

async fn run_thread_scanner(
    runtime: Arc<extrittio_openthread_runtime::ThreadRuntime>,
    cancellation: CancellationToken,
) -> Result<(), String> {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            () = cancellation.cancelled() => return Ok(()),
            _ = interval.tick() => {
                let scan_runtime = runtime.clone();
                match tokio::task::spawn_blocking(move || {
                    let snapshot = scan_runtime.refresh();
                    if !snapshot.available {
                        return Ok(false);
                    }
                    scan_runtime
                        .refresh_scan(std::time::Duration::from_secs(60), false)
                        .map(|_| true)
                }).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => warn!(%error, "OpenThread background scan failed; will retry"),
                    Err(error) => return Err(format!("OpenThread scanner task failed: {error}")),
                }
            }
        }
    }
}

#[cfg(feature = "mdns")]
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
                let snapshot = runtime.snapshot();
                if !snapshot.available {
                    if let Some(registration) = registration.take() {
                        withdraw_thread_service(registration).await;
                        warn!("Thread DNS-SD service withdrawn because OTBR is unavailable");
                    }
                    continue;
                }

                // A border router without an Active Operational Dataset is a
                // valid, ready-to-provision appliance. DNS-SD must not seed a
                // shared development network on its behalf: that competes
                // with explicit create/import requests and exposes a known
                // network credential on a real installation.
                let status_runtime = runtime.clone();
                let attached = match tokio::task::spawn_blocking(move || {
                    status_runtime.status().map(|status| status.is_attached())
                })
                .await
                {
                    Ok(Ok(attached)) => attached,
                    Ok(Err(error)) => {
                        warn!(%error, "Thread DNS-SD service is waiting for OTBR status");
                        continue;
                    }
                    Err(error) => return Err(format!("Thread DNS-SD status task failed: {error}")),
                };
                if !attached {
                    if let Some(registration) = registration.take() {
                        withdraw_thread_service(registration).await;
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

#[cfg(feature = "mdns")]
async fn withdraw_thread_service(registration: ThreadDnsSdRegistration) {
    match tokio::task::spawn_blocking(move || registration.withdraw()).await {
        Ok(Ok(())) => info!("Withdrew Zenoh DNS-SD service from the Thread mesh"),
        Ok(Err(error)) => {
            warn!(%error, "Failed to withdraw Zenoh DNS-SD service from the Thread mesh")
        }
        Err(error) => warn!(%error, "Thread DNS-SD withdrawal task failed"),
    }
}

#[cfg(feature = "mdns")]
#[derive(Clone)]
struct ThreadDnsSdService {
    instance_name: String,
    service_name: String,
    port: u16,
    tls_enabled: bool,
}

#[cfg(feature = "mdns")]
struct ThreadDnsSdRegistration {
    registration: ThreadDnsSdRegistrationKind,
    generation: u64,
}

#[cfg(feature = "mdns")]
enum ThreadDnsSdRegistrationKind {
    #[cfg(target_os = "macos")]
    NativeBonjour(NativeBonjourRegistration),
    #[cfg(not(target_os = "macos"))]
    PortableMdns {
        daemon: ServiceDaemon,
        fullname: String,
    },
}

#[cfg(feature = "mdns")]
impl ThreadDnsSdRegistration {
    fn withdraw(self) -> Result<(), String> {
        match self.registration {
            #[cfg(target_os = "macos")]
            ThreadDnsSdRegistrationKind::NativeBonjour(registration) => registration.withdraw(),
            #[cfg(not(target_os = "macos"))]
            ThreadDnsSdRegistrationKind::PortableMdns { daemon, fullname } => {
                let receiver = daemon
                    .unregister(&fullname)
                    .map_err(|error| error.to_string())?;
                let _ = receiver.recv_timeout(Duration::from_secs(1));
                daemon
                    .shutdown()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        }
    }
}

#[cfg(feature = "mdns")]
fn register_thread_dns_sd_service(
    service: ThreadDnsSdService,
    address: std::net::Ipv6Addr,
    generation: u64,
) -> Result<ThreadDnsSdRegistration, String> {
    validate_thread_dns_sd_service(&service)?;

    #[cfg(target_os = "macos")]
    {
        Ok(ThreadDnsSdRegistration {
            registration: ThreadDnsSdRegistrationKind::NativeBonjour(
                NativeBonjourRegistration::register(&service, address)?,
            ),
            generation,
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
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
            registration: ThreadDnsSdRegistrationKind::PortableMdns {
                daemon,
                fullname: format!("{}.{}", service.instance_name, service_type),
            },
            generation,
        })
    }
}

#[cfg(all(feature = "mdns", target_os = "macos"))]
struct NativeBonjourRegistration {
    child: std::process::Child,
}

#[cfg(all(feature = "mdns", target_os = "macos"))]
impl NativeBonjourRegistration {
    fn register(service: &ThreadDnsSdService, address: std::net::Ipv6Addr) -> Result<Self, String> {
        use std::process::{Command, Stdio};

        // `dns-sd -P` is Apple's supported custom-host registration interface.
        // It atomically publishes PTR/SRV/TXT/AAAA records for the Thread ULA;
        // macOS 26 rejects the corresponding low-level record sequence.
        // `dns-sd -P` passes its numeric port argument directly to the C
        // API, whose port parameter is network byte order. Convert here so a
        // configured 7447 is published as 7447 rather than byte-swapped 5917.
        let port = service.port.to_be().to_string();
        let host = format!("{}.local.", service.instance_name);
        let address = address.to_string();
        let child = Command::new("/usr/bin/dns-sd")
            .args([
                "-P",
                service.instance_name.as_str(),
                service.service_name.as_str(),
                "local.",
                port.as_str(),
                host.as_str(),
                address.as_str(),
                "role=server",
                "proto=zenoh",
                "version=1",
                if service.tls_enabled {
                    "tls=1"
                } else {
                    "tls=0"
                },
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Failed to start macOS DNS-SD proxy registration: {error}"))?;
        Ok(Self { child })
    }

    fn withdraw(mut self) -> Result<(), String> {
        if self
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            self.child.kill().map_err(|error| error.to_string())?;
            let _ = self.child.wait();
        }
        Ok(())
    }
}

#[cfg(feature = "mdns")]
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

#[cfg(feature = "mdns")]
fn valid_dns_sd_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(feature = "mdns")]
fn zenoh_listener_accepts(listen_host: &str, address: std::net::Ipv6Addr) -> bool {
    let listen_host = listen_host.trim();
    listen_host == "::" || listen_host.parse::<std::net::Ipv6Addr>() == Ok(address)
}

/// Precomposed worker capabilities; runtime scheduling needs no persistence ports.
pub(crate) struct WorkerApplications {
    subscriber_applications: crate::zenoh_handler::DeviceMessageApplications,
    firmware_migration_application:
        extrittio_backend_core::application::FirmwareMigrationApplication,
    outbox_application: extrittio_backend_core::OutboxWorkerApplication,
    rule_delivery: extrittio_backend_core::application::RuleDeliveryApplication,
    metrics_worker: extrittio_backend_core::application::MetricsWorkerApplication,
    checker_application: extrittio_backend_core::DeviceIngressApplication,
    retention_application: extrittio_backend_core::AlertMaintenanceApplication,
    log_retention_application: extrittio_backend_core::LogIngressApplication,
    command_application: extrittio_backend_core::CommandWorkerApplication,
    telemetry_application: extrittio_backend_core::TelemetryMaintenanceApplication,
}
impl WorkerApplications {
    pub(crate) fn new(
        persistence: &extrittio_backend_core::RepositorySetInput,
        session: &Arc<zenoh::Session>,
        metrics: &Arc<crate::state::ZenohMetrics>,
    ) -> Self {
        let subscriber_applications = crate::zenoh_handler::DeviceMessageApplications {
            identity: extrittio_backend_core::DeviceIngressApplication::new(
                persistence.device_ingress.clone(),
                Arc::new(crate::auth::SystemClock),
            ),
            telemetry: extrittio_backend_core::TelemetryIngressApplication::new(
                persistence.telemetry.clone(),
                persistence.device_ingress.clone(),
                Arc::new(crate::auth::SystemClock),
            ),
            events: extrittio_backend_core::EventIngressApplication::new(
                persistence.events.clone(),
                persistence.devices.clone(),
                persistence.device_ingress.clone(),
                Arc::new(crate::auth::SystemClock),
            ),
            contracts: extrittio_backend_core::application::ContractIngressApplication::new(
                persistence.devices.clone(),
            ),
            logs: extrittio_backend_core::LogIngressApplication::new(
                persistence.logs.clone(),
                Arc::new(crate::auth::SystemClock),
            ),
            commands: extrittio_backend_core::CommandWorkerApplication::new(
                persistence.commands.clone(),
                Arc::new(crate::auth::SystemClock),
            ),
            shadows: extrittio_backend_core::DeviceShadowApplication::new(
                persistence.shadows.clone(),
                Arc::new(crate::auth::SystemClock),
            ),
            reports: extrittio_backend_core::application::DeviceReportApplication::new(
                extrittio_backend_core::DeviceShadowApplication::new(
                    persistence.shadows.clone(),
                    Arc::new(crate::auth::SystemClock),
                ),
                extrittio_backend_core::application::FirmwareReportApplication::new(
                    persistence.firmware.clone(),
                    Arc::new(crate::auth::SystemClock),
                ),
            ),
        };
        let firmware_migration_application =
            extrittio_backend_core::application::FirmwareMigrationApplication::new(
                persistence.firmware.clone(),
            );
        let outbox_application =
            extrittio_backend_core::OutboxWorkerApplication::new(persistence.outbox.clone());
        let rule_delivery = extrittio_backend_core::application::RuleDeliveryApplication::new(
            outbox_application.clone(),
            extrittio_backend_core::AlertWorkerApplication::new(persistence.alerts.clone()),
            extrittio_backend_core::CommandApplication::new(
                persistence.commands.clone(),
                persistence.devices.clone(),
                Arc::new(crate::outbound::device_bus::ZenohDeviceBus::new(
                    session.clone(),
                    metrics.clone(),
                )),
            ),
            extrittio_backend_core::RuleRuntimeApplication::new(persistence.rules.clone()),
            Arc::new(crate::outbound::webhook::HttpWebhookSender),
        );
        let metrics_worker = extrittio_backend_core::application::MetricsWorkerApplication::new(
            persistence.metrics.clone(),
            Arc::new(crate::auth::SystemClock),
        );
        let checker_application = extrittio_backend_core::DeviceIngressApplication::new(
            persistence.device_ingress.clone(),
            Arc::new(crate::auth::SystemClock),
        );
        let retention_application = extrittio_backend_core::AlertMaintenanceApplication::new(
            persistence.alerts.clone(),
            persistence.rules.clone(),
        );
        let log_retention_application = extrittio_backend_core::LogIngressApplication::new(
            persistence.logs.clone(),
            Arc::new(crate::auth::SystemClock),
        );
        let command_application = extrittio_backend_core::CommandWorkerApplication::new(
            persistence.commands.clone(),
            Arc::new(crate::auth::SystemClock),
        );
        let telemetry_application = extrittio_backend_core::TelemetryMaintenanceApplication::new(
            persistence.telemetry.clone(),
            Arc::new(crate::auth::SystemClock),
        );
        Self {
            subscriber_applications,
            firmware_migration_application,
            outbox_application,
            rule_delivery,
            metrics_worker,
            checker_application,
            retention_application,
            log_retention_application,
            command_application,
            telemetry_application,
        }
    }
}
