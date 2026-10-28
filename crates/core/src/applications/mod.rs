//! Applications: uninstall apps together with the files they scatter through ~/Library
//! and /Library, find support files left by apps that are already gone, and gather
//! installer images that have done their job.

mod matching;

use crate::apps::{self, AppInfo};
use crate::cleanup::{CleanEvent, CleanSummary};
use crate::fsutil::{self, home_dir};
use crate::privileged;
use crate::progress::ScanControl;
use crate::remove::{RemoveMode, Remover};
use crate::safety;
use crate::spotlight;
use matching::{LibEntry, Universe};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const DAY: i64 = 86_400;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedFile {
    pub path: String,
    /// Where it lives, e.g. "Preferences", "Launch Daemon".
    pub label: &'static str,
    pub size: u64,
    /// Needs the administrator password to remove.
    pub admin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    /// The bundle path: unique even when two copies share a bundle id
    /// (e.g. IDLE.app in two Python versions).
    pub id: String,
    pub bundle_id: String,
    pub name: String,
    pub version: Option<String>,
    pub path: String,
    pub size: u64,
    pub related: Vec<RelatedFile>,
    pub related_size: u64,
    pub last_used: Option<i64>,
    pub app_store: bool,
    pub running: bool,
    /// Another installed copy shares this bundle id, so its support files stay put.
    pub shares_support_files: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub id: String,
    /// The same vendor still has apps installed; these may be shared, so check first.
    pub vendor_installed: bool,
    pub files: Vec<RelatedFile>,
    pub size: u64,
    pub last_activity: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installer {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub added: Option<i64>,
    /// An installed app this image appears to be for.
    pub installed_app: Option<String>,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsReport {
    pub apps: Vec<AppEntry>,
    pub leftovers: Vec<Leftover>,
    pub installers: Vec<Installer>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "id")]
pub enum Target {
    App(String),
    Leftover(String),
    Installer(String),
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Removing this path needs root: we can't write its folder, or it is a folder we
/// can't write (moving a directory rewrites its ".." entry).
fn needs_admin(path: &Path) -> bool {
    let writable = |p: &Path| {
        CString::new(p.as_os_str().as_bytes()).is_ok_and(|c| unsafe { libc::access(c.as_ptr(), libc::W_OK) } == 0)
    };
    let parent_ok = path.parent().is_some_and(writable);
    let is_dir = fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
    !parent_ok || (is_dir && !writable(path))
}

fn related(entries: &[&LibEntry]) -> Vec<RelatedFile> {
    let mut files: Vec<RelatedFile> = entries
        .par_iter()
        .map(|e| RelatedFile {
            path: e.path.to_string_lossy().into_owned(),
            label: e.label,
            size: fsutil::disk_usage(&e.path),
            admin: needs_admin(&e.path),
        })
        .collect();
    files.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));
    files
}

fn user_app(a: &AppInfo) -> bool {
    let p = a.path.to_string_lossy();
    if p.starts_with("/System/") {
        return false;
    }
    // Apple's own apps can't be removed, except the ones that come from the App Store
    // (Pages, Xcode, GarageBand…).
    !a.bundle_id.starts_with("com.apple.") || a.path.join("Contents/_MASReceipt").exists()
}

const INSTALLER_EXTS: &[&str] = &["dmg", "pkg", "mpkg", "xip", "iso"];

fn installers(home: &Path, apps: &[AppEntry], ctl: &ScanControl) -> Vec<Installer> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        for p in fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()) {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with('.') {
                continue;
            }
            let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if INSTALLER_EXTS.contains(&ext.as_str()) {
                out.push(p);
            } else if depth > 0 && p.is_dir() && !matches!(ext.as_str(), "app" | "photoslibrary" | "bundle") {
                walk(&p, depth - 1, out);
            }
        }
    }
    let mut found = Vec::new();
    walk(home, 0, &mut found);
    for d in ["Downloads", "Desktop", "Documents"] {
        walk(&home.join(d), 4, &mut found);
    }
    let cutoff = now() - 30 * DAY;
    let mut out: Vec<Installer> = found
        .par_iter()
        .map(|p| {
            ctl.set_current(p);
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            let installed_app = apps
                .iter()
                .find(|a| matching::tokens(&a.name, "").iter().any(|t| lower.contains(t.as_str())))
                .map(|a| a.name.clone());
            let meta = spotlight::meta(p);
            let added = meta.added.or_else(|| crate::cleanup::mtime_of(p));
            Installer {
                path: p.to_string_lossy().into_owned(),
                size: fsutil::disk_usage(p),
                recommended: installed_app.is_some() || added.is_some_and(|t| t < cutoff),
                name,
                added,
                installed_app,
            }
        })
        .collect();
    out.sort_unstable_by_key(|i| std::cmp::Reverse(i.size));
    out
}

pub fn scan(ctl: &ScanControl) -> AppsReport {
    let started = Instant::now();
    fsutil::set_observer_io_policy();
    let home = home_dir();
    let running = apps::running_bundle_ids();
    let library = matching::index_library(&home);
    ctl.add(library.len() as u64, 0);

    let installed: Vec<AppInfo> = apps::installed().into_iter().filter(user_app).collect();
    let universe = Universe::build(&running);

    // Two copies with one bundle id (two Python versions' IDLE.app) share their support
    // files; removing one copy must leave those for the other.
    let mut copies: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for a in &installed {
        *copies.entry(a.bundle_id.to_ascii_lowercase()).or_default() += 1;
    }
    let mut claimed: HashSet<&Path> = HashSet::new();
    let mut entries: Vec<AppEntry> = installed
        .iter()
        .map(|a| {
            let mine: Vec<&LibEntry> = library.iter().filter(|e| matching::belongs_to(e, &a.bundle_id, &a.name)).collect();
            claimed.extend(mine.iter().map(|e| e.path.as_path()));
            let shared = copies[&a.bundle_id.to_ascii_lowercase()] > 1;
            (a, if shared { Vec::new() } else { mine }, shared)
        })
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(a, mine, shared)| {
            ctl.set_current(&a.path);
            let size = fsutil::disk_usage(&a.path);
            ctl.add(1, size);
            let related = related(&mine);
            AppEntry {
                id: a.path.to_string_lossy().into_owned(),
                bundle_id: a.bundle_id.clone(),
                name: a.name.clone(),
                version: a.version.clone(),
                path: a.path.to_string_lossy().into_owned(),
                size,
                related_size: related.iter().map(|f| f.size).sum(),
                related,
                last_used: spotlight::meta(&a.path).last_used,
                app_store: a.path.join("Contents/_MASReceipt").exists(),
                running: running.contains(&a.bundle_id.to_ascii_lowercase()),
                shares_support_files: shared,
            }
        })
        .collect();
    entries.sort_unstable_by_key(|a| std::cmp::Reverse(a.size + a.related_size));

    // Orphans: bundle-id named entries nothing installed claims, grouped per app.
    let mut orphans: BTreeMap<String, Vec<&LibEntry>> = BTreeMap::new();
    for e in &library {
        if claimed.contains(e.path.as_path()) || ctl.is_cancelled() {
            continue;
        }
        if let Some(id) = matching::orphan_id(e, &universe) {
            orphans.entry(id).or_default().push(e);
        }
    }
    let mut leftovers: Vec<Leftover> = orphans
        .into_par_iter()
        .filter_map(|(id, list)| {
            let files = related(&list);
            let size: u64 = files.iter().map(|f| f.size).sum();
            ctl.add(files.len() as u64, size);
            (size > 0).then(|| Leftover {
                vendor_installed: false,
                last_activity: list.iter().filter_map(|e| crate::cleanup::mtime_of(&e.path)).max(),
                id,
                files,
                size,
            })
        })
        .collect();
    // Something still writing to it is not an orphan: a shared component of an installed
    // app (Adobe's growth SDK), or an app you run from a build folder (`tauri dev`).
    let vendors: HashSet<String> =
        installed.iter().map(|a| a.bundle_id.to_ascii_lowercase().split('.').take(2).collect::<Vec<_>>().join(".")).collect();
    let now = now();
    leftovers.retain(|l| {
        let age = l.last_activity.map_or(i64::MAX, |t| now - t);
        let vendor = l.id.split('.').take(2).collect::<Vec<_>>().join(".");
        age > 14 * DAY && !(vendors.contains(&vendor) && age < 60 * DAY)
    });
    for l in &mut leftovers {
        l.vendor_installed = vendors.contains(&l.id.split('.').take(2).collect::<Vec<_>>().join("."));
    }
    leftovers.sort_unstable_by_key(|l| std::cmp::Reverse(l.size));

    let installers = installers(&home, &entries, ctl);

    AppsReport {
        apps: entries,
        leftovers,
        installers,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: ctl.is_cancelled(),
    }
}

/// Paths root may touch: apps in /Applications and system-wide support folders.
fn admin_allowed(path: &Path) -> bool {
    const SYSTEM_DIRS: &[&str] = &[
        "/Library/LaunchAgents",
        "/Library/LaunchDaemons",
        "/Library/PrivilegedHelperTools",
        "/Library/Application Support",
        "/Library/Caches",
        "/Library/Preferences",
        "/Library/Logs",
    ];
    let Some(parent) = path.parent() else { return false };
    let is_app = path.extension().is_some_and(|e| e == "app") && path.starts_with("/Applications");
    (is_app || SYSTEM_DIRS.iter().any(|d| parent == Path::new(d)))
        && safety::protection(path, &home_dir()).is_none()
        && !path.components().any(|c| c.as_os_str() == "..")
}

/// A free name in the user's Trash for a root-moved item.
fn trash_destination(path: &Path) -> PathBuf {
    let trash = home_dir().join(".Trash");
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut dest = trash.join(&name);
    let mut n = 1;
    while dest.exists() {
        n += 1;
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
            _ => (name.clone(), String::new()),
        };
        dest = trash.join(format!("{stem} {n}{ext}"));
    }
    dest
}

struct Job {
    id: String,
    name: String,
    size: u64,
    paths: Vec<PathBuf>,
    error: Option<String>,
}

impl AppsReport {
    fn job(&self, t: &Target) -> Option<Job> {
        let job = |id: &str, name: &str, size, paths: Vec<PathBuf>, error| Job { id: id.into(), name: name.into(), size, paths, error };
        match t {
            Target::App(id) => self.apps.iter().find(|a| &a.id == id).map(|a| {
                let mut paths = vec![PathBuf::from(&a.path)];
                paths.extend(a.related.iter().map(|f| PathBuf::from(&f.path)));
                let running = a.running && apps::running_bundle_ids().contains(&a.bundle_id.to_ascii_lowercase());
                let error = running.then(|| format!("{} is running. Quit it, then try again.", a.name));
                job(&a.id, &a.name, a.size + a.related_size, paths, error)
            }),
            Target::Leftover(id) => self
                .leftovers
                .iter()
                .find(|l| &l.id == id)
                .map(|l| job(&l.id, &l.id, l.size, l.files.iter().map(|f| PathBuf::from(&f.path)).collect(), None)),
            Target::Installer(p) => {
                self.installers.iter().find(|i| &i.path == p).map(|i| job(&i.path, &i.name, i.size, vec![PathBuf::from(&i.path)], None))
            }
        }
    }

    /// Everything goes to the Trash. Root-owned items are batched behind one password
    /// prompt; launchd jobs are unloaded before their plists move.
    pub fn remove(&mut self, targets: &[Target], mut on_event: impl FnMut(CleanEvent)) -> CleanSummary {
        let mut jobs: Vec<Job> = targets.iter().filter_map(|t| self.job(t)).collect();
        let remover = Remover::default();
        let uid = unsafe { libc::getuid() };
        let mut admin_cmds: Vec<Vec<String>> = Vec::new();
        let total = jobs.len();

        for (index, job) in jobs.iter_mut().enumerate() {
            on_event(CleanEvent::Started { id: job.id.clone(), name: job.name.clone(), size: job.size, index, total });
            if job.error.is_some() {
                continue;
            }
            for p in &job.paths {
                let is_launchd = p.extension().is_some_and(|e| e == "plist")
                    && p.parent().is_some_and(|d| d.ends_with("LaunchAgents") || d.ends_with("LaunchDaemons"));
                if needs_admin(p) {
                    if !admin_allowed(p) {
                        job.error = Some(format!("Won't remove {} with administrator rights", p.display()));
                        break;
                    }
                    if is_launchd {
                        let domain = if p.starts_with("/Library/LaunchDaemons") { "system".to_string() } else { format!("gui/{uid}") };
                        admin_cmds.push(vec!["/bin/launchctl".into(), "bootout".into(), domain, p.to_string_lossy().into()]);
                    }
                    admin_cmds.push(vec!["/bin/mv".into(), "-n".into(), p.to_string_lossy().into(), trash_destination(p).to_string_lossy().into()]);
                } else {
                    if is_launchd {
                        let _ = std::process::Command::new("/bin/launchctl")
                            .args(["bootout", &format!("gui/{uid}")])
                            .arg(p)
                            .output();
                    }
                    if let Err(e) = remover.remove(p, RemoveMode::Trash) {
                        job.error = Some(e);
                        break;
                    }
                }
            }
        }

        let admin_result = privileged::run_as_admin(&admin_cmds, "PenCleaner needs your password to remove items installed for all users.");

        let mut summary = CleanSummary::default();
        for job in jobs {
            let left: Vec<&PathBuf> = job.paths.iter().filter(|p| fs::symlink_metadata(p).is_ok()).collect();
            match (&job.error, left.first()) {
                (None, None) => {
                    summary.bytes_freed += job.size;
                    summary.cleaned.push(job.id.clone());
                    on_event(CleanEvent::Cleaned { id: job.id, bytes: job.size });
                }
                (err, rest) => {
                    let error = err.clone().unwrap_or_else(|| match &admin_result {
                        Err(e) => e.clone(),
                        Ok(()) => format!("Couldn't remove {}", rest.map_or(String::new(), |p| p.display().to_string())),
                    });
                    summary.failed.push((job.id.clone(), error.clone()));
                    on_event(CleanEvent::Failed { id: job.id, error, bytes: 0 });
                }
            }
        }

        let done: HashSet<&str> = summary.cleaned.iter().map(String::as_str).collect();
        self.apps.retain(|a| !done.contains(a.id.as_str()));
        self.leftovers.retain(|l| !done.contains(l.id.as_str()));
        self.installers.retain(|i| !done.contains(i.path.as_str()));
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_paths_are_tightly_limited() {
        assert!(admin_allowed(Path::new("/Applications/Zoom.app")));
        assert!(admin_allowed(Path::new("/Library/LaunchDaemons/us.zoom.ZoomDaemon.plist")));
        assert!(!admin_allowed(Path::new("/Library/LaunchDaemons")));
        assert!(!admin_allowed(Path::new("/Applications")));
        assert!(!admin_allowed(Path::new("/System/Library/LaunchDaemons/com.apple.x.plist")));
        assert!(!admin_allowed(Path::new("/Library/Extensions/Foo.kext")));
        assert!(!admin_allowed(Path::new("/usr/local/bin/foo")));
    }

    #[test]
    fn trash_destination_never_overwrites() {
        let d = trash_destination(Path::new("/Applications/__pencleaner_test__.app"));
        assert!(d.starts_with(home_dir().join(".Trash")));
        assert!(!d.exists());
    }
}
