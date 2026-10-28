//! Login and background items, read without any password: launchd plists from disk,
//! login items and services that apps register from inside their bundles (matched
//! against what launchd has loaded), and classic "Open at Login" apps.
//!
//! `sfltool dumpbtm` has the full Background Task Management list (what System Settings
//! shows, with developer names and switched-off items), but on recent macOS it needs an
//! administrator password. So it's never part of a scan, only `full_list`, which the
//! user starts and approves.

use crate::privileged;
use crate::remove::{RemoveMode, Remover};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// Opens with your session (SMAppService login items and "Open at Login" apps).
    LoginItem,
    /// Runs in the background for your user.
    Agent,
    /// Runs in the background for the whole Mac, as root.
    Daemon,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchItem {
    pub id: String,
    pub name: String,
    pub developer: Option<String>,
    pub kind: ItemKind,
    /// A launchd plist we can disable or remove. Otherwise it was registered by its app
    /// and only System Settings can switch it off.
    pub controllable: bool,
    pub enabled: bool,
    pub running: bool,
    pub plist: Option<String>,
    pub executable: Option<String>,
    /// The app it belongs to (for its icon and name).
    pub app_path: Option<String>,
    pub app_name: Option<String>,
    /// Its plist or program no longer exists.
    pub broken: bool,
    /// PenCleaner's own (the watcher, Open at Login): switched in PenCleaner's Settings, not here.
    pub is_self: bool,
}

#[derive(Default)]
struct Rec {
    fields: HashMap<String, String>,
}

impl Rec {
    fn get(&self, k: &str) -> Option<&str> {
        self.fields.get(k).map(String::as_str).filter(|v| *v != "(null)" && !v.is_empty())
    }
}

/// Splits `sfltool dumpbtm` into records for the given UIDs.
fn parse(text: &str, uids: &[i64]) -> Vec<Rec> {
    let mut out = Vec::new();
    let mut take = false;
    let mut cur: Option<Rec> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Records for UID ") {
            out.extend(cur.take());
            let uid: i64 = rest.split_whitespace().next().and_then(|u| u.parse().ok()).unwrap_or(i64::MIN);
            take = uids.contains(&uid);
            continue;
        }
        if !take {
            continue;
        }
        // Record headers sit at one space of indent ("#12:"); embedded ids are deeper.
        if line.starts_with(" #") && t.ends_with(':') {
            out.extend(cur.take());
            cur = Some(Rec::default());
            continue;
        }
        if let (Some(rec), Some((k, v))) = (cur.as_mut(), t.split_once(": ")) {
            if !k.starts_with('#') {
                rec.fields.insert(k.trim().to_owned(), v.trim().to_owned());
            }
        }
    }
    out.extend(cur);
    out
}

fn decode_url(url: &str) -> String {
    let raw = url.strip_prefix("file://").unwrap_or(url);
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).trim_end_matches('/').to_owned()
}

/// "8.homebrew.mxcl.kafka" → "homebrew.mxcl.kafka"
fn label_of(identifier: &str) -> &str {
    match identifier.split_once('.') {
        Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => identifier,
    }
}

/// Items from a BTM dump, the way System Settings sees them.
fn from_btm(dump: &str, running: &HashSet<String>) -> Vec<LaunchItem> {
    let uid = unsafe { libc::getuid() } as i64;
    let recs = parse(dump, &[uid, -2, 0]);
    let apps: HashMap<&str, (&str, String)> = recs
        .iter()
        .filter(|r| r.get("Type").is_some_and(|t| t.starts_with("app")))
        .filter_map(|r| Some((r.get("Identifier")?, (r.get("Name").unwrap_or(""), decode_url(r.get("URL")?)))))
        .collect();
    recs.iter()
        .filter_map(|r| {
            let ty = r.get("Type")?;
            let kind = if ty.starts_with("login item") {
                ItemKind::LoginItem
            } else if ty.contains("daemon") {
                ItemKind::Daemon
            } else if ty.contains("agent") {
                ItemKind::Agent
            } else {
                return None;
            };
            let legacy = ty.starts_with("legacy");
            let parent = r.get("Parent Identifier").and_then(|p| apps.get(p));
            let resolve = |rel: &str| {
                let p = decode_url(rel);
                match parent {
                    Some((_, app)) if !p.starts_with('/') => format!("{app}/{p}"),
                    _ => p,
                }
            };
            let label = label_of(r.get("Identifier")?).to_owned();
            let plist = legacy.then(|| r.get("URL").map(resolve)).flatten();
            let executable = r.get("Executable Path").map(resolve);
            let disposition = r.get("Disposition").unwrap_or("");
            Some(LaunchItem {
                running: running.contains(&label),
                enabled: disposition.starts_with("[enabled") || disposition.contains(" enabled"),
                name: r.get("Name").map(str::to_owned).unwrap_or_else(|| label.clone()),
                developer: r.get("Developer Name").map(str::to_owned),
                kind,
                controllable: legacy && plist.is_some(),
                broken: plist.as_deref().is_some_and(|p| !Path::new(p).exists()),
                is_self: false,
                app_name: parent.map(|(n, _)| (*n).to_owned()).filter(|n| !n.is_empty()),
                app_path: parent.map(|(_, a)| a.clone()),
                id: label,
                plist,
                executable,
            })
        })
        .collect()
}

/// The scan list plus everything System Settings knows: developer names, its on/off
/// state, and items apps registered but you switched off. Asks for the administrator
/// password (sfltool needs it), so only run it when the user asks.
pub fn full_list(open_at_login_apps: bool) -> Result<Vec<LaunchItem>, String> {
    let dump = privileged::run_as_admin_output(
        &[vec!["/usr/bin/sfltool".into(), "dumpbtm".into()]],
        "PenCleaner needs your password to read the full list of login and background items from macOS.",
        Some(std::time::Duration::from_secs(180)),
    )?;
    let (_, running) = launchd_state();
    let btm = from_btm(&dump, &running);
    if btm.is_empty() {
        return Err("macOS returned an empty list.".into());
    }
    Ok(merge(list(open_at_login_apps), btm))
}

/// Folds BTM details into the scan list; BTM-only items are appended.
fn merge(mut items: Vec<LaunchItem>, btm: Vec<LaunchItem>) -> Vec<LaunchItem> {
    for b in btm {
        match items.iter_mut().find(|i| i.id == b.id) {
            Some(i) => {
                i.developer = i.developer.take().or(b.developer);
                i.app_name = i.app_name.take().or(b.app_name);
                i.app_path = i.app_path.take().or(b.app_path);
                // App-registered items: System Settings' switch is the truth.
                if !i.controllable {
                    i.enabled = b.enabled;
                }
            }
            None => items.push(b),
        }
    }
    items.sort_by_key(|i| (i.kind as u8, !i.enabled, i.name.to_lowercase()));
    items
}

const SELF_MSG: &str = "That's one of PenCleaner's own items. Change it in PenCleaner's Settings.";

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// Labels launchd has been told to keep disabled.
fn disabled_labels(domain: &str) -> HashSet<String> {
    run("/bin/launchctl", &["print-disabled", domain])
        .lines()
        .filter(|l| l.contains("=> disabled") || l.contains("=> true"))
        .filter_map(|l| l.split('"').nth(1).map(str::to_owned))
        .collect()
}

/// Labels launchd has loaded, and those with a live process, in the user and system
/// domains. Loaded = enabled, even when not running right now.
fn launchd_state() -> (HashSet<String>, HashSet<String>) {
    let (mut loaded, mut running) = (HashSet::new(), HashSet::new());
    for l in run("/bin/launchctl", &["list"]).lines().skip(1) {
        let mut cols = l.split_whitespace();
        if let (Some(pid), Some(_), Some(label)) = (cols.next(), cols.next(), cols.next()) {
            loaded.insert(label.to_owned());
            if pid != "-" {
                running.insert(label.to_owned());
            }
        }
    }
    let system = run("/bin/launchctl", &["print", "system"]);
    let services = system.split("services = {").nth(1).unwrap_or("").split('}').next().unwrap_or("");
    for l in services.lines() {
        let mut cols = l.split_whitespace();
        if let (Some(pid), Some(_), Some(label)) = (cols.next(), cols.next(), cols.next()) {
            loaded.insert(label.to_owned());
            if pid.parse::<u32>().is_ok_and(|p| p > 0) {
                running.insert(label.to_owned());
            }
        }
    }
    (loaded, running)
}

/// "Open at Login" apps, via System Events. macOS asks once for permission to control
/// System Events; if that's declined this returns nothing.
fn open_at_login() -> Vec<(String, String)> {
    let script = r#"tell application "System Events" to get {name, path} of every login item"#;
    // The first call shows a permission dialog; don't let an unanswered one stall a scan.
    let out = crate::fsutil::run_with_timeout("/usr/bin/osascript", &["-e", script], std::time::Duration::from_secs(15)).unwrap_or_default();
    // AppleScript lists come back as "a, b, c, /p/a, /p/b, /p/c".
    let parts: Vec<&str> = out.trim().split(", ").filter(|s| !s.is_empty()).collect();
    let half = parts.len() / 2;
    if !parts.len().is_multiple_of(2) || half == 0 {
        return vec![];
    }
    (0..half).map(|i| (parts[i].to_owned(), parts[half + i].to_owned())).collect()
}

/// `open_at_login_apps` also asks System Events for classic "Open at Login" apps, which
/// shows a one-time macOS permission prompt.
pub fn list(open_at_login_apps: bool) -> Vec<LaunchItem> {
    let uid = unsafe { libc::getuid() };
    let disabled: HashSet<String> = disabled_labels(&format!("gui/{uid}")).into_iter().chain(disabled_labels("system")).collect();
    let (loaded, running) = launchd_state();

    let mut seen = HashSet::new();
    let mut items: Vec<LaunchItem> = from_plists(&disabled, &running)
        .into_iter()
        .chain(from_bundles(&loaded, &running))
        .filter(|i| seen.insert(i.id.clone()))
        .collect();

    for (name, path) in if open_at_login_apps { open_at_login() } else { vec![] } {
        if !seen.insert(format!("login:{name}")) {
            continue;
        }
        let path = path.trim_end_matches('/').to_owned();
        items.push(LaunchItem {
            id: format!("login:{name}"),
            developer: None,
            kind: ItemKind::LoginItem,
            controllable: true,
            enabled: true,
            running: false,
            plist: None,
            executable: None,
            broken: !Path::new(&path).exists(),
            is_self: false,
            app_name: Some(name.clone()),
            app_path: Some(path),
            name,
        });
    }

    // Full List builds on this list, so it inherits the marking too.
    for i in &mut items {
        i.is_self = crate::protection::persistence::is_ours(i);
    }
    items.sort_by_key(|i| (i.kind as u8, !i.enabled, i.name.to_lowercase()));
    items
}

/// Login items and services apps register from inside their bundle (SMAppService).
/// Only the ones launchd has loaded are switched on; the rest are just shipped.
fn from_bundles(loaded: &HashSet<String>, running: &HashSet<String>) -> Vec<LaunchItem> {
    let mut out = Vec::new();
    for app in crate::apps::installed().into_iter().filter(|a| !a.path.starts_with("/System")) {
        let lib = app.path.join("Contents/Library");
        let base = |kind, name: String, id: String, executable: Option<String>, plist: Option<String>| LaunchItem {
            running: running.contains(&id),
            id,
            name,
            developer: None,
            kind,
            controllable: false,
            enabled: true,
            plist,
            executable,
            app_path: Some(app.path.to_string_lossy().into_owned()),
            app_name: Some(app.name.clone()),
            broken: false,
            is_self: false,
        };
        for helper in std::fs::read_dir(lib.join("LoginItems")).into_iter().flatten().flatten().map(|e| e.path()) {
            if let Some(h) = crate::apps::read_bundle(&helper).filter(|h| loaded.contains(&h.bundle_id)) {
                out.push(base(ItemKind::LoginItem, h.name, h.bundle_id, Some(helper.to_string_lossy().into_owned()), None));
            }
        }
        for (dir, kind) in [("LaunchAgents", ItemKind::Agent), ("LaunchDaemons", ItemKind::Daemon)] {
            for plist in std::fs::read_dir(lib.join(dir)).into_iter().flatten().flatten().map(|e| e.path()) {
                let Some(d) = plist::Value::from_file(&plist).ok().and_then(|v| v.into_dictionary()) else { continue };
                let Some(label) = d.get("Label").and_then(|v| v.as_string()).filter(|l| loaded.contains(*l)) else { continue };
                // BundleProgram is relative to the app.
                let exe = d
                    .get("BundleProgram")
                    .and_then(|v| v.as_string())
                    .map(|p| app.path.join(p).to_string_lossy().into_owned())
                    .or_else(|| d.get("Program").and_then(|v| v.as_string()).map(str::to_owned));
                let name = exe.as_deref().and_then(|e| Path::new(e).file_name()).map_or(label.to_owned(), |n| n.to_string_lossy().into_owned());
                out.push(base(kind, name, label.to_owned(), exe, Some(plist.to_string_lossy().into_owned())));
            }
        }
    }
    out
}

/// Third-party launchd jobs from their plist folders.
pub fn from_plists(disabled: &HashSet<String>, running: &HashSet<String>) -> Vec<LaunchItem> {
    let home = crate::fsutil::home_dir();
    let dirs = [
        (home.join("Library/LaunchAgents"), ItemKind::Agent),
        (PathBuf::from("/Library/LaunchAgents"), ItemKind::Agent),
        (PathBuf::from("/Library/LaunchDaemons"), ItemKind::Daemon),
    ];
    let mut out = Vec::new();
    for (dir, kind) in dirs {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "plist") {
                continue;
            }
            let dict = plist::Value::from_file(&path).ok().and_then(|v| v.into_dictionary());
            let get = |k: &str| dict.as_ref()?.get(k)?.as_string().map(str::to_owned);
            let label = get("Label").unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
            let executable = get("Program").or_else(|| {
                dict.as_ref()?.get("ProgramArguments")?.as_array()?.first()?.as_string().map(str::to_owned)
            });
            let plist_disabled = dict.as_ref().and_then(|d| d.get("Disabled")).and_then(|v| v.as_boolean()).unwrap_or(false);
            let broken = executable.as_deref().is_some_and(|e| e.starts_with('/') && !Path::new(e).exists());
            let app_path = executable.as_deref().and_then(|e| e.find(".app/").map(|i| e[..i + 4].to_owned()));
            let name = executable
                .as_deref()
                .and_then(|e| Path::new(e).file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| label.clone());
            out.push(LaunchItem {
                enabled: !plist_disabled && !disabled.contains(&label),
                running: running.contains(&label),
                id: label,
                name,
                developer: None,
                kind,
                controllable: true,
                plist: Some(path.to_string_lossy().into_owned()),
                executable,
                app_name: app_path.as_deref().and_then(|a| crate::apps::read_bundle(Path::new(a))).map(|a| a.name),
                app_path,
                broken,
                is_self: false,
            });
        }
    }
    out
}

fn domain_of(item: &LaunchItem) -> String {
    if item.kind == ItemKind::Daemon {
        "system".into()
    } else {
        format!("gui/{}", unsafe { libc::getuid() })
    }
}

fn exec(argv: &[String], admin: bool) -> Result<(), String> {
    if admin {
        return privileged::run_as_admin(&[argv.to_vec()], "PenCleaner needs your password to change a background item that runs for all users.");
    }
    let out = Command::new(&argv[0]).args(&argv[1..]).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// Turns a plist-based item on or off. Reversible: the plist stays where it is.
pub fn set_enabled(item: &LaunchItem, enabled: bool) -> Result<(), String> {
    if item.is_self {
        return Err(SELF_MSG.into());
    }
    let Some(plist) = item.plist.as_deref().filter(|_| item.controllable) else {
        return Err("This item was registered by its app. Switch it off in System Settings → Login Items.".into());
    };
    let domain = domain_of(item);
    let target = format!("{domain}/{}", item.id);
    let admin = item.kind == ItemKind::Daemon;
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    if enabled {
        exec(&s(&["/bin/launchctl", "enable", &target]), admin)?;
        // Already loaded is fine.
        let _ = exec(&s(&["/bin/launchctl", "bootstrap", &domain, plist]), admin);
    } else {
        exec(&s(&["/bin/launchctl", "disable", &target]), admin)?;
        let _ = exec(&s(&["/bin/launchctl", "bootout", &target]), admin);
    }
    Ok(())
}

/// Stops the item and moves its plist (or login entry) away.
pub fn remove(item: &LaunchItem) -> Result<(), String> {
    if item.is_self {
        return Err(SELF_MSG.into());
    }
    if let Some(name) = item.id.strip_prefix("login:") {
        let script = format!(r#"tell application "System Events" to delete login item "{}""#, name.replace('"', "\\\""));
        return exec(&["/usr/bin/osascript".into(), "-e".into(), script], false);
    }
    let Some(plist) = item.plist.as_deref().filter(|_| item.controllable) else {
        return Err("This item was registered by its app. Uninstall the app or use System Settings → Login Items.".into());
    };
    let domain = domain_of(item);
    let path = PathBuf::from(plist);
    if path.starts_with("/Library") {
        let dest = crate::fsutil::home_dir().join(".Trash").join(path.file_name().unwrap_or_default());
        let cmds = vec![
            vec!["/bin/launchctl".into(), "bootout".into(), format!("{domain}/{}", item.id)],
            vec!["/bin/mv".into(), "-n".into(), plist.to_owned(), dest.to_string_lossy().into_owned()],
        ];
        privileged::run_as_admin(&cmds, "PenCleaner needs your password to remove a background item installed for all users.")?;
        return if path.exists() { Err("The item is still there.".into()) } else { Ok(()) };
    }
    let _ = exec(&["/bin/launchctl".into(), "bootout".into(), format!("{domain}/{}", item.id)], false);
    Remover::default().remove(&path, RemoveMode::Trash)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMP: &str = r#"
========================
 Records for UID 501 : 95613FE1
========================

 Items:

 #1:
                 UUID: D354E8EC
                 Name: Movavi Screen Recorder
                 Type: app (0x2)
          Disposition: [disabled, allowed, notified] (0xa)
           Identifier: 2.com.movavi.screenrecorder
                  URL: file:///Applications/Movavi%20Screen%20Recorder.app/

 #2:
                 Name: Movavi Helper
       Developer Name: Movavi Software
                 Type: login item (0x4)
          Disposition: [disabled, allowed, notified] (0xa)
           Identifier: 4.com.movavi.helper
                  URL: Contents/Library/LoginItems/Helper.app
    Parent Identifier: 2.com.movavi.screenrecorder

 #3:
                 Name: kafka-server-start
       Developer Name: (null)
                 Type: legacy agent (0x10008)
          Disposition: [enabled, allowed, notified] (0xb)
           Identifier: 8.homebrew.mxcl.kafka
                  URL: file:///Users/me/Library/LaunchAgents/homebrew.mxcl.kafka.plist
      Executable Path: /usr/local/opt/kafka/bin/kafka-server-start
  Embedded Item Identifiers:
    #1: com.example.nested
"#;

    #[test]
    fn parses_btm_and_merges_into_the_scan_list() {
        let btm = from_btm(DUMP, &HashSet::new());
        assert_eq!(btm.len(), 2);
        let helper = btm.iter().find(|i| i.id == "com.movavi.helper").unwrap();
        assert_eq!(helper.kind, ItemKind::LoginItem);
        assert!(!helper.enabled);
        assert_eq!(helper.developer.as_deref(), Some("Movavi Software"));
        assert_eq!(helper.app_path.as_deref(), Some("/Applications/Movavi Screen Recorder.app"));
        assert_eq!(helper.executable, None);

        // The scan already had kafka (from its plist) but not the switched-off helper.
        let scanned = vec![LaunchItem {
            id: "homebrew.mxcl.kafka".into(),
            name: "kafka-server-start".into(),
            developer: None,
            kind: ItemKind::Agent,
            controllable: true,
            enabled: false,
            running: false,
            plist: Some("/Users/me/Library/LaunchAgents/homebrew.mxcl.kafka.plist".into()),
            executable: None,
            app_path: None,
            app_name: None,
            broken: false,
            is_self: false,
        }];
        let merged = merge(scanned, btm);
        assert_eq!(merged.len(), 2, "kafka merged, helper added");
        let kafka = merged.iter().find(|i| i.id == "homebrew.mxcl.kafka").unwrap();
        assert!(!kafka.enabled, "launchd's disabled state wins for plist items");
    }

    #[test]
    fn lists_items_without_asking_for_a_password() {
        // Must return quickly and without sfltool (which prompts for a password).
        let t = std::time::Instant::now();
        let items = list(false);
        assert!(t.elapsed().as_secs() < 15, "took {:?}", t.elapsed());
        let mut ids: Vec<_> = items.iter().map(|i| &i.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids are unique");
    }
}
