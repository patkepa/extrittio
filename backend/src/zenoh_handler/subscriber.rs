use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::Ordering;
use tracing::{info, warn};

use crate::rule_engine::actions::enqueue_pending_actions;
use crate::rule_engine::cache::RuleCache;
use crate::rule_engine::types::PendingAction;
use crate::state::{DbPool, ZenohMetrics};

use super::handlers;

/// Start zenoh subscribers for telemetry, heartbeat, shadow, log, and command
/// response topics.
///
/// Spawns five subscriber handlers in background tokio tasks (heartbeat,
/// shadow_report, shadow_get, log, command_response) and runs the telemetry
/// handler in the current task. All loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
///
/// Synchronous handler functions (DB-touching) are dispatched via
/// `spawn_blocking` to avoid starving the Tokio runtime, except where the
/// handler manages `spawn_blocking` internally (e.g. `shadow_get`).
///
/// # Errors
///
/// Returns an error if any Zenoh subscriber declaration fails.
pub async fn run_subscriber(
    session: Arc<zenoh::Session>,
    db_pool: DbPool,
    zenoh_metrics: Arc<ZenohMetrics>,
    rule_cache: Arc<RwLock<RuleCache>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use extrittio_common::topics::patterns;

    let telemetry_sub = session.declare_subscriber(patterns::TELEMETRY).await?;

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

    // Spawn heartbeat handler in a background task
    let heartbeat_pool = db_pool.clone();
    let heartbeat_metrics = zenoh_metrics.clone();
    let heartbeat_cache = rule_cache.clone();
    tokio::spawn(async move {
        loop {
            match heartbeat_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = heartbeat_pool.clone();
                    let cache = heartbeat_cache.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        handlers::heartbeat::handle_heartbeat(&pool, &payload, &cache)
                    })
                    .await;
                    heartbeat_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);

                    if let Ok(actions) = result {
                        enqueue_actions(heartbeat_pool.clone(), actions, "heartbeat").await;
                    }
                }
                Err(e) => {
                    warn!("Heartbeat subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow report handler
    let shadow_report_pool = db_pool.clone();
    let shadow_report_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match shadow_report_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = shadow_report_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::shadow::handle_shadow_report(&pool, &payload);
                    })
                    .await;
                    shadow_report_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Shadow report subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow get handler (async — DB part uses spawn_blocking internally)
    let shadow_get_pool = db_pool.clone();
    let shadow_get_session = session.clone();
    let shadow_get_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match shadow_get_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    handlers::shadow::handle_shadow_get(
                        &shadow_get_pool,
                        &shadow_get_session,
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
                    break;
                }
            }
        }
    });

    // Spawn log handler
    let log_pool = db_pool.clone();
    let log_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match log_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = log_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::log::handle_device_log(&pool, &payload);
                    })
                    .await;
                    log_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Log subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn command response handler
    let cmd_response_pool = db_pool.clone();
    let cmd_response_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match cmd_response_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = cmd_response_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::command_response::handle_command_response(&pool, &payload);
                    })
                    .await;
                    cmd_response_metrics
                        .messages_in
                        .fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!("Command response subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Run telemetry handler in the current task
    loop {
        match telemetry_sub.recv_async().await {
            Ok(sample) => {
                let payload = sample.payload().to_bytes().to_vec();
                let pool = db_pool.clone();
                let cache = rule_cache.clone();
                let result = tokio::task::spawn_blocking(move || {
                    handlers::telemetry::handle_telemetry(&pool, &payload, &cache)
                })
                .await;
                zenoh_metrics.messages_in.fetch_add(1, Ordering::Relaxed);

                if let Ok(actions) = result {
                    enqueue_actions(db_pool.clone(), actions, "telemetry").await;
                }
            }
            Err(e) => {
                warn!("Telemetry subscriber channel closed: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn enqueue_actions(db_pool: DbPool, actions: Vec<PendingAction>, source: &'static str) {
    if actions.is_empty() {
        return;
    }

    let count = actions.len();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = db_pool.get().map_err(|e| e.to_string())?;
        enqueue_pending_actions(&mut conn, &actions)
    })
    .await;

    match result {
        Ok(Ok(inserted)) => {
            info!(
                "Enqueued {} rule action(s) from {} into durable outbox",
                inserted, source
            );
        }
        Ok(Err(e)) => warn!(
            "Failed to enqueue {} rule action(s) from {}: {}",
            count, source, e
        ),
        Err(e) => warn!(
            "Rule action enqueue task panicked for {} action(s) from {}: {}",
            count, source, e
        ),
    }
}
