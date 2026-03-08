use chrono::Utc;
use diesel::prelude::*;
use std::time::Duration;
use tracing::{info, warn};

use crate::db::schema::devices;
use crate::state::DbPool;

pub async fn run_offline_checker(db_pool: DbPool, timeout_secs: u64) {
    let interval = Duration::from_secs(60); // check every minute
    info!("Offline checker started (timeout: {}s)", timeout_secs);

    loop {
        tokio::time::sleep(interval).await;

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
