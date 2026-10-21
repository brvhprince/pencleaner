use crate::fsutil;
use crate::safety;
use serde::{Deserialize, Serialize};
use std::fs;
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

pub fn remove_items(
    items: &[RemoveItem],
    mode: RemoveMode,
    mut on_event: impl FnMut(RemoveEvent),
) -> RemoveSummary {
    let home = fsutil::home_dir();
    let mut trash = trash::TrashContext::default();
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // Finder's method plays a sound per item and needs Automation permission.
        trash.set_delete_method(DeleteMethod::NsFileManager);
    }

    let mut summary = RemoveSummary::default();
    for (index, item) in items.iter().enumerate() {
        let path_str = item.path.to_string_lossy().into_owned();
        on_event(RemoveEvent::Started { path: path_str.clone(), index, total: items.len() });

        let result = remove_one(&item.path, item.size, mode, &home, &trash);
        match result {
            Ok(bytes) => {
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

fn remove_one(
    path: &Path,
    known_size: Option<u64>,
    mode: RemoveMode,
    home: &Path,
    trash: &trash::TrashContext,
) -> Result<u64, String> {
    // Check the path as given and with its parent resolved, so /var/db (a symlink to
    // /private/var/db) can't sneak past the rules. The item itself may be a symlink we
    // mean to remove, so only the parent is resolved.
    let resolved = match (path.parent().map(fs::canonicalize), path.file_name()) {
        (Some(Ok(parent)), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    };
    for p in [path, resolved.as_path()] {
        if let Some(reason) = safety::protection(p, home) {
            return Err(format!("Protected: {reason}"));
        }
    }
    let md = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    let bytes = known_size.unwrap_or_else(|| fsutil::disk_usage(path));
    match mode {
        RemoveMode::Trash => trash.delete(path).map_err(|e| e.to_string())?,
        RemoveMode::Delete if md.is_dir() => fs::remove_dir_all(path).map_err(|e| e.to_string())?,
        RemoveMode::Delete => fs::remove_file(path).map_err(|e| e.to_string())?,
    }
    Ok(bytes)
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
}
