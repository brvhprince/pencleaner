//! My Clutter: the user's own files. Large & old files, exact duplicates, similar images
//! and the Downloads pile. Everything here is personal data, so removal always goes to
//! the Trash and nothing is pre-selected by the engine.

mod duplicates;
mod similar;

use crate::cleanup::dev::{is_build_artifact, BUNDLE_EXTS};
use crate::fsutil::{self, home_dir};
use crate::imaging::IMAGE_EXTS;
use crate::progress::ScanControl;
use crate::remove::{RemoveEvent, RemoveMode, RemoveSummary, Remover};
use crate::spotlight;
use parking_lot::Mutex;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

/// Files below this never matter to any of the four views.
const MIN_FILE: u64 = 32 * 1024;
pub const LARGE_FILE: u64 = 50 * 1000 * 1000;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    Image,
    Video,
    Audio,
    Archive,
    Installer,
    Document,
    Folder,
    Other,
}

pub fn kind_of(path: &Path, is_dir: bool) -> FileKind {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let e = ext.as_str();
    if is_dir {
        return if e == "app" { FileKind::Installer } else { FileKind::Folder };
    }
    match e {
        _ if IMAGE_EXTS.contains(&e) => FileKind::Image,
        "cr2" | "cr3" | "nef" | "arw" | "dng" | "raf" | "orf" | "rw2" | "svg" => FileKind::Image,
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "wmv" | "flv" | "mpg" | "mpeg" => FileKind::Video,
        "mp3" | "m4a" | "wav" | "aac" | "flac" | "aiff" | "aif" | "ogg" | "opus" => FileKind::Audio,
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" => FileKind::Archive,
        "dmg" | "pkg" | "mpkg" | "xip" | "iso" | "exe" | "msi" | "apk" | "aab" | "ipa" => FileKind::Installer,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "key" | "pages" | "numbers" | "txt" | "rtf" | "csv"
        | "md" | "epub" | "psd" | "ai" | "sketch" | "fig" | "xd" => FileKind::Document,
        _ => FileKind::Other,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    /// The path; also the id the UI sends back.
    pub path: String,
    pub name: String,
    pub size: u64,
    pub kind: FileKind,
    pub modified: Option<i64>,
    pub created: Option<i64>,
    /// Last opened, from Spotlight. Absent if never opened through Finder or an app.
    pub last_used: Option<i64>,
    /// When it landed in its folder (Downloads), from Spotlight.
    pub added: Option<i64>,
    /// Download origin, e.g. "github.com".
    pub source: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSet {
    pub id: String,
    pub files: Vec<FileInfo>,
    /// The copy we suggest keeping.
    pub keep: String,
    /// Space freed by removing all but `keep`; less than the sum when copies are APFS clones.
    pub reclaimable: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClutterReport {
    pub root: String,
    pub files_scanned: u64,
    pub large: Vec<FileInfo>,
    pub duplicates: Vec<FileSet>,
    pub similar: Vec<FileSet>,
    pub downloads: Vec<FileInfo>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

/// A file seen by the walk, before any expensive lookups.
#[derive(Debug, Clone)]
pub(crate) struct Rec {
    pub path: PathBuf,
    pub len: u64,
    pub alloc: u64,
    pub dev: u64,
    pub ino: u64,
    pub modified: Option<i64>,
    pub created: Option<i64>,
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    t.ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

impl Rec {
    fn from(path: PathBuf, md: &Metadata) -> Self {
        Rec {
            len: md.len(),
            alloc: fsutil::allocated(md),
            dev: md.dev(),
            ino: md.ino(),
            modified: secs(md.modified()),
            created: secs(md.created()),
            path,
        }
    }

    pub fn info(&self) -> FileInfo {
        FileInfo {
            path: self.path.to_string_lossy().into_owned(),
            name: self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            size: self.alloc,
            kind: kind_of(&self.path, false),
            modified: self.modified,
            created: self.created,
            last_used: None,
            added: None,
            source: None,
            width: None,
            height: None,
        }
    }
}

fn with_spotlight(mut info: FileInfo) -> FileInfo {
    let m = spotlight::meta(Path::new(&info.path));
    info.last_used = m.last_used;
    info.added = m.added;
    info.source = m.source;
    info
}

struct Walk<'a> {
    ctl: &'a ScanControl,
    home: PathBuf,
    /// Ignored folders: never walked.
    skip: &'a [PathBuf],
    out: Mutex<Vec<Rec>>,
}

fn walk(dir: &Path, depth: usize, w: &Walk) {
    if depth > 40 || w.ctl.is_cancelled() {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut names = Vec::new();
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            dirs.push((name, e.path()));
        } else if ft.is_file() {
            names.push(name);
            files.push(e);
        }
    }
    let mut found = Vec::new();
    let mut bytes = 0;
    for e in &files {
        let Ok(md) = e.metadata() else { continue };
        if md.len() < MIN_FILE || fsutil::is_dataless(&md) {
            continue;
        }
        bytes += md.len();
        found.push(Rec::from(e.path(), &md));
    }
    w.ctl.add(files.len() as u64, bytes);
    if depth < 8 {
        w.ctl.set_current(dir);
    }
    if !found.is_empty() {
        w.out.lock().extend(found);
    }

    let subdirs: Vec<PathBuf> = dirs
        .into_iter()
        .filter(|(name, path)| {
            let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
            !(name.starts_with('.')
                || name == "node_modules"
                || BUNDLE_EXTS.contains(&ext)
                || (dir == w.home && name == "Library")
                || w.skip.iter().any(|s| path.starts_with(s))
                || is_build_artifact(name, path, &names))
        })
        .map(|(_, p)| p)
        .collect();
    subdirs.par_iter().for_each(|p| walk(p, depth + 1, w));
}

fn downloads(root: &Path, ctl: &ScanControl) -> Vec<FileInfo> {
    let dir = if root.file_name().is_some_and(|n| n == "Downloads") { root.to_path_buf() } else { root.join("Downloads") };
    let Ok(rd) = fs::read_dir(&dir) else { return vec![] };
    let entries: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    entries
        .par_iter()
        .filter_map(|p| {
            let md = fs::symlink_metadata(p).ok()?;
            ctl.set_current(p);
            let size = if md.is_dir() { fsutil::disk_usage(p) } else { fsutil::allocated(&md) };
            Some(with_spotlight(FileInfo {
                path: p.to_string_lossy().into_owned(),
                name: p.file_name()?.to_string_lossy().into_owned(),
                size,
                kind: kind_of(p, md.is_dir()),
                modified: secs(md.modified()),
                created: secs(md.created()),
                last_used: None,
                added: None,
                source: None,
                width: None,
                height: None,
            }))
        })
        .collect()
}

pub fn scan(root: &Path, ctl: &ScanControl) -> ClutterReport {
    scan_with(root, LARGE_FILE, &crate::settings::Ignore::default(), ctl)
}

/// `large`: the "large file" threshold in bytes. Ignored folders are never walked and
/// ignored files never reported.
pub fn scan_with(root: &Path, large: u64, ignore: &crate::settings::Ignore, ctl: &ScanControl) -> ClutterReport {
    let started = Instant::now();
    fsutil::set_observer_io_policy();
    let w = Walk { ctl, home: home_dir(), skip: ignore.paths(), out: Mutex::new(Vec::new()) };
    walk(root, 0, &w);
    let recs = w.out.into_inner();
    let files_scanned = ctl.snapshot().items;

    let mut large: Vec<FileInfo> =
        recs.par_iter().filter(|r| r.len >= large).map(|r| with_spotlight(r.info())).collect();
    large.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));

    let duplicates = if ctl.is_cancelled() { vec![] } else { duplicates::find(&recs, ctl) };
    let extra_copies: HashSet<&Path> = duplicates
        .iter()
        .flat_map(|s| s.files.iter().filter(|f| f.path != s.keep).map(|f| Path::new(f.path.as_str())))
        .collect();
    let similar = if ctl.is_cancelled() { vec![] } else { similar::find(&recs, &extra_copies, ctl) };
    let mut downloads = if ctl.is_cancelled() { vec![] } else { downloads(root, ctl) };
    downloads.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));

    ClutterReport {
        root: root.to_string_lossy().into_owned(),
        files_scanned,
        large,
        duplicates,
        similar,
        downloads,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: ctl.is_cancelled(),
    }
}

impl ClutterReport {
    /// Drops ignored files; duplicate and similar sets left with one file disappear.
    pub fn apply_ignore(&mut self, ig: &crate::settings::Ignore) {
        if ig.is_empty() {
            return;
        }
        let gone = |f: &FileInfo| ig.path_str(&f.path) || ig.item(&format!("clutter:{}", f.path));
        self.large.retain(|f| !gone(f));
        self.downloads.retain(|f| !gone(f));
        for sets in [&mut self.duplicates, &mut self.similar] {
            for s in sets.iter_mut() {
                s.files.retain(|f| !gone(f));
                if !s.files.iter().any(|f| f.path == s.keep) {
                    if let Some(first) = s.files.first() {
                        s.keep = first.path.clone();
                    }
                }
                s.reclaimable = s.files.iter().filter(|f| f.path != s.keep).map(|f| f.size).sum();
            }
            sets.retain(|s| s.files.len() > 1 && !ig.item(&format!("clutter:{}", s.id)));
        }
    }

    fn known_paths(&self) -> HashSet<&str> {
        let sets = self.duplicates.iter().chain(&self.similar).flat_map(|s| &s.files);
        self.large.iter().chain(&self.downloads).chain(sets).map(|f| f.path.as_str()).collect()
    }

    /// Moves the chosen files to the Trash. Only paths from this report are accepted.
    pub fn remove(&mut self, ids: &[String], on_event: impl FnMut(RemoveEvent)) -> RemoveSummary {
        self.remove_with(ids, RemoveMode::Trash, on_event)
    }

    fn remove_with(&mut self, ids: &[String], mode: RemoveMode, mut on_event: impl FnMut(RemoveEvent)) -> RemoveSummary {
        let known = self.known_paths();
        let size_of = |p: &str| {
            let sets = self.duplicates.iter().chain(&self.similar).flat_map(|s| &s.files);
            self.large.iter().chain(&self.downloads).chain(sets).find(|f| f.path == p).map_or(0, |f| f.size)
        };
        let jobs: Vec<(String, u64)> =
            ids.iter().filter(|id| known.contains(id.as_str())).map(|id| (id.clone(), size_of(id))).collect();
        drop(known);

        let remover = Remover::default();
        let mut summary = RemoveSummary::default();
        for (index, (path, bytes)) in jobs.iter().enumerate() {
            on_event(RemoveEvent::Started { path: path.clone(), index, total: jobs.len() });
            match remover.remove(Path::new(path), mode) {
                Ok(()) => {
                    summary.bytes_freed += bytes;
                    summary.removed.push((path.clone(), *bytes));
                    on_event(RemoveEvent::Removed { path: path.clone(), bytes: *bytes });
                }
                Err(error) => {
                    summary.failed.push((path.clone(), error.clone()));
                    on_event(RemoveEvent::Failed { path: path.clone(), error });
                }
            }
        }

        let gone: HashSet<&str> = summary.removed.iter().map(|(p, _)| p.as_str()).collect();
        self.large.retain(|f| !gone.contains(f.path.as_str()));
        self.downloads.retain(|f| !gone.contains(f.path.as_str()));
        for sets in [&mut self.duplicates, &mut self.similar] {
            for s in sets.iter_mut() {
                s.files.retain(|f| !gone.contains(f.path.as_str()));
            }
            sets.retain(|s| s.files.len() > 1);
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_duplicates_and_skips_build_output() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let blob: Vec<u8> = (0..700_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = blob.clone();
        *other.last_mut().unwrap() ^= 1; // same size, different content

        fs::create_dir_all(root.join("Documents/old")).unwrap();
        fs::create_dir_all(root.join("Downloads")).unwrap();
        fs::write(root.join("Documents/report.bin"), &blob).unwrap();
        fs::write(root.join("Documents/old/report copy.bin"), &blob).unwrap();
        fs::write(root.join("Downloads/report.bin"), &blob).unwrap();
        fs::write(root.join("Documents/different.bin"), &other).unwrap();
        // Inside a Rust target dir: Cleanup's business, never clutter.
        fs::create_dir_all(root.join("proj/target")).unwrap();
        fs::write(root.join("proj/Cargo.toml"), b"[package]").unwrap();
        fs::write(root.join("proj/target/report.bin"), &blob).unwrap();

        let ctl = ScanControl::new();
        let mut report = scan(&root, &ctl);
        assert_eq!(report.duplicates.len(), 1);
        let set = &report.duplicates[0];
        assert_eq!(set.files.len(), 3);
        assert!(!set.keep.contains("Downloads"), "keeps the copy outside Downloads: {}", set.keep);
        assert!(set.reclaimable > 0);
        assert_eq!(report.downloads.len(), 1);

        let victim = set.files.iter().find(|f| f.path != set.keep).unwrap().path.clone();
        // Delete, not Trash: tests must not leave files in the user's Trash.
        let summary = report.remove_with(&[victim.clone(), "/etc/hosts".into()], RemoveMode::Delete, |_| {});
        assert_eq!(summary.removed.len(), 1, "unknown paths are ignored");
        assert!(!Path::new(&victim).exists());
        assert_eq!(report.duplicates[0].files.len(), 2);
    }
}
