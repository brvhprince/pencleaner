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

/// Where a file's data starts on the physical device. APFS clones (what Finder makes when
/// you duplicate a file, and what pnpm/Bun use to install packages) share their blocks, so
/// two copies reporting the same address free nothing when one is deleted.
/// (ATTR_CMNEXT_PRIVATESIZE would be the direct answer, but it reports 0 for files written
/// before clone tracking existed on the volume, so it can't be trusted.)
pub fn physical_offset(path: &Path) -> Option<u64> {
    use std::os::unix::io::AsRawFd;

    // <sys/fcntl.h>, #pragma pack(4)
    #[repr(C, packed(4))]
    struct Log2Phys {
        flags: u32,
        contigbytes: i64,
        devoffset: i64,
    }
    const F_LOG2PHYS_EXT: libc::c_int = 65;

    let f = fs::File::open(path).ok()?;
    let mut l = Log2Phys { flags: 0, contigbytes: 1, devoffset: 0 };
    let r = unsafe { libc::fcntl(f.as_raw_fd(), F_LOG2PHYS_EXT, &mut l as *mut Log2Phys) };
    let off = l.devoffset;
    (r != -1 && off > 0).then_some(off as u64)
}

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
    fn kills_commands_that_overrun() {
        let t = std::time::Instant::now();
        assert_eq!(run_with_timeout("/bin/sleep", &["5"], std::time::Duration::from_millis(300)), None);
        assert!(t.elapsed().as_secs() < 2);
        assert_eq!(run_with_timeout("/bin/echo", &["hi"], std::time::Duration::from_secs(5)).as_deref(), Some("hi\n"));
    }

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
