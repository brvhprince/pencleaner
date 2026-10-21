//! Space Lens: one parallel walk of a volume or folder that keeps a size tree of
//! directories only. Files are listed live when a folder is opened, which keeps
//! memory bounded by the directory count (a 1 TB home has ~1M dirs vs ~8M files).

use crate::fsutil::{self, InodeSet};
use crate::progress::ScanControl;
use crate::safety;
use crate::{Error, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Never descended into: other mounts, and the Data volume's second view of /Users etc.
const SKIP: &[&str] = &[
    "/System/Volumes",
    "/Volumes",
    "/dev",
    "/net",
    "/home",
    "/cores",
    // Magic prefixes that re-expose the whole filesystem by path or inode.
    "/.nofollow",
    "/.resolve",
    "/.vol",
];

/// Listing responses are capped; the remainder is summarized as "Other items".
const MAX_ENTRIES: usize = 1500;

#[derive(Debug, Default)]
pub struct DirNode {
    pub name: Box<str>,
    /// Allocated bytes of everything below, plus the directory itself.
    pub size: u64,
    /// Files and folders below (not counting itself).
    pub items: u64,
    pub children: Vec<DirNode>,
    pub denied: bool,
}

#[derive(Debug)]
pub struct ScanTree {
    pub root_path: PathBuf,
    pub root: DirNode,
    pub denied: u64,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Folder,
    App,
    Bundle,
    File,
    Symlink,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub items: u64,
    pub kind: EntryKind,
    /// Why this entry cannot be removed, if it can't.
    pub protected: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub items: u64,
    pub denied: bool,
    pub entries: Vec<Entry>,
    /// Entries beyond `MAX_ENTRIES`, aggregated.
    pub rest_count: u64,
    pub rest_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root: String,
    pub size: u64,
    pub items: u64,
    pub denied: u64,
    pub cancelled: bool,
    pub elapsed_ms: u64,
}

struct Ctx<'a> {
    ctl: &'a ScanControl,
    inodes: InodeSet,
    devices: Vec<u64>,
    denied: AtomicU64,
}

pub fn scan(root: &Path, ctl: &ScanControl) -> Result<ScanTree> {
    let started = Instant::now();
    fsutil::set_observer_io_policy();
    let md = fs::metadata(root).map_err(|e| Error::io(root, e))?;
    let mut devices = vec![md.dev()];
    if root == Path::new("/") {
        // /Users, /Applications… are firmlinks into the Data volume.
        if let Ok(data) = fs::metadata("/System/Volumes/Data") {
            devices.push(data.dev());
        }
    }
    let ctx = Ctx { ctl, inodes: InodeSet::default(), devices, denied: AtomicU64::new(0) };

    // APFS metadata calls are the bottleneck: past ~8 threads time barely improves
    // while kernel CPU doubles (measured on 4.5M items: 4→62s, 8→42s, 16→39s, 32→41s).
    let threads = std::env::var("PENCLEANER_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| (std::thread::available_parallelism().map_or(8, |n| n.get()) / 2).max(4));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(16 * 1024 * 1024)
        .thread_name(|i| format!("space-lens-{i}"))
        .build()
        .map_err(|e| Error::Other(e.to_string()))?;

    let name = display_name(root);
    let root_node = pool.install(|| walk(root, name.into(), fsutil::allocated(&md), &ctx));

    Ok(ScanTree {
        root_path: root.to_path_buf(),
        root: root_node,
        denied: ctx.denied.load(Ordering::Relaxed),
        cancelled: ctl.is_cancelled(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn walk(path: &Path, name: Box<str>, own_size: u64, ctx: &Ctx) -> DirNode {
    let mut node = DirNode { name, size: own_size, ..Default::default() };
    if ctx.ctl.is_cancelled() {
        return node;
    }
    ctx.ctl.set_current(path);
    let rd = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => {
            node.denied = true;
            ctx.denied.fetch_add(1, Ordering::Relaxed);
            return node;
        }
    };

    let mut subdirs = Vec::new();
    let (mut items, mut file_bytes) = (0u64, 0u64);
    for entry in rd.flatten() {
        // DirEntry::metadata does not follow symlinks.
        let Ok(md) = entry.metadata() else { continue };
        items += 1;
        if md.is_dir() {
            let p = entry.path();
            if !ctx.devices.contains(&md.dev())
                || fsutil::is_dataless(&md)
                || SKIP.iter().any(|s| p == Path::new(s))
            {
                continue;
            }
            let name: Box<str> = entry.file_name().to_string_lossy().into();
            subdirs.push((p, name, fsutil::allocated(&md)));
        } else if ctx.inodes.first_sighting(&md) {
            file_bytes += fsutil::allocated(&md);
        }
    }
    ctx.ctl.add(items, file_bytes);

    let mut children: Vec<DirNode> = subdirs
        .into_par_iter()
        .map(|(p, n, s)| walk(&p, n, s, ctx))
        .collect();
    children.sort_unstable_by_key(|c| std::cmp::Reverse(c.size));
    children.shrink_to_fit();

    node.size += file_bytes + children.iter().map(|c| c.size).sum::<u64>();
    node.items = items + children.iter().map(|c| c.items).sum::<u64>();
    node.children = children;
    node
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn kind_of(name: &str, is_dir: bool, is_symlink: bool) -> EntryKind {
    if is_symlink {
        return EntryKind::Symlink;
    }
    if !is_dir {
        return EntryKind::File;
    }
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "app" => EntryKind::App,
        "bundle" | "framework" | "photoslibrary" | "musiclibrary" | "tvlibrary" | "xcarchive"
        | "plugin" | "kext" | "pkg" | "logicx" | "fcpbundle" | "imovielibrary" | "band" => {
            EntryKind::Bundle
        }
        _ => EntryKind::Folder,
    }
}

impl ScanTree {
    pub fn summary(&self) -> ScanSummary {
        ScanSummary {
            root: self.root_path.to_string_lossy().into_owned(),
            size: self.root.size,
            items: self.root.items,
            denied: self.denied,
            cancelled: self.cancelled,
            elapsed_ms: self.elapsed_ms,
        }
    }

    fn rel_names(&self, path: &Path) -> Option<Vec<String>> {
        let rel = path.strip_prefix(&self.root_path).ok()?;
        Some(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect())
    }

    pub fn node(&self, path: &Path) -> Option<&DirNode> {
        let mut node = &self.root;
        for name in self.rel_names(path)? {
            node = node.children.iter().find(|c| *c.name == *name)?;
        }
        Some(node)
    }

    /// Folders come from the tree; files are read live from disk.
    pub fn list(&self, path: &Path, home: &Path) -> Result<Listing> {
        let node = self.node(path).ok_or_else(|| Error::NotInTree(path.to_path_buf()))?;
        let mut entries: Vec<Entry> = node
            .children
            .iter()
            .map(|c| {
                let p = path.join(&*c.name);
                Entry {
                    kind: kind_of(&c.name, true, false),
                    protected: safety::protection(&p, home),
                    name: c.name.to_string(),
                    path: p.to_string_lossy().into_owned(),
                    size: c.size,
                    items: c.items,
                }
            })
            .collect();

        if let Ok(rd) = fs::read_dir(path) {
            for entry in rd.flatten() {
                let Ok(md) = entry.metadata() else { continue };
                if md.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let p = entry.path();
                entries.push(Entry {
                    kind: kind_of(&name, false, md.file_type().is_symlink()),
                    protected: safety::protection(&p, home),
                    name,
                    path: p.to_string_lossy().into_owned(),
                    size: fsutil::allocated(&md),
                    items: 0,
                });
            }
        }

        entries.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
        let (mut rest_count, mut rest_size) = (0, 0);
        if entries.len() > MAX_ENTRIES {
            for e in entries.drain(MAX_ENTRIES..) {
                rest_count += 1;
                rest_size += e.size;
            }
        }

        Ok(Listing {
            path: path.to_string_lossy().into_owned(),
            name: node.name.to_string(),
            size: node.size,
            items: node.items,
            denied: node.denied,
            entries,
            rest_count,
            rest_size,
        })
    }

    /// Updates sizes after `path` was removed. `file_bytes` is used when the path is a
    /// file (folders already know their size). Returns false if it wasn't in the tree.
    pub fn forget(&mut self, path: &Path, file_bytes: u64) -> bool {
        let Some(names) = self.rel_names(path) else { return false };
        let Some((last, parents)) = names.split_last() else { return false };
        if path.parent().and_then(|p| self.node(p)).is_none() {
            return false;
        }
        let (bytes, items) = match self.node(path) {
            Some(n) => (n.size, n.items + 1),
            None => (file_bytes, 1),
        };
        let mut node = &mut self.root;
        node.size = node.size.saturating_sub(bytes);
        node.items = node.items.saturating_sub(items);
        for name in parents {
            node = node.children.iter_mut().find(|c| *c.name == **name).expect("validated");
            node.size = node.size.saturating_sub(bytes);
            node.items = node.items.saturating_sub(items);
        }
        if let Some(i) = node.children.iter().position(|c| *c.name == **last) {
            node.children.remove(i);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, kb: usize) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![7u8; kb * 1024]).unwrap();
    }

    #[test]
    fn scans_lists_and_forgets() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        write(&root.join("big/a.bin"), 512);
        write(&root.join("big/deeper/b.bin"), 256);
        write(&root.join("small/c.bin"), 8);
        write(&root.join("loose.bin"), 64);

        let ctl = ScanControl::new();
        let mut tree = scan(&root, &ctl).unwrap();
        assert!(!tree.cancelled);
        assert_eq!(tree.root.children[0].name.as_ref(), "big");
        assert!(tree.root.size >= (512 + 256 + 8 + 64) * 1024);
        // big, deeper, small, a, b, c, loose
        assert_eq!(tree.root.items, 7);

        let home = Path::new("/nonexistent-home");
        let listing = tree.list(&root, home).unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["big", "loose.bin", "small"]);
        assert_eq!(listing.entries[1].kind, EntryKind::File);

        let before = tree.root.size;
        let big = tree.node(&root.join("big")).unwrap().size;
        assert!(tree.forget(&root.join("big"), 0));
        assert_eq!(tree.root.size, before - big);
        assert!(tree.node(&root.join("big")).is_none());
        assert_eq!(tree.root.items, 3);
    }

    #[test]
    fn cancellation_stops_early() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("x/y.bin"), 4);
        let ctl = ScanControl::new();
        ctl.cancel();
        let tree = scan(dir.path(), &ctl).unwrap();
        assert!(tree.cancelled);
        assert_eq!(tree.root.items, 0);
    }
}
