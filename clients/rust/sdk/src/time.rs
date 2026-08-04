use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current time as milliseconds since the Unix epoch.
pub fn now_millis() -> i64 {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock before Unix epoch")
        .as_millis();
    i64::try_from(ms).expect("timestamp overflows i64")
}
