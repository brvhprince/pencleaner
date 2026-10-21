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
    let mut out = Vec::new();
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
        assert!(apps.iter().any(|a| a.bundle_id == "com.apple.calculator"));
        assert!(super::running_bundle_ids().contains("com.apple.finder"));
    }
}
