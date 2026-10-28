//! Live readings for the menu bar: disk, memory, CPU, battery, network, Trash. All read
//! without root. CPU and network are rates, so a `Sampler` keeps the previous reading;
//! slow facts (model name, battery wear, Trash size) are cached.

use crate::fsutil::{self, home_dir};
use crate::performance::memory;
use crate::volumes;
use parking_lot::Mutex;
use serde::Serialize;
use std::process::Command;
use std::time::{Duration, Instant};

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
    /// 0–100, the way Activity Monitor's pressure graph means it.
    pub memory_pressure: u8,
    pub memory_level: String,
    pub swap_used: u64,
    pub cpu_load: u8,
    pub battery: Option<Battery>,
    pub net_up: u64,
    pub net_down: u64,
    pub trash: u64,
    pub uptime_secs: u64,
    /// "good", "fair" or "attention".
    pub health: String,
}

/// Battery cycle count and health %, from ioreg.
type Wear = (Option<u32>, Option<u8>);

#[derive(Default)]
struct State {
    cpu: Option<[u64; 4]>,
    net: Option<(u64, u64, Instant)>,
    model: Option<String>,
    trash: Option<(u64, Instant)>,
    battery_wear: Option<(Wear, Instant)>,
}

#[derive(Default)]
pub struct Sampler(Mutex<State>);

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// Cumulative CPU ticks: user, system, idle, nice.
fn cpu_ticks() -> Option<[u64; 4]> {
    extern "C" {
        fn mach_host_self() -> u32;
        fn host_statistics(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    }
    const HOST_CPU_LOAD_INFO: i32 = 3;
    let mut info = [0i32; 4];
    let mut count: u32 = 4;
    let r = unsafe { host_statistics(mach_host_self(), HOST_CPU_LOAD_INFO, info.as_mut_ptr(), &mut count) };
    (r == 0).then(|| info.map(|t| t as u32 as u64))
}

/// Bytes received and sent on all real interfaces (one "<Link#>" row each).
fn net_bytes() -> (u64, u64) {
    let mut down = 0;
    let mut up = 0;
    for l in run("/usr/sbin/netstat", &["-ibn"]).lines().skip(1) {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() < 10 || c[0].starts_with("lo") || !c[2].starts_with("<Link#") {
            continue;
        }
        // Rows with a MAC address have one more column than those without.
        let off = if c.len() >= 11 { 1 } else { 0 };
        down += c[5 + off].parse::<u64>().unwrap_or(0);
        up += c[8 + off].parse::<u64>().unwrap_or(0);
    }
    (down, up)
}

fn battery_now() -> Option<(u8, String, Option<String>)> {
    let text = run("/usr/bin/pmset", &["-g", "batt"]);
    let line = text.lines().find(|l| l.contains("InternalBattery"))?;
    let after = line.split_once('\t').map_or(line, |(_, r)| r);
    let parts: Vec<&str> = after.split(';').map(str::trim).collect();
    let percent = parts.first()?.trim_end_matches('%').parse().ok()?;
    let state = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let remaining = parts
        .get(2)
        .and_then(|r| r.split_whitespace().next())
        .filter(|t| t.contains(':') && *t != "0:00" && !t.contains("no"))
        .map(str::to_owned);
    Some((percent, state, remaining))
}

fn battery_wear() -> Wear {
    let text = run("/usr/sbin/ioreg", &["-rn", "AppleSmartBattery"]);
    let num = |k: &str| {
        text.lines()
            .find(|l| l.trim_start().starts_with(&format!("\"{k}\" =")))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse::<u64>().ok())
    };
    let cycles = num("CycleCount").map(|c| c as u32);
    let max = num("AppleRawMaxCapacity").or_else(|| num("NominalChargeCapacity"));
    let health = match (max, num("DesignCapacity")) {
        (Some(m), Some(d)) if d > 0 => Some(((m * 100) / d).min(100) as u8),
        _ => None,
    };
    (cycles, health)
}

fn model_name() -> String {
    let json = run("/usr/sbin/system_profiler", &["SPHardwareDataType", "-json"]);
    serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| v["SPHardwareDataType"][0]["machine_name"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "Mac".into())
}

fn uptime() -> u64 {
    let boot = run("/usr/sbin/sysctl", &["-n", "kern.boottime"]);
    let secs: u64 = boot.split("sec = ").nth(1).and_then(|s| s.split(',').next()).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    now.saturating_sub(secs)
}

impl Sampler {
    pub fn read(&self) -> Vitals {
        let mut st = self.0.lock();

        let cpu_load = match (cpu_ticks(), st.cpu) {
            (Some(now), Some(before)) => {
                let d: Vec<u64> = now.iter().zip(before).map(|(a, b)| a.saturating_sub(b)).collect();
                let total: u64 = d.iter().sum();
                let busy = d[0] + d[1] + d[3];
                st.cpu = Some(now);
                (busy * 100).checked_div(total).unwrap_or(0) as u8
            }
            (now, _) => {
                st.cpu = now;
                0
            }
        };

        let (down, up) = net_bytes();
        let t = Instant::now();
        let (net_down, net_up) = match st.net {
            Some((d0, u0, t0)) => {
                let secs = t.duration_since(t0).as_secs_f64().max(0.001);
                ((down.saturating_sub(d0) as f64 / secs) as u64, (up.saturating_sub(u0) as f64 / secs) as u64)
            }
            None => (0, 0),
        };
        st.net = Some((down, up, t));

        let model = st.model.get_or_insert_with(model_name).clone();
        let trash = match st.trash {
            Some((b, at)) if at.elapsed() < Duration::from_secs(60) => b,
            _ => {
                let b = fsutil::disk_usage(&home_dir().join(".Trash"));
                st.trash = Some((b, Instant::now()));
                b
            }
        };
        let wear = match st.battery_wear {
            Some((w, at)) if at.elapsed() < Duration::from_secs(300) => w,
            _ => {
                let w = battery_wear();
                st.battery_wear = Some((w, Instant::now()));
                w
            }
        };
        drop(st);

        let battery = battery_now().map(|(percent, state, remaining)| Battery { percent, state, remaining, cycles: wear.0, health: wear.1 });
        let mem = memory::stats();
        // kern.memorystatus_level is the share of memory still available.
        let level: u8 = run("/usr/sbin/sysctl", &["-n", "kern.memorystatus_level"]).trim().parse().unwrap_or(100);
        let disk = volumes::list().into_iter().find(|v| v.is_root);
        let (disk_name, disk_free, disk_total) = disk.map_or(("Mac".into(), 0, 0), |d| (d.name, d.free, d.total));

        let free_share = if disk_total > 0 { disk_free as f64 / disk_total as f64 } else { 1.0 };
        let health = if free_share < 0.1 || mem.pressure == "critical" {
            "attention"
        // Swap alone isn't trouble: macOS swaps idle memory even when pressure is normal.
        } else if free_share < 0.2 || mem.pressure == "warning" {
            "fair"
        } else {
            "good"
        };

        Vitals {
            model,
            disk_name,
            disk_free,
            disk_total,
            memory_pressure: 100u8.saturating_sub(level),
            memory_level: mem.pressure.clone(),
            swap_used: mem.swap_used,
            cpu_load,
            battery,
            net_up,
            net_down,
            trash,
            uptime_secs: uptime(),
            health: health.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
