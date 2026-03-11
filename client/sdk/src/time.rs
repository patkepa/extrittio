use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time as milliseconds since the Unix epoch.
#[allow(clippy::cast_possible_truncation)]
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock before Unix epoch")
        .as_millis() as i64
}
