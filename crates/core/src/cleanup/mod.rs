//! Cleanup: rule sets that find regenerable or forgotten data, grouped for review.
//!
//! Each rule returns `Group`s of `Item`s. An item is what the user ticks; it can cover
//! several paths (a project's node_modules + target) or a command (deleting a simulator).
//! The UI only ever sends item ids back, so nothing outside a scan report can be removed.

mod ai;
pub(crate) mod dev;
mod mail;
mod system;
mod trash_bins;

use crate::apps::{self, AppIndex};
use crate::fsutil::{self, home_dir};
use crate::permissions;
use crate::progress::ScanControl;
use crate::remove::{RemoveMode, Remover};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Items smaller than this are noise in a review list.
const MIN_ITEM: u64 = 64 * 1024;
pub const DAY: i64 = 86_400;
/// Projects untouched for longer than this are pre-selected.
pub const STALE_PROJECT_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    System,
    Developer,
    Ai,
    Mail,
    Trash,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Folder,
    File,
    App,
    Project,
    Model,
    Device,
    Trash,
}

/// How an item goes away.
#[derive(Debug, Clone)]
pub enum Action {
    Paths(Vec<PathBuf>),
    /// Run a tool that owns the data, e.g. `xcrun simctl delete <udid>`.
    Command(Vec<String>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub name: String,
    /// Secondary line: where it lives, what it contains.
    pub detail: Option<String>,
    /// Folder to reveal in Finder.
    pub path: Option<String>,
    pub size: u64,
    pub kind: ItemKind,
    pub mode: RemoveMode,
    /// Unix seconds of the last sign of use, when it means something.
    pub last_activity: Option<i64>,
    /// Pre-ticked in the review list.
    pub recommended: bool,
    pub warning: Option<String>,
    #[serde(skip)]
    pub action: Action,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: &'static str,
    pub category: Category,
    pub title: &'static str,
    pub description: &'static str,
    pub items: Vec<Item>,
    pub size: u64,
    /// Shown when the group couldn't be fully scanned.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub groups: Vec<Group>,
    pub elapsed_ms: u64,
    pub full_disk_access: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum CleanEvent {
    Started { id: String, name: String, size: u64, index: usize, total: usize },
    Cleaned { id: String, bytes: u64 },
    Failed { id: String, error: String, bytes: u64 },
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanSummary {
    pub cleaned: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub bytes_freed: u64,
    pub by_category: HashMap<String, u64>,
}

/// Shared context for rules.
pub struct Cx<'a> {
    pub home: PathBuf,
    pub ctl: &'a ScanControl,
    pub apps: AppIndex,
    pub running: HashSet<String>,
    pub now: i64,
}

impl Cx<'_> {
    pub fn p(&self, rel: &str) -> PathBuf {
        self.home.join(rel)
    }

    /// Measures `path`, reporting progress. None for missing or tiny paths.
    pub fn measure(&self, path: &Path) -> Option<u64> {
        if self.ctl.is_cancelled() || fs::symlink_metadata(path).is_err() {
            return None;
        }
        self.ctl.set_current(path);
        let size = fsutil::disk_usage(path);
        self.ctl.add(1, size);
        (size >= MIN_ITEM).then_some(size)
    }

    pub fn is_running(&self, bundle_id: &str) -> bool {
        self.running.contains(&bundle_id.to_ascii_lowercase())
    }

    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rel) => format!("~/{}", rel.display()),
            Err(_) => path.display().to_string(),
        }
    }
}

/// Builder for the common case: one path, measured.
pub struct ItemSpec<'p> {
    pub path: &'p Path,
    pub name: String,
    pub kind: ItemKind,
    pub mode: RemoveMode,
    pub recommended: bool,
}

impl ItemSpec<'_> {
    pub fn build(self, cx: &Cx) -> Option<Item> {
        let size = cx.measure(self.path)?;
        Some(Item {
            id: self.path.to_string_lossy().into_owned(),
            detail: Some(cx.tilde(self.path)),
            path: Some(self.path.to_string_lossy().into_owned()),
            last_activity: mtime_of(self.path),
            name: self.name,
            size,
            kind: self.kind,
            mode: self.mode,
            recommended: self.recommended,
            warning: None,
            action: Action::Paths(vec![self.path.to_path_buf()]),
        })
    }
}

pub fn mtime(md: &Metadata) -> Option<i64> {
    md.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

pub fn mtime_of(path: &Path) -> Option<i64> {
    fs::symlink_metadata(path).ok().as_ref().and_then(mtime)
}

/// Entries of a directory (empty if unreadable), sorted by name for stable output.
pub fn children(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| !is_ds_store(p)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn is_ds_store(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n == ".DS_Store")
}

pub fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// True when the folder exists but this process can't list it (no Full Disk Access).
pub fn unreadable(dir: &Path) -> bool {
    dir.exists() && fs::read_dir(dir).is_err()
}

pub fn group(id: &'static str, category: Category, title: &'static str, description: &'static str, items: Vec<Item>) -> Group {
    Group { id, category, title, description, items, size: 0, note: None }
}

type Rule = fn(&Cx) -> Vec<Group>;

fn rules(category: Category) -> &'static [Rule] {
    match category {
        Category::System => &[system::user_caches, system::logs, system::temporary, system::ios],
        Category::Developer => &[dev::projects, dev::xcode, dev::simulators, dev::package_caches, dev::ide_caches, dev::virtual_machines],
        Category::Ai => &[ai::models, ai::tool_caches],
        Category::Mail => &[mail::attachments],
        Category::Trash => &[trash_bins::trash_bins],
    }
}

/// Groups scheduled Smart Care may clean on its own: junk that rebuilds itself. The Trash,
/// projects, simulators, VMs, iOS backups, Mail and AI models always wait for a person.
pub const AUTO_GROUPS: [&str; 7] = ["user-caches", "user-logs", "temporary", "package-caches", "ide-caches", "xcode", "ai-caches"];

pub const ALL: [Category; 5] = [Category::System, Category::Developer, Category::Ai, Category::Mail, Category::Trash];

pub fn scan(categories: &[Category], ctl: &ScanControl) -> Report {
    let started = Instant::now();
    fsutil::set_observer_io_policy();
    let cx = Cx {
        home: home_dir(),
        ctl,
        apps: AppIndex::load(),
        running: apps::running_bundle_ids(),
        now: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
    };
    let rules: Vec<Rule> = categories.iter().flat_map(|c| rules(*c).iter().copied()).collect();
    let mut groups: Vec<Group> = rules.par_iter().map(|r| r(&cx)).collect::<Vec<_>>().into_iter().flatten().collect();

    for g in &mut groups {
        g.items.sort_unstable_by_key(|i| std::cmp::Reverse(i.size));
        g.size = g.items.iter().map(|i| i.size).sum();
    }
    groups.retain(|g| !g.items.is_empty() || g.note.is_some());

    Report {
        groups,
        elapsed_ms: started.elapsed().as_millis() as u64,
        full_disk_access: permissions::has_full_disk_access(),
        cancelled: ctl.is_cancelled(),
    }
}

impl Report {
    /// What an unattended clean may remove: recommended, warning-free items of the
    /// self-rebuilding groups.
    pub fn auto_ids(&self) -> HashSet<String> {
        self.groups
            .iter()
            .filter(|g| AUTO_GROUPS.contains(&g.id))
            .flat_map(|g| g.items.iter().filter(|i| i.recommended && i.warning.is_none()).map(|i| i.id.clone()))
            .collect()
    }

    /// Drops ignored items, and ignored paths from inside multi-path items.
    pub fn apply_ignore(&mut self, ig: &crate::settings::Ignore) {
        if ig.is_empty() {
            return;
        }
        for g in &mut self.groups {
            g.items.retain_mut(|i| {
                if ig.item(&format!("cleanup:{}", i.id)) || i.path.as_deref().is_some_and(|p| ig.path_str(p)) {
                    return false;
                }
                if let Action::Paths(paths) = &mut i.action {
                    let before = paths.len();
                    paths.retain(|p| !ig.path(p));
                    if paths.is_empty() {
                        return false;
                    }
                    if paths.len() != before {
                        i.size = paths.iter().map(|p| fsutil::disk_usage(p)).sum();
                    }
                }
                true
            });
            g.size = g.items.iter().map(|i| i.size).sum();
        }
        self.groups.retain(|g| !g.items.is_empty() || g.note.is_some());
    }

    /// Removes the chosen items and drops them from the report.
    pub fn clean(&mut self, ids: &HashSet<String>, mut on_event: impl FnMut(CleanEvent)) -> CleanSummary {
        let remover = Remover::default();
        let chosen: Vec<(Category, Item)> = self
            .groups
            .iter()
            .flat_map(|g| g.items.iter().filter(|i| ids.contains(&i.id)).map(move |i| (g.category, i.clone())))
            .collect();

        let mut summary = CleanSummary::default();
        let total = chosen.len();
        for (index, (category, item)) in chosen.into_iter().enumerate() {
            on_event(CleanEvent::Started { id: item.id.clone(), name: item.name.clone(), size: item.size, index, total });
            let result = match &item.action {
                Action::Paths(paths) => {
                    let errors: Vec<String> = paths.iter().filter_map(|p| remover.remove(p, item.mode).err()).collect();
                    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
                }
                Action::Command(argv) => run(argv),
            };
            match result {
                Ok(()) => {
                    summary.bytes_freed += item.size;
                    *summary.by_category.entry(format!("{category:?}").to_lowercase()).or_default() += item.size;
                    summary.cleaned.push(item.id.clone());
                    on_event(CleanEvent::Cleaned { id: item.id, bytes: item.size });
                }
                Err(error) => {
                    summary.failed.push((item.id.clone(), error.clone()));
                    on_event(CleanEvent::Failed { id: item.id, error, bytes: 0 });
                }
            }
        }

        let cleaned: HashSet<&String> = summary.cleaned.iter().collect();
        for g in &mut self.groups {
            g.items.retain(|i| !cleaned.contains(&i.id));
            g.size = g.items.iter().map(|i| i.size).sum();
        }
        summary
    }
}

fn run(argv: &[String]) -> Result<(), String> {
    let (cmd, args) = argv.split_first().ok_or("empty command")?;
    let out = Command::new(cmd).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, paths: Vec<PathBuf>, size: u64) -> Item {
        Item {
            id: id.into(),
            name: id.into(),
            detail: None,
            path: None,
            size,
            kind: ItemKind::Folder,
            mode: RemoveMode::Delete,
            last_activity: None,
            recommended: true,
            warning: None,
            action: Action::Paths(paths),
        }
    }

    #[test]
    fn auto_clean_takes_only_safe_recommended_junk() {
        let mut warned = item("cache-warned", vec![], 1);
        warned.warning = Some("in use".into());
        let mut unticked = item("cache-unticked", vec![], 1);
        unticked.recommended = false;
        let report = Report {
            groups: vec![
                group("user-caches", Category::System, "Caches", "", vec![item("cache-ok", vec![], 1), warned, unticked]),
                group("trash", Category::Trash, "Trash", "", vec![item("trash", vec![], 1)]),
                group("projects", Category::Developer, "Projects", "", vec![item("node_modules", vec![], 1)]),
            ],
            elapsed_ms: 0,
            full_disk_access: true,
            cancelled: false,
        };
        assert_eq!(report.auto_ids(), HashSet::from(["cache-ok".to_string()]));
    }

    #[test]
    fn ignore_list_drops_items_and_parts_of_items() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("proj/node_modules"), dir.path().join("proj/target"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let mut report = Report {
            groups: vec![group("g", Category::Developer, "G", "", vec![item("project", vec![a.clone(), b.clone()], 10), item("sim", vec![], 5)])],
            elapsed_ms: 0,
            full_disk_access: true,
            cancelled: false,
        };
        let mut s = crate::settings::Settings::default();
        s.add_ignore(crate::settings::IgnoreKind::Path, &b.to_string_lossy(), "target", "cleanup");
        s.add_ignore(crate::settings::IgnoreKind::Item, "cleanup:sim", "sim", "cleanup");
        report.apply_ignore(&s.matcher());
        let items = &report.groups[0].items;
        assert_eq!(items.len(), 1, "ignored id dropped");
        match &items[0].action {
            Action::Paths(p) => assert_eq!(p, &vec![a.clone()], "ignored folder taken out of the item"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn cleans_only_chosen_items_and_drops_them_from_the_report() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::create_dir_all(a.join("x")).unwrap();
        fs::create_dir_all(&b).unwrap();
        let mut report = Report {
            groups: vec![group("g", Category::System, "G", "", vec![item("a", vec![a.clone()], 10), item("b", vec![b.clone()], 20)])],
            elapsed_ms: 0,
            full_disk_access: true,
            cancelled: false,
        };
        let mut events = Vec::new();
        let summary = report.clean(&HashSet::from(["a".to_string(), "unknown".to_string()]), |e| events.push(e));
        assert!(!a.exists() && b.exists());
        assert_eq!(summary.bytes_freed, 10);
        assert_eq!(summary.by_category["system"], 10);
        assert_eq!(events.len(), 2);
        assert_eq!(report.groups[0].items.len(), 1);
        assert_eq!(report.groups[0].size, 20);
    }
}
