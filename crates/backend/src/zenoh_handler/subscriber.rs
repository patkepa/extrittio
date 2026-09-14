use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::{info, warn};

use crate::state::ZenohMetrics;
use crate::zenoh_handler::DeviceMessageApplications;

use super::handlers;

/// Start zenoh subscribers for telemetry, heartbeat, shadow, log, and command
/// response topics.
///
/// Spawns five subscriber handlers in background tokio tasks (heartbeat,
/// shadow_report, shadow_get, log, command_response) and runs the telemetry
/// handler in the current task. All loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
///
/// Handlers receive core application capabilities; persistence and blocking
/// boundaries remain behind those applications.
///
/// # Errors
///
/// Returns an error if any Zenoh subscriber declaration fails.
pub(crate) async fn run_subscriber(
    session: Arc<zenoh::Session>,
    applications: DeviceMessageApplications,
    zenoh_metrics: Arc<ZenohMetrics>,
    rule_cache: Arc<crate::rule_snapshots::RuleSnapshotStore>,
    max_payload_size_bytes: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use extrittio_common::topics;
    use extrittio_common::topics::patterns;

    let telemetry_sub = session.declare_subscriber(patterns::TELEMETRY).await?;

    let contract_sub = session
        .declare_subscriber(patterns::CONTRACT_INGRESS)
        .await?;

    let heartbeat_sub = session.declare_subscriber(patterns::HEARTBEAT).await?;

    let shadow_report_sub = session.declare_subscriber(patterns::SHADOW_REPORT).await?;

    let shadow_get_sub = session.declare_subscriber(patterns::SHADOW_GET).await?;

    let log_sub = session.declare_subscriber(patterns::LOGS).await?;

    let cmd_response_sub = session
        .declare_subscriber(patterns::COMMANDS_RESPONSE)
        .await?;

    info!(
        "Zenoh subscribers declared for telemetry, heartbeat, shadow, log, and command response topics"
    );
    let mut subscriber_tasks = tokio::task::JoinSet::new();

    let contract_applications = applications.clone();
    let contract_metrics = zenoh_metrics.clone();
    let contract_cache = rule_cache.clone();
    subscriber_tasks.spawn(async move {
        loop {
            match contract_sub.recv_async().await {
                Ok(sample) => {
                    let Some((topic_device_id, topic, payload)) =
                        accept_contract_sample(&sample, max_payload_size_bytes)
                    else {
                        continue;
                    };
                    let Some(identity) = handlers::resolve_ingress_identity(
                        &contract_applications.identity,
                        "contract event",
                        &topic_device_id,
                        true,
                    )
                    .await
                    else {
                        continue;
                    };
                    let handled = handlers::contract_ingress::handle_contract_ingress(
                        &contract_applications.contracts,
                        &contract_applications.commands,
                        &contract_applications.events,
                        &identity,
                        &topic,
                        &payload,
                        &contract_cache,
                    )
                    .await;
                    if handled {
                        contract_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(error) => {
                    return Err::<(), String>(format!(
                        "contract event subscriber channel closed: {error}"
                    ));
                }
            }
        }
    });

    // Spawn heartbeat handler in a background task
    let heartbeat_applications = applications.clone();
    let heartbeat_metrics = zenoh_metrics.clone();
    let heartbeat_cache = rule_cache.clone();
    subscriber_tasks.spawn(async move {
        loop {
            match heartbeat_sub.recv_async().await {
                Ok(sample) => {
                    let Some((topic_device_id, payload)) = accept_sample(
                        &sample,
                        topics::heartbeat_device_id,
                        max_payload_size_bytes,
                        "heartbeat",
                    ) else {
                        continue;
                    };
                    let identity = handlers::resolve_ingress_identity(
                        &heartbeat_applications.identity,
                        "heartbeat",
                        &topic_device_id,
                        true,
                    )
                    .await;
                    handlers::heartbeat::handle_heartbeat(
                        &heartbeat_applications.identity,
                        identity,
                        &topic_device_id,
                        &payload,
                        &heartbeat_cache,
                    )
                    .await;
                    heartbeat_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Heartbeat subscriber channel closed: {}", e);
                    return Err::<(), String>(format!("heartbeat subscriber channel closed: {e}"));
                }
            }
        }
    });

    // Spawn shadow report handler
    let shadow_report_applications = applications.clone();
    let shadow_report_metrics = zenoh_metrics.clone();
    subscriber_tasks.spawn(async move {
        loop {
            match shadow_report_sub.recv_async().await {
                Ok(sample) => {
                    let Some((topic_device_id, payload)) = accept_sample(
                        &sample,
                        topics::shadow_report_device_id,
                        max_payload_size_bytes,
                        "shadow report",
                    ) else {
                        continue;
                    };
                    handlers::shadow::handle_shadow_report(
                        &shadow_report_applications.identity,
                        &shadow_report_applications.reports,
                        &topic_device_id,
                        &payload,
                    )
                    .await;
                    shadow_report_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Shadow report subscriber channel closed: {}", e);
                    return Err::<(), String>(format!(
                        "shadow report subscriber channel closed: {e}"
                    ));
                }
            }
        }
    });

    // Spawn shadow get handler (async — DB part uses spawn_blocking internally)
    let shadow_get_applications = applications.clone();
    let shadow_get_session = session.clone();
    let shadow_get_metrics = zenoh_metrics.clone();
    subscriber_tasks.spawn(async move {
        loop {
            match shadow_get_sub.recv_async().await {
                Ok(sample) => {
                    let Some((topic_device_id, payload)) = accept_sample(
                        &sample,
                        topics::shadow_get_device_id,
                        max_payload_size_bytes,
                        "shadow get",
                    ) else {
                        continue;
                    };
                    handlers::shadow::handle_shadow_get(
                        &shadow_get_applications.identity,
                        &shadow_get_applications.shadows,
                        &shadow_get_session,
                        &topic_device_id,
                        &payload,
                        &shadow_get_metrics,
                    )
                    .await;
                    shadow_get_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Shadow get subscriber channel closed: {}", e);
                    return Err::<(), String>(format!("shadow get subscriber channel closed: {e}"));
                }
            }
        }
    });

    // Spawn log handler
    let log_applications = applications.clone();
    let log_metrics = zenoh_metrics.clone();
    subscriber_tasks.spawn(async move {
        loop {
            match log_sub.recv_async().await {
                Ok(sample) => {
                    let Some((topic_device_id, payload)) = accept_sample(
                        &sample,
                        topics::logs_device_id,
                        max_payload_size_bytes,
                        "device log",
                    ) else {
                        continue;
                    };
                    let Some(identity) = handlers::resolve_ingress_identity(
                        &log_applications.identity,
                        "device log",
                        &topic_device_id,
                        true,
                    )
                    .await
                    else {
                        continue;
                    };
                    handlers::log::handle_device_log(
                        &log_applications.logs,
                        &identity,
                        &topic_device_id,
                        &payload,
                    )
                    .await;
                    log_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Log subscriber channel closed: {}", e);
                    return Err::<(), String>(format!("log subscriber channel closed: {e}"));
                }
            }
        }
    });

    // Spawn command response handler
    let cmd_response_applications = applications.clone();
    let cmd_response_metrics = zenoh_metrics.clone();
    subscriber_tasks.spawn(async move {
        loop {
            match cmd_response_sub.recv_async().await {
                Ok(sample) => {
                    let Some((topic_device_id, payload)) = accept_sample(
                        &sample,
                        topics::commands_response_device_id,
                        max_payload_size_bytes,
                        "command response",
                    ) else {
                        continue;
                    };
                    let Some(identity) = handlers::resolve_ingress_identity(
                        &cmd_response_applications.identity,
                        "command response",
                        &topic_device_id,
                        true,
                    )
                    .await
                    else {
                        continue;
                    };
                    handlers::command_response::handle_command_response(
                        &cmd_response_applications.commands,
                        &identity,
                        &topic_device_id,
                        &payload,
                    )
                    .await;
                    cmd_response_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Command response subscriber channel closed: {}", e);
                    return Err::<(), String>(format!(
                        "command response subscriber channel closed: {e}"
                    ));
                }
            }
        }
    });

    // Run telemetry handler in the current task
    loop {
        tokio::select! {
            child = subscriber_tasks.join_next() => {
                let message = match child {
                    Some(Ok(Ok(()))) => "a Zenoh subscriber exited unexpectedly".to_string(),
                    Some(Ok(Err(error))) => error,
                    Some(Err(error)) => format!("Zenoh subscriber task panicked: {error}"),
                    None => "all Zenoh subscriber tasks exited".to_string(),
                };
                return Err(message.into());
            }
            sample = telemetry_sub.recv_async() => {
                match sample {
                    Ok(sample) => {
                        let Some((topic_device_id, payload)) = accept_sample(
                            &sample,
                            topics::telemetry_device_id,
                            max_payload_size_bytes,
                            "telemetry",
                        ) else {
                            continue;
                        };
                        let Some(identity) = handlers::resolve_ingress_identity(
                            &applications.identity,
                            "telemetry",
                            &topic_device_id,
                            true,
                        )
                        .await
                        else {
                            continue;
                        };
                        handlers::telemetry::handle_telemetry(
                            &applications.telemetry,
                            &identity,
                            &topic_device_id,
                            &payload,
                            &rule_cache,
                        )
                        .await;
                        zenoh_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(error) => {
                        return Err(format!("telemetry subscriber channel closed: {error}").into());
                    }
                }
            }
        }
    }
}

fn accept_sample<'a>(
    sample: &'a zenoh::sample::Sample,
    topic_device_id: impl Fn(&'a str) -> Option<&'a str>,
    max_payload_size_bytes: usize,
    label: &str,
) -> Option<(String, Vec<u8>)> {
    let topic = sample.key_expr().as_str();
    let Some(device_id) = topic_device_id(topic) else {
        warn!("{label} sample arrived on invalid topic `{topic}`; dropping message");
        return None;
    };

    let payload_len = sample.payload().len();
    if payload_len > max_payload_size_bytes {
        warn!(
            "{label} sample for device {device_id} exceeded payload limit: {payload_len} > {max_payload_size_bytes}; dropping message"
        );
        return None;
    }

    Some((device_id.to_string(), sample.payload().to_bytes().to_vec()))
}

fn accept_contract_sample(
    sample: &zenoh::sample::Sample,
    max_payload_size_bytes: usize,
) -> Option<(String, String, Vec<u8>)> {
    let topic = sample.key_expr().as_str();
    let Some(device_id) = extrittio_common::topics::contract_device_id(topic) else {
        warn!("contract event sample arrived on invalid topic `{topic}`; dropping message");
        return None;
    };
    let payload_len = sample.payload().len();
    if payload_len > max_payload_size_bytes {
        warn!(
            "contract event sample for device {device_id} exceeded server payload limit: {payload_len} > {max_payload_size_bytes}; dropping message"
        );
        return None;
    }
    Some((
        device_id.to_string(),
        topic.to_string(),
        sample.payload().to_bytes().to_vec(),
    ))
}
