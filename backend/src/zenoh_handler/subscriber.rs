use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::{info, warn};

use crate::state::{DbPool, ZenohMetrics};

use super::handlers;

/// Start zenoh subscribers for telemetry and heartbeat topics.
///
/// Spawns one subscriber handler in a background tokio task and runs the other
/// in the current task. Both loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
///
/// All synchronous handler functions (DB-touching) are dispatched via
/// `spawn_blocking` to avoid starving the Tokio runtime.
///
/// # Errors
///
/// Returns an error if any Zenoh subscriber declaration fails.
pub async fn run_subscriber(
    session: Arc<zenoh::Session>,
    db_pool: DbPool,
    zenoh_metrics: Arc<ZenohMetrics>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use extrittio_common::topics::patterns;

    let telemetry_sub = session
        .declare_subscriber(patterns::TELEMETRY)
        .await?;

    let heartbeat_sub = session
        .declare_subscriber(patterns::HEARTBEAT)
        .await?;

    let shadow_report_sub = session
        .declare_subscriber(patterns::SHADOW_REPORT)
        .await?;

    let shadow_get_sub = session
        .declare_subscriber(patterns::SHADOW_GET)
        .await?;

    let log_sub = session
        .declare_subscriber(patterns::LOGS)
        .await?;

    let cmd_response_sub = session
        .declare_subscriber(patterns::COMMANDS_RESPONSE)
        .await?;

    info!(
        "Zenoh subscribers declared for telemetry, heartbeat, shadow, log, and command response topics"
    );

    // Spawn heartbeat handler in a background task
    let heartbeat_pool = db_pool.clone();
    let heartbeat_metrics = zenoh_metrics.clone();
    tokio::spawn(async move {
        loop {
            match heartbeat_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes().to_vec();
                    let pool = heartbeat_pool.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        handlers::heartbeat::handle_heartbeat(&pool, &payload);
                    })
                    .await;
                    heartbeat_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
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
                    shadow_report_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
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
                    shadow_get_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
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
                    cmd_response_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
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
                let _ = tokio::task::spawn_blocking(move || {
                    handlers::telemetry::handle_telemetry(&pool, &payload);
                })
                .await;
                zenoh_metrics.messages_in.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                warn!("Telemetry subscriber channel closed: {}", e);
                break;
            }
        }
    }

    Ok(())
}
