//! Live readings for the menu bar / tray popover: disk, memory, CPU, battery, network,
//! Trash or Recycle Bin. Read per platform without admin rights; the shape and the
//! "Mac Health" verdict are shared.

use serde::Serialize;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Sampler;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Sampler;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    /// "charging", "charged", "discharging", "finishing charge"…
    pub state: String,
    pub remaining: Option<String>,
    pub cycles: Option<u32>,
    /// Full-charge capacity as a share of the original design, e.g. 73.
    pub health: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vitals {
    pub model: String,
    pub disk_name: String,
    pub disk_free: u64,
    pub disk_total: u64,
    /// 0–100, the way Activity Monitor's pressure graph means it (Windows: memory load).
    pub memory_pressure: u8,
    pub memory_level: String,
    pub swap_used: u64,
    pub cpu_load: u8,
    pub battery: Option<Battery>,
    pub net_up: u64,
    pub net_down: u64,
    pub trash: u64,
    pub uptime_secs: u64,
    /// CPU temperature and fans, where the machine exposes them (Intel Macs; see `sensors`).
    pub sensors: Option<crate::sensors::Sensors>,
    /// "good", "fair" or "attention".
    pub health: String,
}

/// "good", "fair" or "attention", from free disk space, memory pressure ("normal",
/// "warning", "critical") and CPU temperature (0 when unknown).
pub fn health(disk_free: u64, disk_total: u64, pressure: &str, celsius: f32) -> String {
    let free_share = if disk_total > 0 { disk_free as f64 / disk_total as f64 } else { 1.0 };
    if free_share < 0.1 || pressure == "critical" || celsius >= 100.0 {
        "attention"
    } else if free_share < 0.2 || pressure == "warning" || celsius >= 90.0 {
        "fair"
    } else {
        "good"
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn reads_live_vitals_and_rates() {
        let s = Sampler::default();
        let first = s.read();
        assert!(first.disk_total > 0 && first.memory_pressure <= 100);
        assert_eq!(first.cpu_load, 0, "first read has no previous sample");
        std::thread::sleep(Duration::from_millis(300));
        let second = s.read();
        assert!(second.cpu_load <= 100);
        assert!(!second.model.is_empty() && second.uptime_secs > 0);
    }
}
