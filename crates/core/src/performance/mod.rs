//! Performance: memory, login and background items, Time Machine snapshots, and the
//! maintenance scripts that used to need Terminal.

pub mod items;
pub mod memory;
pub mod snapshots;

use crate::bytes_label;
use crate::fsutil::home_dir;
use crate::privileged;
use items::LaunchItem;
use memory::{MemoryStats, ProcessMemory};
use serde::Serialize;
use snapshots::Snapshot;
use std::path::PathBuf;
use std::process::Command;

const LSREGISTER: &str =
    "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
const GB: u64 = 1_000_000_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub admin: bool,
    /// Takes minutes or keeps working in the background afterwards.
    pub slow: bool,
    pub recommended: bool,
    /// Why it's recommended right now.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfReport {
    pub memory: MemoryStats,
    pub processes: Vec<ProcessMemory>,
    pub items: Vec<LaunchItem>,
    pub snapshots: Vec<Snapshot>,
    pub purgeable: Option<u64>,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResult {
    pub id: String,
    pub ok: bool,
    pub message: String,
}

fn mail_index() -> Option<PathBuf> {
    let mail = home_dir().join("Library/Mail");
    let mut stores: Vec<(u32, PathBuf)> = std::fs::read_dir(&mail)
        .ok()?
        .flatten()
        .filter_map(|e| Some((e.file_name().to_string_lossy().strip_prefix('V')?.parse().ok()?, e.path())))
        .collect();
    stores.sort();
    let index = stores.pop()?.1.join("MailData/Envelope Index");
    index.exists().then_some(index)
}

fn tasks(mem: &MemoryStats, snaps: &[Snapshot], purgeable: Option<u64>) -> Vec<Task> {
    let swap_heavy = mem.swap_used > GB;
    let ram_reason = if mem.pressure != "normal" {
        Some(format!("Memory pressure is {}.", mem.pressure))
    } else if swap_heavy {
        Some(format!("{} of memory has spilled into swap on disk.", bytes_label(mem.swap_used)))
    } else {
        None
    };
    let mail_size = mail_index().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len());
    let snap_reason = (!snaps.is_empty()).then(|| match purgeable {
        Some(p) if p > GB => format!("{} local snapshots are holding about {}.", snaps.len(), bytes_label(p)),
        _ => format!("{} local snapshots on this Mac.", snaps.len()),
    });
    vec![
        Task {
            id: "free-ram",
            title: "Free Up RAM",
            description: "Asks macOS to drop inactive memory and file caches so apps that need memory get it without swapping.",
            admin: true,
            slow: false,
            recommended: ram_reason.is_some(),
            reason: ram_reason,
        },
        Task {
            id: "flush-dns",
            title: "Flush DNS Cache",
            description: "Clears remembered website addresses. Fixes sites that won't load after a network or DNS change.",
            admin: true,
            slow: false,
            recommended: true,
            reason: Some("Quick and harmless.".into()),
        },
        Task {
            id: "thin-snapshots",
            title: "Remove Time Machine Snapshots",
            description: "Deletes local snapshots. Backups on your Time Machine drive are not touched.",
            admin: true,
            slow: false,
            recommended: purgeable.is_some_and(|p| p > 10 * GB) && !snaps.is_empty(),
            reason: snap_reason,
        },
        Task {
            id: "quicklook",
            title: "Reset Quick Look",
            description: "Clears cached file previews and thumbnails. Fixes previews that are stale or blank.",
            admin: false,
            slow: false,
            recommended: true,
            reason: Some("Quick and harmless.".into()),
        },
        Task {
            id: "speed-up-mail",
            title: "Speed Up Mail",
            description: "Compacts Mail's message database. Quit Mail first.",
            admin: false,
            slow: false,
            recommended: mail_size > 200 * 1000 * 1000,
            reason: (mail_size > 0).then(|| format!("Mail's database is {}.", bytes_label(mail_size))),
        },
        Task {
            id: "rebuild-launch-services",
            title: "Rebuild Launch Services",
            description: "Re-registers apps. Fixes duplicate entries in Open With and documents opening in the wrong app.",
            admin: false,
            slow: true,
            recommended: false,
            reason: None,
        },
        Task {
            id: "reindex-spotlight",
            title: "Reindex Spotlight",
            description: "Rebuilds the search index from scratch. Fixes missing search results; searching is incomplete and the Mac busier until it finishes.",
            admin: true,
            slow: true,
            recommended: false,
            reason: None,
        },
        Task {
            id: "font-cache",
            title: "Clear Font Caches",
            description: "Removes your font caches. Fixes garbled or missing fonts; takes effect after you log out.",
            admin: false,
            slow: false,
            recommended: false,
            reason: None,
        },
    ]
}

pub fn scan(open_at_login_apps: bool) -> PerfReport {
    let memory = memory::stats();
    let snapshots = snapshots::list();
    let purgeable = snapshots::purgeable();
    PerfReport {
        tasks: tasks(&memory, &snapshots, purgeable),
        processes: memory::top_processes(10),
        items: items::list(open_at_login_apps),
        memory,
        snapshots,
        purgeable,
    }
}

fn argv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn run_user(cmds: &[Vec<String>]) -> Result<(), String> {
    for c in cmds {
        let out = Command::new(&c[0]).args(&c[1..]).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or("failed").to_owned());
        }
    }
    Ok(())
}

/// Commands for a task; root ones are batched behind one password prompt.
fn commands(id: &str) -> Result<Vec<Vec<String>>, String> {
    Ok(match id {
        "free-ram" => vec![argv(&["/usr/sbin/purge"])],
        "flush-dns" => vec![argv(&["/usr/bin/dscacheutil", "-flushcache"]), argv(&["/usr/bin/killall", "-HUP", "mDNSResponder"])],
        "thin-snapshots" => snapshots::delete_commands(&[]),
        "reindex-spotlight" => vec![argv(&["/usr/bin/mdutil", "-E", "/"])],
        "quicklook" => vec![argv(&["/usr/bin/qlmanage", "-r", "cache"]), argv(&["/usr/bin/qlmanage", "-r"])],
        "rebuild-launch-services" => vec![
            argv(&[LSREGISTER, "-gc"]),
            argv(&[LSREGISTER, "-r", "-f", "-domain", "local", "-domain", "system", "-domain", "user"]),
        ],
        "font-cache" => vec![argv(&["/usr/bin/atsutil", "databases", "-removeUser"])],
        "speed-up-mail" => {
            if crate::apps::running_bundle_ids().contains("com.apple.mail") {
                return Err("Quit Mail first.".into());
            }
            let index = mail_index().ok_or("Mail's database isn't readable. Grant Full Disk Access.")?;
            vec![vec!["/usr/bin/sqlite3".into(), index.to_string_lossy().into_owned(), "VACUUM;".into()]]
        }
        other => return Err(format!("Unknown task {other}")),
    })
}

/// Runs the tasks and reports each outcome. `before` and `after` memory let the UI say
/// how much RAM was actually freed.
pub fn run_tasks(ids: &[String], mut on_start: impl FnMut(&str)) -> Vec<TaskResult> {
    let known = tasks(&MemoryStats::default(), &[], None);
    let admin_ids: Vec<&String> = ids.iter().filter(|id| known.iter().any(|t| t.id == id.as_str() && t.admin)).collect();
    let mut results = Vec::new();

    // User-level tasks first: they never prompt.
    for id in ids.iter().filter(|id| !admin_ids.contains(id)) {
        on_start(id);
        let r = commands(id).and_then(|c| run_user(&c));
        results.push(TaskResult { id: id.clone(), ok: r.is_ok(), message: r.err().unwrap_or_else(|| "Done.".into()) });
    }

    if !admin_ids.is_empty() {
        let before = memory::stats();
        let mut batch = Vec::new();
        let mut planned = Vec::new();
        for id in &admin_ids {
            on_start(id);
            match commands(id) {
                Ok(c) => {
                    batch.extend(c);
                    planned.push((*id).clone());
                }
                Err(e) => results.push(TaskResult { id: (*id).clone(), ok: false, message: e }),
            }
        }
        let outcome = privileged::run_as_admin(&batch, "PenCleaner needs your password to run maintenance tasks.");
        let after = memory::stats();
        for id in planned {
            let message = match (&outcome, id.as_str()) {
                (Err(e), _) => e.clone(),
                (Ok(()), "free-ram") => {
                    let freed = before.used.saturating_sub(after.used);
                    if freed > 0 { format!("Freed {}.", bytes_label(freed)) } else { "Done. Memory was already in good shape.".into() }
                }
                (Ok(()), "reindex-spotlight") => "Started. Spotlight keeps reindexing in the background.".into(),
                (Ok(()), _) => "Done.".into(),
            };
            results.push(TaskResult { id, ok: outcome.is_ok(), message });
        }
    }
    results
}

/// Deletes specific snapshots, or all of them when `ids` is empty.
pub fn delete_snapshots(ids: &[String]) -> Result<(), String> {
    privileged::run_as_admin(&snapshots::delete_commands(ids), "PenCleaner needs your password to remove Time Machine snapshots.")
}
