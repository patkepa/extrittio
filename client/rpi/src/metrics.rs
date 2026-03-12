use std::collections::HashMap;

/// Reads CPU temperature from thermal zone (°C).
/// Falls back to 0.0 if unavailable.
pub fn cpu_temperature() -> f32 {
    // Try all thermal zones, prefer the first one
    for path in &[
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/thermal/thermal_zone1/temp",
    ] {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(millidegrees) = contents.trim().parse::<f32>() {
                return millidegrees / 1000.0;
            }
        }
    }
    0.0
}

/// Reads memory usage as a percentage (0-100).
/// Parses /proc/meminfo for MemTotal and MemAvailable.
pub fn memory_usage_percent() -> f32 {
    let Ok(contents) = std::fs::read_to_string("/proc/meminfo") else {
        return 0.0;
    };

    let mut total: Option<u64> = None;
    let mut available: Option<u64> = None;

    for line in contents.lines() {
        if line.starts_with("MemTotal:") {
            total = parse_meminfo_kb(line);
        } else if line.starts_with("MemAvailable:") {
            available = parse_meminfo_kb(line);
        }
        if total.is_some() && available.is_some() {
            break;
        }
    }

    match (total, available) {
        (Some(t), Some(a)) if t > 0 => ((t - a) as f32 / t as f32) * 100.0,
        _ => 0.0,
    }
}

fn parse_meminfo_kb(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1)?.parse().ok()
}

/// CPU usage snapshot for computing delta between two readings.
#[derive(Clone)]
pub struct CpuSnapshot {
    idle: u64,
    total: u64,
}

impl CpuSnapshot {
    /// Takes a snapshot of /proc/stat CPU counters.
    pub fn take() -> Option<Self> {
        let contents = std::fs::read_to_string("/proc/stat").ok()?;
        let cpu_line = contents.lines().next()?;
        if !cpu_line.starts_with("cpu ") {
            return None;
        }

        let fields: Vec<u64> = cpu_line
            .split_whitespace()
            .skip(1) // skip "cpu"
            .filter_map(|s| s.parse().ok())
            .collect();

        if fields.len() < 4 {
            return None;
        }

        let idle = fields[3] + fields.get(4).copied().unwrap_or(0); // idle + iowait
        let total: u64 = fields.iter().sum();
        Some(Self { idle, total })
    }

    /// Computes CPU usage percentage between two snapshots.
    pub fn usage_since(&self, prev: &CpuSnapshot) -> f32 {
        let total_delta = self.total.saturating_sub(prev.total);
        let idle_delta = self.idle.saturating_sub(prev.idle);

        if total_delta == 0 {
            return 0.0;
        }

        ((total_delta - idle_delta) as f32 / total_delta as f32) * 100.0
    }
}

/// Collects extended system metrics into a string-string map for the metadata field.
pub fn extended_metrics() -> HashMap<String, String> {
    let mut m = HashMap::new();

    // Load averages
    if let Ok(contents) = std::fs::read_to_string("/proc/loadavg") {
        let parts: Vec<&str> = contents.split_whitespace().collect();
        if parts.len() >= 3 {
            m.insert("load_1m".into(), parts[0].into());
            m.insert("load_5m".into(), parts[1].into());
            m.insert("load_15m".into(), parts[2].into());
        }
    }

    // Disk usage for root filesystem
    if let Some((total_gb, used_percent)) = disk_usage("/") {
        m.insert("disk_total_gb".into(), format!("{total_gb:.1}"));
        m.insert("disk_used_percent".into(), format!("{used_percent:.1}"));
    }

    // Network RX/TX bytes (sum across all non-lo interfaces)
    if let Some((rx, tx)) = network_bytes() {
        m.insert("net_rx_bytes".into(), rx.to_string());
        m.insert("net_tx_bytes".into(), tx.to_string());
    }

    // Uptime
    if let Ok(contents) = std::fs::read_to_string("/proc/uptime") {
        if let Some(uptime) = contents.split_whitespace().next() {
            m.insert("system_uptime_secs".into(), uptime.into());
        }
    }

    // Number of processes
    if let Ok(entries) = std::fs::read_dir("/proc") {
        let count = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|s| s.chars().all(|c| c.is_ascii_digit()))
            })
            .count();
        m.insert("process_count".into(), count.to_string());
    }

    m
}

fn disk_usage(path: &str) -> Option<(f64, f64)> {
    use std::ffi::CString;
    let c_path = CString::new(path).ok()?;

    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) != 0 {
            return None;
        }
        let block_size = stat.f_frsize as f64;
        let total = stat.f_blocks as f64 * block_size;
        let available = stat.f_bavail as f64 * block_size;
        let used = total - available;
        let total_gb = total / 1_073_741_824.0;
        let used_percent = if total > 0.0 {
            (used / total) * 100.0
        } else {
            0.0
        };
        Some((total_gb, used_percent))
    }
}

fn network_bytes() -> Option<(u64, u64)> {
    let contents = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut rx_total: u64 = 0;
    let mut tx_total: u64 = 0;

    for line in contents.lines().skip(2) {
        // Skip header lines
        let Some((iface, stats)) = line.split_once(':') else {
            continue;
        };
        let iface = iface.trim();
        if iface == "lo" {
            continue;
        }
        let fields: Vec<&str> = stats.split_whitespace().collect();
        if fields.len() >= 9 {
            rx_total += fields[0].parse::<u64>().unwrap_or(0);
            tx_total += fields[8].parse::<u64>().unwrap_or(0);
        }
    }

    Some((rx_total, tx_total))
}
