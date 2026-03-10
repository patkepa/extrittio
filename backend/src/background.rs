use chrono::Utc;
use diesel::prelude::*;
use std::time::Duration;
use tracing::{info, warn};

use crate::db::schema::{command_history, devices};
use crate::state::DbPool;

pub async fn run_offline_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(60); // check every minute
    info!("Offline checker started (timeout: {}s)", timeout_secs);

    loop {
        tokio::time::sleep(interval).await;

        #[allow(clippy::cast_possible_wrap)]
        let cutoff = Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);

        let mut conn = match db_pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                warn!("DB pool error in offline checker: {}", e);
                continue;
            }
        };

        match diesel::update(
            devices::table
                .filter(devices::status.ne("offline"))
                .filter(devices::last_seen.lt(cutoff)),
        )
        .set(devices::status.eq("offline"))
        .execute(&mut conn)
        {
            Ok(count) => {
                if count > 0 {
                    info!("Marked {} devices as offline", count);
                }
            }
            Err(e) => {
                warn!("Offline checker error: {}", e);
            }
        }
    }
}

pub async fn run_command_timeout_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(30);
    info!("Command timeout checker started (timeout: {}s)", timeout_secs);

    loop {
        tokio::time::sleep(interval).await;

        #[allow(clippy::cast_possible_wrap)]
        let cutoff = Utc::now().naive_utc() - chrono::TimeDelta::seconds(timeout_secs as i64);

        let mut conn = match db_pool.get() {
            Ok(conn) => conn,
            Err(e) => {
                warn!("DB pool error in command timeout checker: {}", e);
                continue;
            }
        };

        match diesel::update(
            command_history::table
                .filter(command_history::status.eq_any(&["sent", "delivered"]))
                .filter(command_history::created_at.lt(cutoff)),
        )
        .set((
            command_history::status.eq("timed_out"),
            command_history::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        {
            Ok(count) => {
                if count > 0 {
                    info!("Marked {} commands as timed_out", count);
                }
            }
            Err(e) => {
                warn!("Command timeout checker error: {}", e);
            }
        }
    }
}
