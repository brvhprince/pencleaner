//! Memory figures the way Activity Monitor computes them, from `vm_stat` and sysctl.

use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    pub total: u64,
    /// App + wired + compressed, like Activity Monitor's "Memory Used".
    pub used: u64,
    pub app: u64,
    pub wired: u64,
    pub compressed: u64,
    /// File cache macOS hands back instantly when apps need it.
    pub cached: u64,
    pub free: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    /// "normal", "warning" or "critical".
    pub pressure: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMemory {
    pub name: String,
    /// The .app the process belongs to, for its icon.
    pub app_path: Option<String>,
    pub bytes: u64,
    pub processes: u32,
}

fn sysctl(name: &str) -> String {
    Command::new("/usr/sbin/sysctl")
        .args(["-n", name])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// "total = 7168.00M  used = 5884.50M ..." → (used, total) bytes.
fn parse_swap(s: &str) -> (u64, u64) {
    let field = |key: &str| -> u64 {
        s.split(key)
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|v| {
                let (num, unit) = v.split_at(v.len().saturating_sub(1));
                let n: f64 = num.parse().ok()?;
                Some(match unit {
                    "G" => n * 1073741824.0,
                    "M" => n * 1048576.0,
                    "K" => n * 1024.0,
                    _ => n,
                } as u64)
            })
            .unwrap_or(0)
    };
    (field("used = "), field("total = "))
}

pub fn stats() -> MemoryStats {
    let out = Command::new("/usr/bin/vm_stat").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let page: u64 = out
        .lines()
        .next()
        .and_then(|l| l.split("page size of ").nth(1))
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(4096);
    let mut pages: HashMap<String, u64> = HashMap::new();
    for line in out.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            if let Ok(n) = v.trim().trim_end_matches('.').parse::<u64>() {
                pages.insert(k.trim().trim_matches('"').to_owned(), n);
            }
        }
    }
    let p = |k: &str| pages.get(k).copied().unwrap_or(0) * page;

    let total: u64 = sysctl("hw.memsize").parse().unwrap_or(0);
    let wired = p("Pages wired down");
    let compressed = p("Pages occupied by compressor");
    let app = p("Anonymous pages").saturating_sub(p("Pages purgeable"));
    let cached = p("File-backed pages") + p("Pages purgeable");
    let used = app + wired + compressed;
    let (swap_used, swap_total) = parse_swap(&sysctl("vm.swapusage"));
    let pressure = match sysctl("kern.memorystatus_vm_pressure_level").as_str() {
        "2" => "warning",
        "4" => "critical",
        _ => "normal",
    }
    .to_owned();
    MemoryStats { total, used, app, wired, compressed, cached, free: total.saturating_sub(used + cached), swap_used, swap_total, pressure }
}

/// The app bundle a process runs from: the outermost ".app" in its path.
fn app_of(path: &str) -> Option<&str> {
    let end = path.find(".app/").map(|i| i + 4).or_else(|| path.ends_with(".app").then_some(path.len()))?;
    Some(&path[..end])
}

/// Biggest memory users, with helpers folded into their app (Chrome's renderers etc).
pub fn top_processes(limit: usize) -> Vec<ProcessMemory> {
    // Full executable paths (not -c short names) so helpers group under their app.
    let Ok(full) = Command::new("/bin/ps").args(["-axo", "rss=,comm="]).output() else { return vec![] };
    let mut by: HashMap<String, ProcessMemory> = HashMap::new();
    for line in String::from_utf8_lossy(&full.stdout).lines() {
        let line = line.trim();
        let Some((rss, cmd)) = line.split_once(' ') else { continue };
        let Ok(kb) = rss.trim().parse::<u64>() else { continue };
        let cmd = cmd.trim();
        let app = app_of(cmd);
        let key = app.unwrap_or(cmd).to_owned();
        let name = match app {
            Some(a) => a.rsplit('/').next().unwrap_or(a).trim_end_matches(".app").to_owned(),
            None => cmd.rsplit('/').next().unwrap_or(cmd).to_owned(),
        };
        let e = by.entry(key).or_insert(ProcessMemory { name, app_path: app.map(str::to_owned), bytes: 0, processes: 0 });
        e.bytes += kb * 1024;
        e.processes += 1;
    }
    let mut v: Vec<ProcessMemory> = by.into_values().collect();
    v.sort_unstable_by_key(|p| std::cmp::Reverse(p.bytes));
    v.truncate(limit);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_swap_and_app_paths() {
        assert_eq!(parse_swap("total = 7168.00M  used = 5884.50M  free = 1283.50M  (encrypted)"), (6170345472, 7516192768));
        assert_eq!(
            app_of("/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Helpers/Google Chrome Helper (Renderer).app/Contents/MacOS/x"),
            Some("/Applications/Google Chrome.app")
        );
        assert_eq!(app_of("/usr/libexec/trustd"), None);
    }

    #[test]
    fn reads_live_memory() {
        let m = stats();
        assert!(m.total > 0 && m.used > 0 && m.used <= m.total);
        assert!(!top_processes(5).is_empty());
    }
}
