//! Login and background items, from the same Background Task Management database that
//! System Settings → General → Login Items shows (`sfltool dumpbtm`, readable without
//! root), plus launchd's own state for what is running or disabled.

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

/// Labels with a live process, in the user and system domains.
fn running_labels() -> HashSet<String> {
    let mut out: HashSet<String> = run("/bin/launchctl", &["list"])
        .lines()
        .skip(1)
        .filter_map(|l| {
            let mut cols = l.split_whitespace();
            let pid = cols.next()?;
            let label = cols.nth(1)?;
            (pid != "-").then(|| label.to_owned())
        })
        .collect();
    let system = run("/bin/launchctl", &["print", "system"]);
    let services = system.split("services = {").nth(1).unwrap_or("").split('}').next().unwrap_or("");
    for l in services.lines() {
        let mut cols = l.split_whitespace();
        if let (Some(pid), Some(_), Some(label)) = (cols.next(), cols.next(), cols.next()) {
            if pid.parse::<u32>().is_ok_and(|p| p > 0) {
                out.insert(label.to_owned());
            }
        }
    }
    out
}

/// "Open at Login" apps, via System Events. macOS asks once for permission to control
/// System Events; if that's declined this returns nothing.
fn open_at_login() -> Vec<(String, String)> {
    let script = r#"tell application "System Events" to get {name, path} of every login item"#;
    let out = run("/usr/bin/osascript", &["-e", script]);
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
    let uid = unsafe { libc::getuid() } as i64;
    let dump = run("/usr/bin/sfltool", &["dumpbtm"]);
    let recs = parse(&dump, &[uid, -2, 0]);

    // Parent apps by identifier, to name items and resolve their relative paths.
    let apps: HashMap<&str, (&str, String)> = recs
        .iter()
        .filter(|r| r.get("Type").is_some_and(|t| t.starts_with("app")))
        .filter_map(|r| Some((r.get("Identifier")?, (r.get("Name").unwrap_or(""), decode_url(r.get("URL")?)))))
        .collect();
    let disabled: HashSet<String> = disabled_labels(&format!("gui/{uid}")).into_iter().chain(disabled_labels("system")).collect();
    let running = running_labels();

    let mut seen = HashSet::new();
    let mut items: Vec<LaunchItem> = recs
        .iter()
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
            let identifier = r.get("Identifier")?;
            // The UI keys rows by label, so one label means one row.
            if !seen.insert(label_of(identifier).to_owned()) {
                return None;
            }
            let legacy = ty.starts_with("legacy");
            let parent = r.get("Parent Identifier").and_then(|p| apps.get(p));
            let resolve = |rel: &str| -> String {
                let p = decode_url(rel);
                match parent {
                    Some((_, app)) if !p.starts_with('/') => format!("{app}/{p}"),
                    _ => p,
                }
            };
            let label = label_of(identifier).to_owned();
            let plist = legacy.then(|| r.get("URL").map(resolve)).flatten();
            let executable = r.get("Executable Path").map(resolve);
            let disposition = r.get("Disposition").unwrap_or("");
            let btm_enabled = disposition.contains("[enabled") || disposition.contains(" enabled");
            let broken = plist.as_deref().is_some_and(|p| !Path::new(p).exists())
                || executable.as_deref().is_some_and(|e| e.starts_with('/') && !Path::new(e).exists());
            let app_path = parent.map(|(_, a)| a.clone()).or_else(|| {
                let e = executable.as_deref()?;
                let end = e.find(".app/")? + 4;
                Some(e[..end].to_owned())
            });
            Some(LaunchItem {
                id: label.clone(),
                name: r.get("Name").map(str::to_owned).unwrap_or_else(|| label.clone()),
                developer: r.get("Developer Name").map(str::to_owned),
                kind,
                controllable: legacy && plist.is_some(),
                enabled: btm_enabled && !disabled.contains(&label),
                running: running.contains(&label),
                app_name: parent.map(|(n, _)| (*n).to_owned()).filter(|n| !n.is_empty()),
                app_path,
                plist,
                executable,
                broken,
            })
        })
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
            app_name: Some(name.clone()),
            app_path: Some(path),
            name,
        });
    }

    items.sort_by_key(|i| (i.kind as u8, !i.enabled, i.name.to_lowercase()));
    items
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
                 Name: Postgres
                 Type: app (0x2)
          Disposition: [enabled, allowed, notified] (0xb)
           Identifier: 2.com.postgresapp.Postgres2
                  URL: file:///Applications/Postgres.app/

 #2:
                 Name: PostgresLoginHelper
       Developer Name: Jakob Egger
                 Type: login item (0x4)
          Disposition: [enabled, allowed, notified] (0xb)
           Identifier: 4.com.postgresapp.Postgres2LoginHelper
                  URL: Contents/Library/LoginItems/PostgresLoginHelper.app
    Parent Identifier: 2.com.postgresapp.Postgres2

 #3:
                 Name: kafka-server-start
                 Type: legacy agent (0x10008)
          Disposition: [disabled, allowed, notified] (0xa)
           Identifier: 8.homebrew.mxcl.kafka
                  URL: file:///Users/me/Library/LaunchAgents/homebrew.mxcl.kafka.plist
      Executable Path: /usr/local/opt/kafka/bin/kafka-server-start
  Embedded Item Identifiers:
    #1: com.example.nested
"#;

    #[test]
    fn parses_btm_records() {
        let recs = parse(DUMP, &[501]);
        assert_eq!(recs.len(), 3);
        assert_eq!(recs[1].get("Parent Identifier"), Some("2.com.postgresapp.Postgres2"));
        assert_eq!(recs[2].get("Disposition"), Some("[disabled, allowed, notified] (0xa)"));
        assert!(parse(DUMP, &[0]).is_empty());
        assert_eq!(label_of("8.homebrew.mxcl.kafka"), "homebrew.mxcl.kafka");
        assert_eq!(decode_url("file:///Applications/Blackmagic%20RAW/Player.app/"), "/Applications/Blackmagic RAW/Player.app");
    }
}
