use crate::fsutil;
use crate::safety;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoveMode {
    /// Recoverable from the Trash. Used for anything the user created.
    Trash,
    /// Gone immediately. Used for caches and build output that regenerate.
    Delete,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveItem {
    pub path: PathBuf,
    /// Size already known to the caller (e.g. from a scan); measured when absent.
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum RemoveEvent {
    Started { path: String, index: usize, total: usize },
    Removed { path: String, bytes: u64 },
    Failed { path: String, error: String },
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSummary {
    pub removed: Vec<(String, u64)>,
    pub failed: Vec<(String, String)>,
    pub bytes_freed: u64,
}

/// Removes paths after checking them against `safety`. Reuse one per batch.
pub struct Remover {
    home: PathBuf,
    trash: trash::TrashContext,
}

impl Default for Remover {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut trash = trash::TrashContext::default();
        #[cfg(target_os = "macos")]
        {
            use trash::macos::{DeleteMethod, TrashContextExtMacos};
            // Finder's method plays a sound per item and needs Automation permission.
            trash.set_delete_method(DeleteMethod::NsFileManager);
        }
        Self { home: fsutil::home_dir(), trash }
    }
}

impl Remover {
    /// A path that is already gone counts as removed.
    pub fn remove(&self, path: &Path, mode: RemoveMode) -> Result<(), String> {
        self.check(path)?;
        let md = match fs::symlink_metadata(path) {
            Ok(md) => md,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        match mode {
            RemoveMode::Trash => self.trash.delete(path).map_err(|e| e.to_string()),
            RemoveMode::Delete => delete(path, md.is_dir()).or_else(|e| {
                // Some tools (Go's module cache) mark their folders read-only.
                if e.kind() == ErrorKind::PermissionDenied && md.is_dir() {
                    make_dirs_writable(path);
                    delete(path, true).map_err(|e| e.to_string())
                } else {
                    Err(e.to_string())
                }
            }),
        }
    }

    /// Checks the path as given and with its parent resolved, so /var/db (a symlink to
    /// /private/var/db) can't sneak past the rules. The item itself may be a symlink we
    /// mean to remove, so only the parent is resolved.
    fn check(&self, path: &Path) -> Result<(), String> {
        let resolved = match (path.parent().map(fs::canonicalize), path.file_name()) {
            (Some(Ok(parent)), Some(name)) => parent.join(name),
            _ => path.to_path_buf(),
        };
        for p in [path, resolved.as_path()] {
            if let Some(reason) = safety::protection(p, &self.home) {
                return Err(format!("Protected: {reason}"));
            }
        }
        Ok(())
    }
}

fn delete(path: &Path, is_dir: bool) -> std::io::Result<()> {
    let result = if is_dir { fs::remove_dir_all(path) } else { fs::remove_file(path) };
    match result {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn make_dirs_writable(path: &Path) {
    let Ok(md) = fs::symlink_metadata(path) else { return };
    if !md.is_dir() {
        return;
    }
    let mut perms = md.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(perms.mode() | 0o700);
    }
    // Windows only: clears the read-only attribute (the Unix caveat clippy warns about
    // doesn't apply; Unix takes the branch above).
    #[cfg(windows)]
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    let _ = fs::set_permissions(path, perms);
    if let Ok(rd) = fs::read_dir(path) {
        for e in rd.flatten() {
            make_dirs_writable(&e.path());
        }
    }
}

pub fn remove_items(
    items: &[RemoveItem],
    mode: RemoveMode,
    mut on_event: impl FnMut(RemoveEvent),
) -> RemoveSummary {
    let remover = Remover::default();
    let mut summary = RemoveSummary::default();
    for (index, item) in items.iter().enumerate() {
        let path_str = item.path.to_string_lossy().into_owned();
        on_event(RemoveEvent::Started { path: path_str.clone(), index, total: items.len() });
        let bytes = item.size.unwrap_or_else(|| fsutil::disk_usage(&item.path));
        match remover.remove(&item.path, mode) {
            Ok(()) => {
                summary.bytes_freed += bytes;
                summary.removed.push((path_str.clone(), bytes));
                on_event(RemoveEvent::Removed { path: path_str, bytes });
            }
            Err(error) => {
                summary.failed.push((path_str.clone(), error.clone()));
                on_event(RemoveEvent::Failed { path: path_str, error });
            }
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_and_refuses_protected() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("junk.bin");
        fs::write(&f, b"hello").unwrap();
        let items = vec![
            RemoveItem { path: f.clone(), size: None },
            RemoveItem { path: PathBuf::from("/System"), size: Some(1) },
        ];
        let mut events = 0;
        let s = remove_items(&items, RemoveMode::Delete, |_| events += 1);
        assert!(!f.exists());
        assert_eq!(s.removed.len(), 1);
        assert_eq!(s.failed.len(), 1);
        assert!(s.failed[0].1.starts_with("Protected"));
        assert_eq!(events, 4);
    }

    #[test]
    fn deletes_read_only_trees_and_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let ro = dir.path().join("gomod/pkg@v1");
        fs::create_dir_all(&ro).unwrap();
        fs::write(ro.join("a.go"), b"package a").unwrap();
        for p in [ro.as_path(), ro.parent().unwrap()] {
            let mut perms = fs::metadata(p).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(p, perms).unwrap();
        }
        let r = Remover::default();
        r.remove(&dir.path().join("gomod"), RemoveMode::Delete).unwrap();
        assert!(!dir.path().join("gomod").exists());
        r.remove(&dir.path().join("never-existed"), RemoveMode::Delete).unwrap();
    }
}
