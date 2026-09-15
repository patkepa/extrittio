use std::collections::HashMap;

use sysinfo::{Networks, System};

/// Holds sysinfo + battery state across telemetry cycles.
/// Created once at startup; call `refresh()` before each reading.
pub struct MetricsCollector {
    system: System,
    networks: Networks,
    battery_manager: Option<battery::Manager>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let mut system = System::new();
        // Initial CPU refresh to prime the two-sample measurement.
        system.refresh_cpu_all();

        // Networks must be a separate struct in sysinfo 0.34.
        let networks = Networks::new_with_refreshed_list();

        let battery_manager = battery::Manager::new()
            .map_err(|e| {
                tracing::warn!("Battery manager init failed, battery metrics disabled: {e}")
            })
            .ok();

        Self {
            system,
            networks,
            battery_manager,
        }
    }

    /// Refresh system metrics. Call once per telemetry cycle.
    pub fn refresh(&mut self) {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        // refresh(true) also refreshes the interface list (detects new interfaces like VPN)
        self.networks.refresh(true);
    }

    /// Battery level as a percentage; absent when no battery is available.
    pub fn battery_level(&self) -> Option<f32> {
        self.first_battery()
            .map(|b| b.state_of_charge().get::<battery::units::ratio::percent>())
    }

    /// Collect system readings; the event builder preserves numeric JSON types.
    pub fn extended_metrics(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();

        // CPU usage (global average across all cores)
        let cpu_usage = self.system.global_cpu_usage();
        m.insert("cpu_usage_percent".into(), format!("{cpu_usage:.1}"));

        // Memory
        let total_mem = self.system.total_memory();
        let used_mem = self.system.used_memory();
        if total_mem > 0 {
            let mem_pct = (used_mem as f64 / total_mem as f64) * 100.0;
            m.insert("memory_usage_percent".into(), format!("{mem_pct:.1}"));
        }
        let total_gb = total_mem as f64 / 1_073_741_824.0;
        m.insert("memory_total_gb".into(), format!("{total_gb:.1}"));

        // Network (sum across all non-lo interfaces via separate Networks struct)
        let mut rx_total: u64 = 0;
        let mut tx_total: u64 = 0;
        for (iface, data) in &self.networks {
            if iface == "lo0" {
                continue;
            }
            rx_total += data.total_received();
            tx_total += data.total_transmitted();
        }
        m.insert("net_rx_bytes".into(), rx_total.to_string());
        m.insert("net_tx_bytes".into(), tx_total.to_string());

        // Battery readings are absent on devices without a battery.
        if let Some(bat) = self.first_battery() {
            let state = match bat.state() {
                battery::State::Charging => "charging",
                battery::State::Discharging => "discharging",
                battery::State::Full => "full",
                _ => "unknown",
            };
            m.insert("battery_state".into(), state.into());
            if let Some(cycles) = bat.cycle_count() {
                m.insert("battery_cycles".into(), cycles.to_string());
            }
            let health = bat
                .state_of_health()
                .get::<battery::units::ratio::percent>();
            m.insert("battery_health_percent".into(), format!("{health:.1}"));
        }

        // System info
        m.insert("system_uptime_secs".into(), System::uptime().to_string());
        m.insert(
            "hostname".into(),
            System::host_name().unwrap_or_else(|| "unknown".into()),
        );
        m.insert(
            "os_version".into(),
            format!(
                "macOS {}",
                System::os_version().unwrap_or_else(|| "unknown".into())
            ),
        );

        // Chip model via sysctl (sysinfo returns generic "Apple" on ARM)
        m.insert("chip_model".into(), chip_model());

        // Load averages via sysinfo (wraps sysctl on macOS internally)
        let load = System::load_average();
        m.insert("load_1m".into(), format!("{:.2}", load.one));
        m.insert("load_5m".into(), format!("{:.2}", load.five));
        m.insert("load_15m".into(), format!("{:.2}", load.fifteen));

        m
    }

    fn first_battery(&self) -> Option<battery::Battery> {
        self.battery_manager
            .as_ref()
            .and_then(|mgr| mgr.batteries().ok())
            .and_then(|mut iter| iter.next())
            .and_then(|result| result.ok())
    }
}

/// Read chip model via sysctl machdep.cpu.brand_string.
fn chip_model() -> String {
    std::process::Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}
