//! Installed and running applications. Cleanup uses this to put names on bundle-id
//! folders and to avoid pulling caches out from under running apps.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fsutil::home_dir;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub bundle_id: String,
    pub path: PathBuf,
    pub version: Option<String>,
}

pub fn read_bundle(app: &Path) -> Option<AppInfo> {
    let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let dict = info.as_dictionary()?;
    let get = |k: &str| dict.get(k).and_then(|v| v.as_string()).map(str::to_owned);
    let bundle_id = get("CFBundleIdentifier")?;
    let name = get("CFBundleDisplayName")
        .or_else(|| get("CFBundleName"))
        // Some apps pad their name with invisible direction marks (U+200E…).
        .map(|n| n.chars().filter(|c| !matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')).collect::<String>())
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| app.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    Some(AppInfo { name, bundle_id, path: app.to_path_buf(), version: get("CFBundleShortVersionString") })
}

/// Apps in the usual locations, one level of subfolders deep (e.g. "Adobe Photoshop 2026/").
pub fn installed() -> Vec<AppInfo> {
    let home = home_dir();
    let roots = [
        PathBuf::from("/Applications"),
        home.join("Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        PathBuf::from("/Applications/Utilities"),
    ];
    let mut out: Vec<AppInfo> = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "app") {
                out.extend(read_bundle(&p));
            } else if e.file_type().is_ok_and(|t| t.is_dir()) {
                if let Ok(inner) = fs::read_dir(&p) {
                    for i in inner.flatten() {
                        let ip = i.path();
                        if ip.extension().is_some_and(|x| x == "app") {
                            out.extend(read_bundle(&ip));
                        }
                    }
                }
            }
        }
    }
    // /Applications/Utilities is both a root and a subfolder of /Applications.
    let mut seen = HashSet::new();
    out.retain(|a| seen.insert(a.path.clone()));
    out
}

/// Bundle ids of running GUI apps and agents, via LaunchServices' `lsappinfo`.
pub fn running_bundle_ids() -> HashSet<String> {
    let Ok(out) = Command::new("/usr/bin/lsappinfo").arg("list").output() else {
        return HashSet::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.split("bundleID=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

/// Bundle ids of every app Spotlight knows about, including helpers nested inside other
/// apps and tools installed outside /Applications.
pub fn spotlight_apps() -> Vec<AppInfo> {
    let Ok(out) = Command::new("/usr/bin/mdfind").arg("kMDItemContentType == com.apple.application-bundle").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| read_bundle(Path::new(l))).collect()
}

/// Labels of installed launchd jobs (file names of their plists), lowercased.
pub fn launchd_labels() -> HashSet<String> {
    let home = home_dir();
    let dirs = [
        PathBuf::from("/Library/LaunchAgents"),
        PathBuf::from("/Library/LaunchDaemons"),
        home.join("Library/LaunchAgents"),
    ];
    dirs.iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flat_map(|rd| rd.flatten())
        .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".plist").map(|s| s.to_ascii_lowercase()))
        .collect()
}

/// Case-insensitive bundle id → app name.
#[derive(Default)]
pub struct AppIndex {
    names: HashMap<String, String>,
}

impl AppIndex {
    pub fn load() -> Self {
        let names = installed().into_iter().map(|a| (a.bundle_id.to_ascii_lowercase(), a.name)).collect();
        Self { names }
    }

    pub fn name(&self, bundle_id: &str) -> Option<&str> {
        self.names.get(&bundle_id.to_ascii_lowercase()).map(String::as_str)
    }

    pub fn is_installed(&self, bundle_id: &str) -> bool {
        self.names.contains_key(&bundle_id.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_system_apps_and_running_finder() {
        let apps = super::installed();
        let mut paths: Vec<_> = apps.iter().map(|a| &a.path).collect();
        let n = paths.len();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), n, "each app listed once");
        assert!(apps.iter().any(|a| a.bundle_id == "com.apple.calculator"));
        assert!(super::running_bundle_ids().contains("com.apple.finder"));
    }
}
