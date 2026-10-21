//! Deciding which ~/Library and /Library entries belong to which app.
//!
//! Apps name their support files after their bundle id (`com.tinyspeck.slackmacgap.plist`,
//! `Containers/com.figma.Desktop`), sometimes after their display name
//! (`Application Support/Slack`), and their launchd helpers after the vendor plus product
//! (`us.zoom.ZoomDaemon` for `us.zoom.xos`). Leftovers are bundle-id entries that nothing
//! installed — app, helper, launchd job or running process — can claim.

use crate::apps::{self, AppInfo};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct LibEntry {
    pub path: PathBuf,
    pub label: &'static str,
    /// Lowercased name with decorations removed: usually a bundle id.
    pub key: String,
}

const USER_DIRS: &[(&str, &str)] = &[
    ("Library/Application Support", "Application Support"),
    ("Library/Caches", "Caches"),
    ("Library/Preferences", "Preferences"),
    ("Library/Preferences/ByHost", "Preferences"),
    ("Library/Containers", "Container"),
    ("Library/Group Containers", "Group Container"),
    ("Library/Saved Application State", "Saved State"),
    ("Library/Logs", "Logs"),
    ("Library/HTTPStorages", "HTTP Storage"),
    ("Library/WebKit", "WebKit Data"),
    ("Library/Cookies", "Cookies"),
    ("Library/LaunchAgents", "Launch Agent"),
    ("Library/Application Scripts", "App Scripts"),
];

const SYSTEM_DIRS: &[(&str, &str)] = &[
    ("/Library/LaunchAgents", "Launch Agent"),
    ("/Library/LaunchDaemons", "Launch Daemon"),
    ("/Library/PrivilegedHelperTools", "Helper Tool"),
    ("/Library/Application Support", "Application Support"),
    ("/Library/Caches", "Caches"),
    ("/Library/Preferences", "Preferences"),
    ("/Library/Logs", "Logs"),
];

/// Words too common to tie a file to an app.
const GENERIC: &[&str] = &[
    "app", "apps", "desktop", "helper", "helpers", "mac", "macos", "osx", "the", "for", "pro", "free", "lite", "plus",
    "beta", "agent", "service", "services", "daemon", "launcher", "update", "updater", "client", "server", "plugin",
    "tool", "tools", "login", "check", "wake", "system", "main", "base", "core", "shared", "group", "groups",
];

fn is_uuidish(s: &str) -> bool {
    (s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')) || (s.len() == 12 && s.chars().all(|c| c.is_ascii_hexdigit()))
}

pub(crate) fn normalize(name: &str) -> String {
    let mut n = name.to_ascii_lowercase();
    for suffix in [".plist", ".savedstate", ".binarycookies"] {
        if let Some(stripped) = n.strip_suffix(suffix) {
            n = stripped.to_owned();
        }
    }
    // ByHost preferences end in a hardware UUID.
    if let Some((head, tail)) = n.rsplit_once('.') {
        if is_uuidish(tail) {
            n = head.to_owned();
        }
    }
    // Group containers: "group.com.x", "TEAMID.com.x", "TEAMID.groups.com.x".
    if let Some((team, rest)) = n.split_once('.') {
        if team.len() == 10 && team.chars().all(|c| c.is_ascii_alphanumeric()) && team.chars().any(|c| c.is_ascii_digit()) {
            n = rest.to_owned();
        }
    }
    for prefix in ["group.", "groups."] {
        if let Some(rest) = n.strip_prefix(prefix) {
            n = rest.to_owned();
        }
    }
    n
}

fn is_bundle_like(key: &str) -> bool {
    let parts: Vec<&str> = key.split('.').collect();
    parts.len() >= 3
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        && (2..=8).contains(&parts[0].len())
        && parts[0].chars().all(|c| c.is_ascii_alphabetic())
}

fn vendor(id: &str) -> String {
    id.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// Distinctive words from an app's name and the product part of its bundle id.
pub(crate) fn tokens(name: &str, bundle_id: &str) -> Vec<String> {
    let lower_id = bundle_id.to_ascii_lowercase();
    let vendor_word = lower_id.split('.').nth(1).unwrap_or("").to_owned();
    let from_name = name.split(|c: char| !c.is_alphanumeric()).map(str::to_ascii_lowercase);
    let from_id = lower_id.split('.').skip(2).flat_map(|p| p.split(['-', '_'])).map(str::to_owned).collect::<Vec<_>>();
    let distinctive = |t: &String| t.len() >= 4 && !GENERIC.contains(&t.as_str()) && !t.chars().all(|c| c.is_ascii_digit());
    let all: Vec<String> = from_name.chain(from_id).filter(distinctive).collect();
    // The vendor's name ("microsoft") would tie every product of theirs together, so it
    // only counts when it is also the product's name (us.zoom.xos, com.docker.docker).
    let mut out: Vec<String> = all.iter().filter(|t| **t != vendor_word).cloned().collect();
    if out.is_empty() {
        out = all;
    }
    out.sort();
    out.dedup();
    out
}

pub fn index_library(home: &Path) -> Vec<LibEntry> {
    let dirs = USER_DIRS
        .iter()
        .map(|(rel, label)| (home.join(rel), *label))
        .chain(SYSTEM_DIRS.iter().map(|(abs, label)| (PathBuf::from(abs), *label)));
    let mut out = Vec::new();
    for (dir, label) in dirs {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "ByHost" {
                continue;
            }
            out.push(LibEntry { key: normalize(&name), path: e.path(), label });
        }
    }
    out
}

/// Does `e` belong to the app with this bundle id and display name?
pub fn belongs_to(e: &LibEntry, bundle_id: &str, name: &str) -> bool {
    let id = bundle_id.to_ascii_lowercase();
    if e.key == id || e.key.starts_with(&format!("{id}.")) {
        return true;
    }
    let lname = name.to_ascii_lowercase();
    if matches!(e.label, "Application Support" | "Caches" | "Logs" | "Preferences") && lname.len() >= 4 && e.key == lname {
        return true;
    }
    if matches!(e.label, "Launch Agent" | "Launch Daemon" | "Helper Tool") && e.key.starts_with(&format!("{}.", vendor(&id))) {
        return tokens(name, bundle_id).iter().any(|t| e.key.contains(t.as_str()));
    }
    false
}

struct Known {
    id: String,
    vendor: String,
    tokens: Vec<String>,
}

/// Everything that counts as installed when looking for orphans.
pub struct Universe(Vec<Known>);

impl Universe {
    pub fn build(running: &HashSet<String>) -> Self {
        let mut known: Vec<Known> = apps::spotlight_apps()
            .into_iter()
            .chain(apps::installed())
            .map(|AppInfo { name, bundle_id, .. }| {
                let id = bundle_id.to_ascii_lowercase();
                Known { vendor: vendor(&id), tokens: tokens(&name, &id), id }
            })
            .collect();
        for id in apps::launchd_labels().into_iter().chain(running.iter().cloned()) {
            known.push(Known { vendor: vendor(&id), tokens: tokens("", &id), id });
        }
        Universe(known)
    }

    #[cfg(test)]
    fn of(list: &[(&str, &str)]) -> Self {
        Universe(
            list.iter()
                .map(|(id, name)| {
                    let id = id.to_ascii_lowercase();
                    Known { vendor: vendor(&id), tokens: tokens(name, &id), id }
                })
                .collect(),
        )
    }

    fn claims(&self, key: &str) -> bool {
        let v = vendor(key);
        self.0.iter().any(|k| {
            key == k.id
                || key.starts_with(&format!("{}.", k.id))
                || k.id.starts_with(&format!("{key}."))
                || (v == k.vendor && k.tokens.iter().any(|t| key.contains(t.as_str())))
        })
    }
}

/// The app id an unclaimed entry was left by, if it looks like an app's at all.
pub fn orphan_id(e: &LibEntry, universe: &Universe) -> Option<String> {
    let key = e.key.as_str();
    if !is_bundle_like(key) || key.starts_with("com.apple.") || key.starts_with("com.pen.cleaner") || universe.claims(key) {
        return None;
    }
    // Group a helper's files with its app: com.figma.desktop.shipit → com.figma.desktop
    Some(key.split('.').take(3).collect::<Vec<_>>().join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, label: &'static str) -> LibEntry {
        LibEntry { path: PathBuf::from(name), label, key: normalize(name) }
    }

    #[test]
    fn normalizes_library_names() {
        assert_eq!(normalize("com.tinyspeck.slackmacgap.plist"), "com.tinyspeck.slackmacgap");
        assert_eq!(normalize("com.adobe.crashreporter.EC0C9769-31A0-5849-AE11-09ADD01D848C.plist"), "com.adobe.crashreporter");
        assert_eq!(normalize("BJ4HAAB9B3.ZoomClient3rd"), "zoomclient3rd");
        assert_eq!(normalize("243LU875E5.groups.com.apple.podcasts"), "com.apple.podcasts");
        assert_eq!(normalize("group.com.docker"), "com.docker");
        assert_eq!(normalize("com.figma.Desktop.savedState"), "com.figma.desktop");
    }

    #[test]
    fn attributes_files_to_their_app() {
        let zoom = ("us.zoom.xos", "zoom.us");
        assert!(belongs_to(&entry("us.zoom.xos.plist", "Preferences"), zoom.0, zoom.1));
        assert!(belongs_to(&entry("us.zoom.ZoomDaemon.plist", "Launch Daemon"), zoom.0, zoom.1));
        let word = ("com.microsoft.Word", "Microsoft Word");
        assert!(!belongs_to(&entry("com.microsoft.autoupdate.helper.plist", "Launch Daemon"), word.0, word.1), "shared updater stays");
        assert!(!belongs_to(&entry("com.microsoft.Excel.plist", "Preferences"), word.0, word.1));
        assert!(belongs_to(&entry("Code", "Application Support"), "com.microsoft.VSCode", "Code"));
        assert!(!belongs_to(&entry("Code", "Containers"), "com.microsoft.VSCode", "Code"));
    }

    #[test]
    fn finds_orphans_but_respects_installed_vendors() {
        let u = Universe::of(&[
            ("com.docker.docker", "Docker"),
            ("com.microsoft.Word", "Microsoft Word"),
            ("com.microsoft.autoupdate.helper", ""),
        ]);
        let orphan = |n: &str| orphan_id(&entry(n, "Preferences"), &u);
        assert_eq!(orphan("com.figma.Desktop.plist").as_deref(), Some("com.figma.desktop"));
        assert_eq!(orphan("com.figma.Desktop.ShipIt.plist").as_deref(), Some("com.figma.desktop"));
        assert_eq!(orphan("com.microsoft.teams2.plist").as_deref(), Some("com.microsoft.teams2"));
        assert_eq!(orphan("com.docker.helper.plist"), None, "Docker is installed");
        assert_eq!(orphan("com.microsoft.autoupdate2.plist"), None, "AutoUpdate's daemon is installed");
        assert_eq!(orphan("com.apple.finder.plist"), None);
        assert_eq!(orphan("Adobe Illustrator 30 Settings"), None, "not a bundle id");
    }
}
