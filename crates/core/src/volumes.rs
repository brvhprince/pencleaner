use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub free: u64,
    pub used: u64,
    pub is_root: bool,
    pub fs_type: String,
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    struct Stat {
        total: u64,
        free: u64,
        mount: String,
        fs_type: String,
    }

    fn statfs(path: &Path) -> Option<Stat> {
        let c = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
            return None;
        }
        let bsize = s.f_bsize as u64;
        let cstr = |b: &[libc::c_char]| unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().into_owned();
        Some(Stat {
            // For APFS this is the whole container, which is what "x of 1 TB used" means.
            total: s.f_blocks * bsize,
            free: s.f_bavail * bsize,
            mount: cstr(&s.f_mntonname),
            fs_type: cstr(&s.f_fstypename),
        })
    }

    /// The startup volume first, then anything mounted under /Volumes.
    pub fn list() -> Vec<Volume> {
        let mut root_name = String::from("Macintosh HD");
        let mut others = Vec::new();
        if let Ok(rd) = fs::read_dir("/Volumes") {
            for e in rd.flatten() {
                let path = e.path();
                let name = e.file_name().to_string_lossy().into_owned();
                // /Volumes/<Startup Disk> is a symlink back to "/".
                if fs::read_link(&path).map(|t| t == Path::new("/")).unwrap_or(false) {
                    root_name = name;
                    continue;
                }
                if let Some(st) = statfs(&path) {
                    if st.mount == path.to_string_lossy() {
                        others.push(volume(name, path, st, false));
                    }
                }
            }
        }
        let mut out = Vec::new();
        if let Some(st) = statfs(Path::new("/")) {
            out.push(volume(root_name, PathBuf::from("/"), st, true));
        }
        others.sort_by(|a, b| a.name.cmp(&b.name));
        out.extend(others);
        out
    }

    fn volume(name: String, mount: PathBuf, st: Stat, is_root: bool) -> Volume {
        Volume {
            name,
            mount: mount.to_string_lossy().into_owned(),
            used: st.total.saturating_sub(st.free),
            total: st.total,
            free: st.free,
            is_root,
            fs_type: st.fs_type,
        }
    }
}
#[cfg(target_os = "macos")]
pub use macos::list;

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW};

    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOVABLE: u32 = 2;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    /// Local drives (fixed and removable), the system drive first.
    pub fn list() -> Vec<Volume> {
        let system = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_ascii_uppercase();
        let mask = unsafe { GetLogicalDrives() };
        let mut out = Vec::new();
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = (b'A' + i as u8) as char;
            let root = format!("{letter}:\\");
            let w = wide(&root);
            let kind = unsafe { GetDriveTypeW(w.as_ptr()) };
            if kind != DRIVE_FIXED && kind != DRIVE_REMOVABLE {
                continue;
            }
            let (mut free, mut total, mut _all_free) = (0u64, 0u64, 0u64);
            if unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, &mut _all_free) } == 0 {
                continue;
            }
            let mut label = [0u16; 261];
            let mut fs_name = [0u16; 261];
            let ok = unsafe {
                GetVolumeInformationW(w.as_ptr(), label.as_mut_ptr(), 261, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), fs_name.as_mut_ptr(), 261)
            } != 0;
            let text = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())]);
            let label = if ok { text(&label) } else { String::new() };
            let is_root = format!("{letter}:") == system;
            let name = if label.is_empty() { if is_root { "Local Disk".to_owned() } else { format!("Drive {letter}") } } else { label };
            out.push(Volume {
                name: format!("{name} ({letter}:)"),
                mount: root,
                used: total.saturating_sub(free),
                total,
                free,
                is_root,
                fs_type: if ok { text(&fs_name) } else { String::new() },
            });
        }
        out.sort_by_key(|v| !v.is_root);
        out
    }
}
#[cfg(windows)]
pub use windows::list;

#[cfg(test)]
mod tests {
    #[test]
    fn startup_volume_is_first() {
        let v = super::list();
        assert!(v[0].is_root);
        assert!(v[0].total > 0 && v[0].used <= v[0].total);
    }
}
