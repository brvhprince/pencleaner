use parking_lot::Mutex;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

// What differs per OS: allocated size, cloud placeholders, hard-link identity, bulk
// directory reads, clone detection. Both files expose the same functions.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// Runs a command with a deadline; a hung system tool (sfltool sometimes is) is killed
/// instead of freezing a scan. Returns stdout, or None on timeout, failure to start, or
/// a non-zero exit.
pub fn run_with_timeout(cmd: &str, args: &[&str], limit: std::time::Duration) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    // Drain on another thread so a chatty child can't block on a full pipe.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = reader.join().ok()?;
                return status.success().then(|| String::from_utf8_lossy(&out).into_owned());
            }
            Ok(None) if started.elapsed() < limit => std::thread::sleep(std::time::Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Where PenCleaner keeps settings, the activity log and caches.
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home_dir).join("PenCleaner")
    } else {
        home_dir().join("Library/Application Support/com.pen.cleaner")
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
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
        let (dev, ino, nlink) = link_identity(md);
        self.first_sighting_of(dev, ino, nlink)
    }

    pub fn first_sighting_of(&self, dev: u64, ino: u64, nlink: u64) -> bool {
        if nlink <= 1 {
            return true;
        }
        self.shards[(ino % 64) as usize].lock().insert((dev, ino))
    }
}

/// One directory entry from `read_dir_bulk`, with what a size scan needs.
pub struct BulkEntry<'a> {
    pub name: &'a std::ffi::OsStr,
    pub is_dir: bool,
    pub dev: u64,
    pub ino: u64,
    pub nlink: u64,
    /// Allocated bytes (a directory's own, not its contents; 0 on APFS).
    pub allocated: u64,
    /// iCloud placeholder (see `is_dataless`).
    pub dataless: bool,
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
#[cfg_attr(not(unix), allow(unused_imports))]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn clones_share_their_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c) = (dir.path().join("a.bin"), dir.path().join("b.bin"), dir.path().join("c.bin"));
        let data = vec![42u8; 2 * 1024 * 1024];
        fs::write(&a, &data).unwrap();
        // A real copy: write the bytes again (std::fs::copy clones on APFS).
        fs::write(&c, &data).unwrap();
        let ok = std::process::Command::new("/bin/cp").arg("-c").arg(&a).arg(&b).status().unwrap();
        assert!(ok.success());
        let (pa, pb, pc) = (physical_offset(&a), physical_offset(&b), physical_offset(&c));
        assert!(pa.is_some());
        assert_eq!(pa, pb, "a clone shares its blocks");
        assert_ne!(pa, pc, "a byte copy has its own blocks");
    }

    #[test]
    #[cfg(unix)]
    fn kills_commands_that_overrun() {
        let t = std::time::Instant::now();
        assert_eq!(run_with_timeout("/bin/sleep", &["5"], std::time::Duration::from_millis(300)), None);
        assert!(t.elapsed().as_secs() < 2);
        assert_eq!(run_with_timeout("/bin/echo", &["hi"], std::time::Duration::from_secs(5)).as_deref(), Some("hi\n"));
    }

    #[test]
    #[cfg(target_os = "macos")]
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
