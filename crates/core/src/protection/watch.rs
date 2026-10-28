//! Background watcher: a launchd agent wakes PenCleaner (`--watch-check`) only when one
//! of the launch-item folders changes. Nothing runs in between. New or changed items are
//! checked (signature, location, XProtect) and reported with a notification.

use super::{malware, persistence};
use crate::fsutil::{home_dir, run_with_timeout};
use crate::performance::items::{self, LaunchItem};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const LABEL: &str = "com.pen.cleaner.watch";
pub const FLAG: &str = "--watch-check";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub at: i64,
    /// launchd label of the new item.
    pub id: String,
    pub name: String,
    pub plist: String,
    pub program: Option<String>,
    /// "high", "medium", or None when nothing looked wrong.
    pub severity: Option<String>,
    pub reasons: Vec<String>,
    /// XProtect's verdict on the program, if it matched.
    pub malware: Option<String>,
}

fn watched_dirs() -> [PathBuf; 3] {
    [home_dir().join("Library/LaunchAgents"), PathBuf::from("/Library/LaunchAgents"), PathBuf::from("/Library/LaunchDaemons")]
}

pub fn data_dir() -> PathBuf {
    crate::fsutil::data_dir()
}

fn agent_path() -> PathBuf {
    home_dir().join(format!("Library/LaunchAgents/{LABEL}.plist"))
}

/// plist path → modification time, for every watched folder.
fn snapshot() -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for dir in watched_dirs() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "plist") && !p.ends_with(format!("{LABEL}.plist")) {
                let mtime = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                out.insert(p.to_string_lossy().into_owned(), mtime);
            }
        }
    }
    out
}

fn state_file(dir: &Path) -> PathBuf {
    dir.join("watch-state.json")
}

fn alerts_file(dir: &Path) -> PathBuf {
    dir.join("alerts.jsonl")
}

fn save_snapshot(dir: &Path, snap: &BTreeMap<String, i64>) {
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(state_file(dir), serde_json::to_vec(snap).unwrap_or_default());
}

/// New or modified plists since the last look, newest snapshot saved.
fn changes(dir: &Path) -> Vec<String> {
    let now = snapshot();
    let before: Option<BTreeMap<String, i64>> = fs::read(state_file(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok());
    save_snapshot(dir, &now);
    let Some(before) = before else { return vec![] };
    now.into_iter().filter(|(p, m)| before.get(p) != Some(m)).map(|(p, _)| p).collect()
}

fn analyze(item: &LaunchItem, engine: Option<&malware::Engine>) -> Alert {
    let online = crate::settings::load(&data_dir()).online_notarization;
    let finding = persistence::analyze(item, online);
    let program = finding.as_ref().map(|f| f.program.clone()).or_else(|| item.executable.clone());
    let malware = match (engine, program.as_deref()) {
        (Some(e), Some(p)) if malware::worth_scanning(Path::new(p)) => {
            let mut s = e.scanner();
            e.scan_file(&mut s, Path::new(p)).into_iter().next().map(|d| d.rule)
        }
        _ => None,
    };
    let severity = if malware.is_some() {
        Some("high".to_owned())
    } else {
        finding.as_ref().map(|f| format!("{:?}", f.severity).to_lowercase())
    };
    Alert {
        at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
        id: item.id.clone(),
        name: item.name.clone(),
        plist: item.plist.clone().unwrap_or_default(),
        program,
        severity,
        reasons: finding.map(|f| f.reasons).unwrap_or_default(),
        malware,
    }
}

fn notify(a: &Alert) {
    let (title, body) = match (&a.malware, a.severity.as_deref()) {
        (Some(rule), _) => ("Malware tried to install itself".to_owned(), format!("{} matches {rule}. Open PenCleaner → Protection to remove it.", a.name)),
        (None, Some(_)) => ("Suspicious background item added".to_owned(), format!("{}: {}", a.name, a.reasons.first().cloned().unwrap_or_default())),
        (None, None) => ("New background item".to_owned(), format!("{} will now run in the background.", a.name)),
    };
    post_notification(&title, &body);
}

/// A macOS notification titled "PenCleaner".
pub fn post_notification(subtitle: &str, body: &str) {
    let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("display notification \"{}\" with title \"PenCleaner\" subtitle \"{}\"", q(body), q(subtitle));
    let _ = run_with_timeout("/usr/bin/osascript", &["-e", &script], Duration::from_secs(10));
}

/// What launchd runs. Records and announces anything new; returns the alerts.
pub fn run_check(dir: &Path) -> Vec<Alert> {
    let changed: HashSet<String> = changes(dir).into_iter().collect();
    if changed.is_empty() {
        return vec![];
    }
    let none = HashSet::new();
    let new_items: Vec<LaunchItem> = items::from_plists(&none, &none).into_iter().filter(|i| i.plist.as_ref().is_some_and(|p| changed.contains(p))).collect();
    let engine = malware::Engine::load().ok();
    let alerts: Vec<Alert> = new_items.iter().map(|i| analyze(i, engine.as_ref())).collect();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(alerts_file(dir)) {
        for a in &alerts {
            let _ = writeln!(f, "{}", serde_json::to_string(a).unwrap_or_default());
        }
    }
    for a in &alerts {
        notify(a);
    }
    alerts
}

pub fn alerts(dir: &Path) -> Vec<Alert> {
    let Ok(f) = fs::File::open(alerts_file(dir)) else { return vec![] };
    let mut v: Vec<Alert> = std::io::BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str(&l).ok()).collect();
    v.reverse();
    v
}

pub fn clear_alerts(dir: &Path) {
    let _ = fs::remove_file(alerts_file(dir));
}

pub fn is_enabled() -> bool {
    agent_path().exists()
}

fn agent_plist(exe: &Path) -> plist::Value {
    let mut d = plist::Dictionary::new();
    d.insert("Label".into(), LABEL.into());
    d.insert("ProgramArguments".into(), plist::Value::Array(vec![exe.to_string_lossy().into_owned().into(), FLAG.into()]));
    d.insert("WatchPaths".into(), plist::Value::Array(watched_dirs().iter().map(|p| p.to_string_lossy().into_owned().into()).collect()));
    d.insert("ProcessType".into(), "Background".into());
    d.insert("LowPriorityIO".into(), true.into());
    d.insert("Nice".into(), 10.into());
    // A burst of installer writes wakes us at most every 10 seconds.
    d.insert("ThrottleInterval".into(), 10.into());
    plist::Value::Dictionary(d)
}

/// Installs and loads the agent; the current folders become the baseline, so only
/// later additions alert. `exe` is the PenCleaner binary launchd should run.
pub fn enable(exe: &Path, dir: &Path) -> Result<(), String> {
    let path = agent_path();
    fs::create_dir_all(path.parent().ok_or("bad path")?).map_err(|e| e.to_string())?;
    agent_plist(exe).to_file_xml(&path).map_err(|e| e.to_string())?;
    save_snapshot(dir, &snapshot());
    let uid = unsafe { libc::getuid() };
    let domain = format!("gui/{uid}");
    let _ = run_with_timeout("/bin/launchctl", &["bootout", &format!("{domain}/{LABEL}")], Duration::from_secs(10));
    run_with_timeout("/bin/launchctl", &["bootstrap", &domain, &path.to_string_lossy()], Duration::from_secs(10))
        .map(|_| ())
        .ok_or_else(|| "launchd didn't accept the watcher".into())
}

/// Keeps an enabled watcher pointed at the PenCleaner that's running now (a moved or
/// rebuilt app would otherwise leave launchd calling a stale binary). The baseline stays.
pub fn repoint(exe: &Path) -> Result<bool, String> {
    let path = agent_path();
    let Some(current) = plist::Value::from_file(&path).ok().and_then(|v| {
        v.as_dictionary()?.get("ProgramArguments")?.as_array()?.first()?.as_string().map(PathBuf::from)
    }) else {
        return Ok(false);
    };
    if current == exe {
        return Ok(false);
    }
    agent_plist(exe).to_file_xml(&path).map_err(|e| e.to_string())?;
    let domain = format!("gui/{}", unsafe { libc::getuid() });
    let _ = run_with_timeout("/bin/launchctl", &["bootout", &format!("{domain}/{LABEL}")], Duration::from_secs(10));
    run_with_timeout("/bin/launchctl", &["bootstrap", &domain, &path.to_string_lossy()], Duration::from_secs(10))
        .map(|_| true)
        .ok_or_else(|| "launchd didn't accept the watcher".into())
}

pub fn disable() -> Result<(), String> {
    let uid = unsafe { libc::getuid() };
    let _ = run_with_timeout("/bin/launchctl", &["bootout", &format!("gui/{uid}/{LABEL}")], Duration::from_secs(10));
    match fs::remove_file(agent_path()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_look_is_a_baseline_then_changes_show() {
        let dir = tempfile::tempdir().unwrap();
        assert!(changes(dir.path()).is_empty(), "first run only records");
        // Simulate a new item by editing the saved snapshot to forget one entry.
        let mut snap: BTreeMap<String, i64> = serde_json::from_slice(&fs::read(state_file(dir.path())).unwrap()).unwrap();
        if let Some(first) = snap.keys().next().cloned() {
            snap.remove(&first);
            save_snapshot(dir.path(), &snap);
            assert_eq!(changes(dir.path()), vec![first]);
        }
        assert!(changes(dir.path()).is_empty());
    }

    #[test]
    fn agent_wakes_only_on_folder_changes() {
        let v = agent_plist(Path::new("/Applications/PenCleaner.app/Contents/MacOS/pencleaner"));
        let d = v.as_dictionary().unwrap();
        assert_eq!(d.get("WatchPaths").unwrap().as_array().unwrap().len(), 3);
        assert!(d.get("KeepAlive").is_none() && d.get("RunAtLoad").is_none(), "never resident");
        assert_eq!(d.get("ProgramArguments").unwrap().as_array().unwrap()[1].as_string(), Some(FLAG));
    }

    #[test]
    fn alerts_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let a = Alert { at: 1, id: "x".into(), name: "x".into(), plist: "/p".into(), program: None, severity: None, reasons: vec![], malware: None };
        fs::write(alerts_file(dir.path()), format!("{}\n", serde_json::to_string(&a).unwrap())).unwrap();
        assert_eq!(alerts(dir.path()), vec![a]);
        clear_alerts(dir.path());
        assert!(alerts(dir.path()).is_empty());
    }
}
