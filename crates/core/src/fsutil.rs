use parking_lot::Mutex;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Bytes actually allocated on disk. Unlike `len()`, this is 0 for iCloud files that
/// have been evicted and small for sparse files, which matches what freeing them returns.
pub fn allocated(md: &Metadata) -> u64 {
    md.blocks() * 512
}

/// `st_flags` bit for iCloud / File Provider items whose contents live only in the cloud.
const SF_DATALESS: u32 = 0x4000_0000;

/// Cloud-only placeholder: occupies no local space, and reading it would download it.
pub fn is_dataless(md: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt as _;
    md.st_flags() & SF_DATALESS != 0
}

/// Makes the whole process a polite observer: touching an iCloud placeholder fails fast
/// instead of downloading it (a scan would otherwise hang on getdirentries), and reads
/// don't bump access times, so "last opened" dates stay meaningful.
pub fn set_observer_io_policy() {
    // <sys/resource.h>; not exposed by the libc crate.
    const IOPOL_TYPE_VFS_ATIME_UPDATES: libc::c_int = 2;
    const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: libc::c_int = 3;
    const IOPOL_SCOPE_PROCESS: libc::c_int = 0;
    const IOPOL_ATIME_UPDATES_OFF: libc::c_int = 1;
    const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: libc::c_int = 1;
    extern "C" {
        fn setiopolicy_np(iotype: libc::c_int, scope: libc::c_int, policy: libc::c_int) -> libc::c_int;
    }
    unsafe {
        setiopolicy_np(
            IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
            IOPOL_SCOPE_PROCESS,
            IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
        );
        setiopolicy_np(IOPOL_TYPE_VFS_ATIME_UPDATES, IOPOL_SCOPE_PROCESS, IOPOL_ATIME_UPDATES_OFF);
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Remembers hard-linked inodes so their bytes are only counted once. Sharded so the
/// parallel walker rarely contends on the same lock.
pub struct InodeSet {
    shards: Vec<Mutex<HashSet<(u64, u64)>>>,
}

impl Default for InodeSet {
    fn default() -> Self {
        Self { shards: (0..64).map(|_| Mutex::new(HashSet::new())).collect() }
    }
}

impl InodeSet {
    /// Returns true the first time an inode is seen.
    pub fn first_sighting(&self, md: &Metadata) -> bool {
        if md.nlink() <= 1 {
            return true;
        }
        let key = (md.dev(), md.ino());
        self.shards[(md.ino() % 64) as usize].lock().insert(key)
    }
}

/// Recursive allocated size of a path, never following symlinks.
pub fn disk_usage(path: &Path) -> u64 {
    let inodes = InodeSet::default();
    usage_inner(path, &inodes)
}

fn usage_inner(path: &Path, inodes: &InodeSet) -> u64 {
    let Ok(md) = fs::symlink_metadata(path) else { return 0 };
    if !md.is_dir() {
        return if inodes.first_sighting(&md) { allocated(&md) } else { 0 };
    }
    if is_dataless(&md) {
        return 0;
    }
    let Ok(rd) = fs::read_dir(path) else { return allocated(&md) };
    let children: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    allocated(&md) + children.par_iter().map(|p| usage_inner(p, inodes)).sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_links_count_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        fs::write(&a, vec![1u8; 64 * 1024]).unwrap();
        fs::hard_link(&a, dir.path().join("b")).unwrap();
        let single = allocated(&fs::metadata(&a).unwrap());
        let total = disk_usage(dir.path());
        let dir_entry = allocated(&fs::metadata(dir.path()).unwrap());
        assert_eq!(total, single + dir_entry);
    }
}
