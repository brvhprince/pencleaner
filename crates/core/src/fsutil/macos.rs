//! macOS specifics of `fsutil`: APFS allocation, iCloud placeholders, clones, bulk reads.

use super::BulkEntry;
use std::fs::{self, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// (device, inode, link count): what makes two hard links the same file.
pub fn link_identity(md: &Metadata) -> (u64, u64, u64) {
    (md.dev(), md.ino(), md.nlink())
}

/// Which volume an entry lives on, so scans don't wander onto other disks.
pub fn device(md: &Metadata) -> u64 {
    md.dev()
}

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

/// Lists a directory with `getattrlistbulk`: names, types and sizes for many entries per
/// system call, instead of a `stat` per entry. Doesn't follow symlinks. Errors mean the
/// directory couldn't be read at all.
pub fn read_dir_bulk(dir: &Path, mut each: impl FnMut(BulkEntry)) -> std::io::Result<()> {
    use std::ffi::{CString, OsStr};
    use std::os::unix::ffi::OsStrExt;

    const VDIR: u32 = 2;
    // <sys/attr.h>; not exposed by the libc crate. Comes right after the returned set.
    const ATTR_CMN_ERROR: u32 = 0x2000_0000;
    let c = CString::new(dir.as_os_str().as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    struct Fd(i32);
    impl Drop for Fd {
        fn drop(&mut self) {
            unsafe { libc::close(self.0) };
        }
    }
    let _guard = Fd(fd);

    let mut attrs = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: libc::ATTR_CMN_RETURNED_ATTRS
            | libc::ATTR_CMN_NAME
            | ATTR_CMN_ERROR
            | libc::ATTR_CMN_DEVID
            | libc::ATTR_CMN_OBJTYPE
            | libc::ATTR_CMN_FLAGS
            | libc::ATTR_CMN_FILEID,
        volattr: 0,
        dirattr: libc::ATTR_DIR_ALLOCSIZE,
        fileattr: libc::ATTR_FILE_LINKCOUNT | libc::ATTR_FILE_ALLOCSIZE,
        forkattr: 0,
    };
    thread_local! {
        static BUF: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; 256 * 1024]);
    }
    BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        loop {
            let n = unsafe {
                libc::getattrlistbulk(fd, (&mut attrs as *mut libc::attrlist).cast(), buf.as_mut_ptr().cast(), buf.len(), 0)
            };
            if n < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if n == 0 {
                return Ok(());
            }
            // Entries are packed one after another; within one, attributes follow the
            // returned set in a fixed order, 4-byte aligned (man getattrlistbulk).
            let mut entry = 0usize;
            for _ in 0..n {
                let base = entry;
                let rd_u32 = |at: usize| u32::from_ne_bytes(buf[at..at + 4].try_into().unwrap());
                let rd_u64 = |at: usize| u64::from_ne_bytes(buf[at..at + 8].try_into().unwrap());
                let len = rd_u32(base) as usize;
                entry += len;
                let mut at = base + 4;
                // attribute_set_t: common, vol, dir, file, fork.
                let returned_common = rd_u32(at);
                let returned_dir = rd_u32(at + 8);
                let returned_file = rd_u32(at + 12);
                at += std::mem::size_of::<libc::attribute_set_t>();
                if returned_common & ATTR_CMN_ERROR != 0 {
                    let err = rd_u32(at);
                    at += 4;
                    if err != 0 {
                        continue; // this one entry couldn't be read; skip it like a failed stat
                    }
                }
                let mut name: &OsStr = OsStr::new("");
                if returned_common & libc::ATTR_CMN_NAME != 0 {
                    let off = i32::from_ne_bytes(buf[at..at + 4].try_into().unwrap()) as isize;
                    let nlen = rd_u32(at + 4) as usize;
                    let start = (at as isize + off) as usize;
                    // Length includes the trailing NUL.
                    name = OsStr::from_bytes(&buf[start..start + nlen.saturating_sub(1)]);
                    at += 8;
                }
                let mut dev = 0u64;
                if returned_common & libc::ATTR_CMN_DEVID != 0 {
                    // Same widening as std's MetadataExt::dev (st_dev is an i32).
                    dev = i32::from_ne_bytes(buf[at..at + 4].try_into().unwrap()) as u64;
                    at += 4;
                }
                let mut objtype = 0u32;
                if returned_common & libc::ATTR_CMN_OBJTYPE != 0 {
                    objtype = rd_u32(at);
                    at += 4;
                }
                let mut flags = 0u32;
                if returned_common & libc::ATTR_CMN_FLAGS != 0 {
                    flags = rd_u32(at);
                    at += 4;
                }
                let mut ino = 0u64;
                if returned_common & libc::ATTR_CMN_FILEID != 0 {
                    ino = rd_u64(at);
                    at += 8;
                }
                let mut allocated = 0u64;
                if returned_dir & libc::ATTR_DIR_ALLOCSIZE != 0 {
                    allocated = rd_u64(at);
                    at += 8;
                }
                let mut nlink = 1u64;
                if returned_file & libc::ATTR_FILE_LINKCOUNT != 0 {
                    nlink = rd_u32(at) as u64;
                    at += 4;
                }
                if returned_file & libc::ATTR_FILE_ALLOCSIZE != 0 {
                    allocated = rd_u64(at);
                }
                each(BulkEntry { name, is_dir: objtype == VDIR, dev, ino, nlink, allocated, dataless: flags & SF_DATALESS != 0 });
            }
        }
    })
}

