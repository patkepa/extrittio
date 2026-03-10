use std::sync::Arc;
use tracing::{info, warn};

use crate::state::DbPool;

use super::handlers;

/// Start zenoh subscribers for telemetry and heartbeat topics.
///
/// Spawns one subscriber handler in a background tokio task and runs the other
/// in the current task. Both loop indefinitely, receiving messages and
/// dispatching them to the appropriate handler function.
///
/// # Errors
///
/// Returns an error if any Zenoh subscriber declaration fails.
pub async fn run_subscriber(
    session: Arc<zenoh::Session>,
    db_pool: DbPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let telemetry_sub = session
        .declare_subscriber("extrittio/devices/*/telemetry")
        .await?;

    let heartbeat_sub = session
        .declare_subscriber("extrittio/devices/*/heartbeat")
        .await?;

    let shadow_report_sub = session
        .declare_subscriber("extrittio/devices/*/shadow/report")
        .await?;

    let shadow_get_sub = session
        .declare_subscriber("extrittio/devices/*/shadow/get")
        .await?;

    let log_sub = session
        .declare_subscriber("extrittio/devices/*/logs")
        .await?;

    let cmd_response_sub = session
        .declare_subscriber("extrittio/devices/*/commands/response")
        .await?;

    info!("Zenoh subscribers declared for telemetry, heartbeat, shadow, log, and command response topics");

    // Spawn heartbeat handler in a background task
    let heartbeat_pool = db_pool.clone();
    tokio::spawn(async move {
        loop {
            match heartbeat_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handlers::heartbeat::handle_heartbeat(&heartbeat_pool, &payload);
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
    tokio::spawn(async move {
        loop {
            match shadow_report_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handlers::shadow::handle_shadow_report(&shadow_report_pool, &payload);
                }
                Err(e) => {
                    warn!("Shadow report subscriber channel closed: {}", e);
                    break;
                }
            }
        }
    });

    // Spawn shadow get handler
    let shadow_get_pool = db_pool.clone();
    let shadow_get_session = session.clone();
    tokio::spawn(async move {
        loop {
            match shadow_get_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handlers::shadow::handle_shadow_get(
                        &shadow_get_pool,
                        &shadow_get_session,
                        &payload,
                    )
                    .await;
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
    tokio::spawn(async move {
        loop {
            match log_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handlers::log::handle_device_log(&log_pool, &payload);
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
    tokio::spawn(async move {
        loop {
            match cmd_response_sub.recv_async().await {
                Ok(sample) => {
                    let payload = sample.payload().to_bytes();
                    handlers::command_response::handle_command_response(
                        &cmd_response_pool,
                        &payload,
                    );
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
                let payload = sample.payload().to_bytes();
                handlers::telemetry::handle_telemetry(&db_pool, &payload);
            }
            Err(e) => {
                warn!("Telemetry subscriber channel closed: {}", e);
                break;
            }
        }
    }

    Ok(())
}
