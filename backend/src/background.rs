use chrono::Utc;
use std::time::Duration;
use tracing::{info, warn};

use crate::db::models::NewDeviceLog;
use diesel::Connection;

use crate::repositories::{command_repo, device_repo, log_repo};
use crate::state::DbPool;

pub async fn run_offline_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(60); // check every minute
    info!("Offline checker started (timeout: {}s)", timeout_secs);

    loop {
        tokio::time::sleep(interval).await;

        #[allow(clippy::cast_possible_wrap)]
        let cutoff = Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;

            // Find and mark devices offline atomically within a transaction
            let (going_offline, count) = conn.transaction::<_, diesel::result::Error, _>(|conn| {
                let going_offline = device_repo::find_devices_going_offline(conn, cutoff)?;
                let count = device_repo::mark_devices_offline(conn, cutoff)?;
                Ok((going_offline, count))
            }).map_err(|e| e.to_string())?;

            // Log an entry for each device that went offline
            for device_id in &going_offline {
                let message = format!(
                    "Device went offline (no heartbeat for {}s)",
                    timeout_secs
                );
                if let Err(e) = log_repo::insert_log(
                    &mut conn,
                    &NewDeviceLog {
                        device_id: device_id.clone(),
                        level: "WARN".to_string(),
                        message,
                    },
                ) {
                    warn!("Failed to insert offline log for {}: {}", device_id, e);
                }
            }

            Ok::<usize, String>(count)
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

        #[allow(clippy::cast_possible_wrap)]
        let cutoff = Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);
        let now = Utc::now().naive_utc();

        let pool = db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| e.to_string())?;
            command_repo::timeout_stale_commands(&mut conn, cutoff, now).map_err(|e| e.to_string())
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
