//! Privacy: browsing traces and recent-items lists. Never saved passwords, never
//! bookmarks. Data is deleted (not trashed): a history file in the Trash is still history.

use crate::apps;
use crate::fsutil::{self, home_dir};
use crate::remove::{RemoveMode, Remover};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Trace {
    History,
    Cookies,
    Autofill,
    RecentItems,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyItem {
    pub id: String,
    /// "Google Chrome", or "macOS" for recent-items lists.
    pub owner: String,
    pub app_path: Option<String>,
    pub trace: Trace,
    pub title: String,
    pub detail: String,
    pub size: u64,
    pub recommended: bool,
    pub warning: Option<String>,
    /// The owning app is open; it must be quit first.
    pub running: bool,
    #[serde(skip)]
    pub paths: Vec<PathBuf>,
}

struct Browser {
    name: &'static str,
    bundle_id: &'static str,
    app: &'static str,
    kind: Engine,
    /// Under ~/Library/Application Support.
    data: &'static str,
}

#[derive(PartialEq)]
enum Engine {
    Chromium,
    Gecko,
}

const BROWSERS: &[Browser] = &[
    Browser { name: "Google Chrome", bundle_id: "com.google.Chrome", app: "/Applications/Google Chrome.app", kind: Engine::Chromium, data: "Google/Chrome" },
    Browser { name: "Brave", bundle_id: "com.brave.Browser", app: "/Applications/Brave Browser.app", kind: Engine::Chromium, data: "BraveSoftware/Brave-Browser" },
    Browser { name: "Microsoft Edge", bundle_id: "com.microsoft.edgemac", app: "/Applications/Microsoft Edge.app", kind: Engine::Chromium, data: "Microsoft Edge" },
    Browser { name: "Arc", bundle_id: "company.thebrowser.Browser", app: "/Applications/Arc.app", kind: Engine::Chromium, data: "Arc/User Data" },
    Browser { name: "Vivaldi", bundle_id: "com.vivaldi.Vivaldi", app: "/Applications/Vivaldi.app", kind: Engine::Chromium, data: "Vivaldi" },
    Browser { name: "Opera", bundle_id: "com.operasoftware.Opera", app: "/Applications/Opera.app", kind: Engine::Chromium, data: "com.operasoftware.Opera" },
    Browser { name: "Firefox", bundle_id: "org.mozilla.firefox", app: "/Applications/Firefox.app", kind: Engine::Gecko, data: "Firefox/Profiles" },
    Browser { name: "Zen", bundle_id: "app.zen-browser.zen", app: "/Applications/Zen.app", kind: Engine::Gecko, data: "zen/Profiles" },
];

const CHROMIUM_HISTORY: &[&str] = &[
    "History", "History-journal", "Visited Links", "Top Sites", "Top Sites-journal", "Shortcuts", "Shortcuts-journal",
    "Network Action Predictor", "Network Action Predictor-journal", "Favicons", "Favicons-journal", "Sessions",
];
const CHROMIUM_COOKIES: &[&str] = &[
    "Cookies", "Cookies-journal", "Network/Cookies", "Network/Cookies-journal", "Local Storage", "Session Storage", "IndexedDB",
    "Service Worker",
];
const CHROMIUM_AUTOFILL: &[&str] = &["Web Data", "Web Data-journal"];
const GECKO_COOKIES: &[&str] = &["cookies.sqlite", "cookies.sqlite-wal", "webappsstore.sqlite", "storage"];
const GECKO_AUTOFILL: &[&str] = &["formhistory.sqlite"];

fn existing(base: &Path, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|n| base.join(n)).filter(|p| fs::symlink_metadata(p).is_ok()).collect()
}

fn profiles(root: &Path, engine: &Engine) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(root) else { return vec![] };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| match engine {
            Engine::Chromium => {
                let n = p.file_name().unwrap_or_default().to_string_lossy();
                n == "Default" || n.starts_with("Profile ") || n == "Guest Profile"
            }
            Engine::Gecko => p.join("prefs.js").exists(),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn item(
    owner: &str,
    app: Option<&str>,
    trace: Trace,
    title: &str,
    detail: String,
    paths: Vec<PathBuf>,
    recommended: bool,
    warning: Option<&str>,
    running: bool,
) -> Option<PrivacyItem> {
    if paths.is_empty() {
        return None;
    }
    let size = paths.iter().map(|p| fsutil::disk_usage(p)).sum();
    let slug = |s: &str| s.to_ascii_lowercase().replace(' ', "-");
    Some(PrivacyItem {
        id: format!("{}:{}", slug(owner), slug(title)),
        owner: owner.into(),
        app_path: app.filter(|a| Path::new(a).exists()).map(str::to_owned),
        trace,
        title: title.into(),
        detail,
        size,
        recommended,
        warning: warning.map(str::to_owned),
        running,
        paths,
    })
}

fn profiles_label(n: usize) -> String {
    if n == 1 { "1 profile".into() } else { format!("{n} profiles") }
}

pub fn scan() -> Vec<PrivacyItem> {
    let home = home_dir();
    let support = home.join("Library/Application Support");
    let running = apps::running_bundle_ids();
    let mut out = Vec::new();

    // Safari (needs Full Disk Access to read its folder).
    let safari = home.join("Library/Safari");
    let safari_on = running.contains("com.apple.safari");
    let container = home.join("Library/Containers/com.apple.Safari/Data/Library");
    out.extend(item(
        "Safari",
        Some("/Applications/Safari.app"),
        Trace::History,
        "Browsing history",
        "Visited sites, recently closed tabs and the downloads list".into(),
        existing(&safari, &["History.db", "History.db-wal", "History.db-shm", "History.db-lock", "RecentlyClosedTabs.plist", "Downloads.plist", "LastSession.plist"]),
        true,
        Some("With iCloud Safari sync on, history from your other devices may come back."),
        safari_on,
    ));
    let mut safari_cookies = existing(&container, &["Cookies/Cookies.binarycookies", "WebKit/WebsiteData"]);
    safari_cookies.extend(existing(&safari, &["LocalStorage", "Databases"]));
    out.extend(item(
        "Safari",
        Some("/Applications/Safari.app"),
        Trace::Cookies,
        "Cookies & website data",
        "Signs you out of websites".into(),
        safari_cookies,
        false,
        Some("You'll be signed out of every website in Safari."),
        safari_on,
    ));

    for b in BROWSERS {
        let root = support.join(b.data);
        let profs = profiles(&root, &b.kind);
        if profs.is_empty() {
            continue;
        }
        let on = running.contains(&b.bundle_id.to_ascii_lowercase());
        let collect = |names: &[&str]| profs.iter().flat_map(|p| existing(p, names)).collect::<Vec<_>>();
        let n = profiles_label(profs.len());
        if b.kind == Engine::Chromium {
            out.extend(item(b.name, Some(b.app), Trace::History, "Browsing history", format!("History, top sites and the downloads list · {n}"), collect(CHROMIUM_HISTORY), true, None, on));
        }
        let cookies = collect(if b.kind == Engine::Chromium { CHROMIUM_COOKIES } else { GECKO_COOKIES });
        out.extend(item(b.name, Some(b.app), Trace::Cookies, "Cookies & website data", format!("Signs you out of websites · {n}"), cookies, false, Some("You'll be signed out of every website in this browser."), on));
        let autofill = collect(if b.kind == Engine::Chromium { CHROMIUM_AUTOFILL } else { GECKO_AUTOFILL });
        let autofill_warning = if b.kind == Engine::Chromium {
            "Also removes saved addresses, payment cards and custom search engines. Saved passwords are never touched."
        } else {
            "Removes saved form entries. Saved passwords are never touched."
        };
        out.extend(item(b.name, Some(b.app), Trace::Autofill, "Autofill data", format!("Form entries · {n}"), autofill, false, Some(autofill_warning), on));
    }

    let lists = home.join("Library/Application Support/com.apple.sharedfilelist");
    let mut recents = existing(
        &lists,
        &[
            "com.apple.LSSharedFileList.RecentDocuments.sfl4",
            "com.apple.LSSharedFileList.RecentApplications.sfl4",
            "com.apple.LSSharedFileList.RecentServers.sfl4",
            "com.apple.LSSharedFileList.RecentHosts.sfl4",
            "com.apple.LSSharedFileList.RecentDocuments.sfl3",
            "com.apple.LSSharedFileList.RecentApplications.sfl3",
            "com.apple.LSSharedFileList.RecentServers.sfl3",
        ],
    );
    // Per-app "Open Recent" menus.
    recents.extend(fs::read_dir(lists.join("com.apple.LSSharedFileList.ApplicationRecentDocuments")).into_iter().flatten().flatten().map(|e| e.path()));
    out.extend(item("macOS", None, Trace::RecentItems, "Recent items", "The Recent Items menu and every app's Open Recent list".into(), recents, true, None, false));

    out
}

pub struct CleanResult {
    pub cleaned: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub bytes: u64,
}

/// Deletes the chosen items. Refuses an item whose browser is open: it would rewrite
/// (or corrupt) the files as they go.
pub fn clean(items: &[PrivacyItem], ids: &HashSet<String>) -> CleanResult {
    let running = apps::running_bundle_ids();
    let remover = Remover::default();
    let mut r = CleanResult { cleaned: vec![], failed: vec![], bytes: 0 };
    let mut recents_changed = false;
    for it in items.iter().filter(|i| ids.contains(&i.id)) {
        let bundle = BROWSERS.iter().find(|b| b.name == it.owner).map(|b| b.bundle_id).or((it.owner == "Safari").then_some("com.apple.Safari"));
        if bundle.is_some_and(|b| running.contains(&b.to_ascii_lowercase())) {
            r.failed.push((it.id.clone(), format!("Quit {} first.", it.owner)));
            continue;
        }
        let errors: Vec<String> = it.paths.iter().filter_map(|p| remover.remove(p, RemoveMode::Delete).err()).collect();
        if errors.is_empty() {
            r.bytes += it.size;
            r.cleaned.push(it.id.clone());
            recents_changed |= it.trace == Trace::RecentItems;
        } else {
            r.failed.push((it.id.clone(), errors.join("; ")));
        }
    }
    if recents_changed {
        // The daemon caches the lists; restarting it makes the change show at once.
        let _ = Command::new("/usr/bin/killall").arg("sharedfilelistd").status();
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_chromium_profiles_and_their_files() {
        let dir = tempfile::tempdir().unwrap();
        for p in ["Default", "Profile 3", "System Profile"] {
            fs::create_dir_all(dir.path().join(p)).unwrap();
        }
        fs::write(dir.path().join("Default/History"), b"x").unwrap();
        fs::write(dir.path().join("Profile 3/Login Data"), b"secret").unwrap();
        let profs = profiles(dir.path(), &Engine::Chromium);
        assert_eq!(profs.len(), 2);
        let history: Vec<_> = profs.iter().flat_map(|p| existing(p, CHROMIUM_HISTORY)).collect();
        assert_eq!(history.len(), 1);
        for list in [CHROMIUM_HISTORY, CHROMIUM_COOKIES, CHROMIUM_AUTOFILL] {
            assert!(!list.iter().any(|n| n.starts_with("Login Data")), "passwords are never offered");
        }
    }

    #[test]
    fn deletes_only_chosen_items() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("History");
        fs::write(&f, b"x").unwrap();
        let mk = |owner: &str, id: &str, paths: Vec<PathBuf>| PrivacyItem {
            id: id.into(),
            owner: owner.into(),
            app_path: None,
            trace: Trace::History,
            title: "t".into(),
            detail: String::new(),
            size: 1,
            recommended: true,
            warning: None,
            running: false,
            paths,
        };
        let g = dir.path().join("Cookies");
        fs::write(&g, b"y").unwrap();
        let items = vec![mk("Imaginary Browser", "a", vec![f.clone()]), mk("Imaginary Browser", "b", vec![g.clone()])];
        let r = clean(&items, &HashSet::from(["a".to_string()]));
        assert_eq!(r.cleaned, vec!["a"]);
        assert!(!f.exists() && g.exists());
    }
}
