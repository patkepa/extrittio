pub fn format_uptime(seconds: Option<i32>) -> Option<String> {
    let seconds = i64::from(seconds?);
    if seconds <= 0 {
        return Some("0s".to_string());
    }
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{minutes}m"));
    }
    Some(parts.join(" "))
}

pub fn format_last_seen(last_seen: Option<chrono::NaiveDateTime>) -> Option<String> {
    let timestamp = last_seen?;
    let seconds = chrono::Utc::now()
        .naive_utc()
        .signed_duration_since(timestamp)
        .num_seconds();
    if seconds < 0 {
        return Some("just now".to_string());
    }
    Some(if seconds < 60 {
        format!("{seconds} seconds ago")
    } else if seconds < 3_600 {
        let minutes = seconds / 60;
        format!(
            "{minutes} minute{} ago",
            if minutes == 1 { "" } else { "s" }
        )
    } else if seconds < 86_400 {
        let hours = seconds / 3_600;
        format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" })
    } else {
        let days = seconds / 86_400;
        format!("{days} day{} ago", if days == 1 { "" } else { "s" })
    })
}
