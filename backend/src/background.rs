use std::time::Duration;
use tracing::{info, warn};

use crate::services::{command_service, device_service};
use crate::state::DbPool;

pub async fn run_offline_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(60); // check every minute
    info!("Offline checker started (timeout: {}s)", timeout_secs);

    loop {
        tokio::time::sleep(interval).await;

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            device_service::check_offline_devices(&mut conn, timeout_secs)
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(count)) => {
                if count > 0 {
                    info!("Marked {} devices as offline", count);
                }
            }
            Ok(Err(msg)) => {
                warn!("Offline checker error: {}", msg);
            }
            Err(e) => {
                warn!("Offline checker task panicked: {}", e);
            }
        }
    }
}

pub async fn run_command_timeout_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(30);
    info!(
        "Command timeout checker started (timeout: {}s)",
        timeout_secs
    );

    loop {
        tokio::time::sleep(interval).await;

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            command_service::timeout_stale(&mut conn, timeout_secs)
                .map_err(|e| e.to_string())
        })
        .await;

        match result {
            Ok(Ok(count)) => {
                if count > 0 {
                    info!("Marked {} commands as timed_out", count);
                }
            }
            Ok(Err(msg)) => {
                warn!("Command timeout checker error: {}", msg);
            }
            Err(e) => {
                warn!("Command timeout checker task panicked: {}", e);
            }
        }
    }
}
