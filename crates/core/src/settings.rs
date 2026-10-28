//! User settings and the ignore list, in one JSON file in PenCleaner's data folder.
//! Missing or unknown fields fall back to defaults, so old files keep working.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IgnoreKind {
    /// A folder or file, and everything inside it, in every module.
    Path,
    /// One result of one module, by its id ("cleanup:project:/…", "updates:/Applications/X.app@2.0").
    Item,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreRule {
    pub kind: IgnoreKind,
    pub value: String,
    /// What the user saw, for the Settings list.
    pub label: String,
    /// Which module it was ignored from (shown in Settings).
    pub module: String,
    pub added: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Cleanup pre-selects project build output untouched this long.
    pub stale_project_days: u32,
    /// My Clutter's "large file" threshold.
    pub large_file_mb: u64,
    /// Applications calls an app unused after this many days.
    pub unused_app_days: u32,
    /// Check for app updates whenever Applications scans (goes online).
    pub check_updates_on_scan: bool,
    /// Ask Apple whether background items are notarized during Protection scans (goes online).
    pub online_notarization: bool,
    /// When an app lands in the Trash, offer to remove what it left behind.
    pub offer_trashed_app_cleanup: bool,
    pub ignore: Vec<IgnoreRule>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            stale_project_days: 30,
            large_file_mb: 50,
            unused_app_days: 180,
            check_updates_on_scan: true,
            online_notarization: true,
            offer_trashed_app_cleanup: true,
            ignore: Vec::new(),
        }
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &Path) -> Settings {
    fs::read(file(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Written to a temporary file and renamed, so a crash can't leave half a settings file.
pub fn save(dir: &Path, s: &Settings) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("settings.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&tmp, file(dir)).map_err(|e| e.to_string())
}

impl Settings {
    /// Adds a rule unless an identical one exists.
    pub fn add_ignore(&mut self, kind: IgnoreKind, value: &str, label: &str, module: &str) {
        let value = if kind == IgnoreKind::Path { value.trim_end_matches('/').to_owned() } else { value.to_owned() };
        if !self.ignore.iter().any(|r| r.kind == kind && r.value == value) {
            let added = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            self.ignore.push(IgnoreRule { kind, value, label: label.into(), module: module.into(), added });
        }
    }

    pub fn remove_ignore(&mut self, kind: IgnoreKind, value: &str) {
        self.ignore.retain(|r| !(r.kind == kind && r.value == value));
    }

    pub fn matcher(&self) -> Ignore {
        Ignore {
            paths: self.ignore.iter().filter(|r| r.kind == IgnoreKind::Path).map(|r| PathBuf::from(&r.value)).collect(),
            items: self.ignore.iter().filter(|r| r.kind == IgnoreKind::Item).map(|r| r.value.clone()).collect(),
        }
    }
}

/// Answers "is this ignored?" for scans. Cheap to clone and pass around.
#[derive(Debug, Clone, Default)]
pub struct Ignore {
    paths: Vec<PathBuf>,
    items: std::collections::HashSet<String>,
}

impl Ignore {
    /// The path is an ignored path or inside one (component-wise: /a/b doesn't cover /a/bc).
    pub fn path(&self, p: &Path) -> bool {
        self.paths.iter().any(|ig| p.starts_with(ig))
    }

    pub fn path_str(&self, p: &str) -> bool {
        self.path(Path::new(p))
    }

    /// `key` is "<module>:<item id>".
    pub fn item(&self, key: &str) -> bool {
        self.items.contains(key)
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.items.is_empty()
    }

    /// Folders scans should not walk into at all.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_old_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Settings::default(), "no file → defaults");
        fs::write(file(dir.path()), br#"{"largeFileMb": 100, "someFutureField": 1}"#).unwrap();
        let s = load(dir.path());
        assert_eq!(s.large_file_mb, 100);
        assert_eq!(s.stale_project_days, 30, "missing fields default");

        let mut s = Settings::default();
        s.add_ignore(IgnoreKind::Path, "/Users/me/Sites/keep/", "keep", "cleanup");
        s.add_ignore(IgnoreKind::Path, "/Users/me/Sites/keep", "keep", "cleanup");
        s.add_ignore(IgnoreKind::Item, "updates:/Applications/X.app@2.0", "X 2.0", "applications");
        assert_eq!(s.ignore.len(), 2, "no duplicates; trailing slash normalised");
        save(dir.path(), &s).unwrap();
        assert_eq!(load(dir.path()), s);
    }

    #[test]
    fn matches_paths_by_component_and_items_exactly() {
        let mut s = Settings::default();
        s.add_ignore(IgnoreKind::Path, "/Users/me/Sites/keep", "keep", "cleanup");
        s.add_ignore(IgnoreKind::Item, "cleanup:sim:ABC", "iPhone", "cleanup");
        let ig = s.matcher();
        assert!(ig.path(Path::new("/Users/me/Sites/keep")));
        assert!(ig.path(Path::new("/Users/me/Sites/keep/node_modules")));
        assert!(!ig.path(Path::new("/Users/me/Sites/keeper")));
        assert!(ig.item("cleanup:sim:ABC"));
        assert!(!ig.item("cleanup:sim:ABD"));
        s.remove_ignore(IgnoreKind::Item, "cleanup:sim:ABC");
        assert!(!s.matcher().item("cleanup:sim:ABC"));
    }
}
