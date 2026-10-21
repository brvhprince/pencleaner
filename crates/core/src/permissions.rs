use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::fsutil::home_dir;

pub const FULL_DISK_ACCESS_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// macOS has no API to ask directly, so probe locations that TCC guards and only a
/// process with Full Disk Access can read. The first probe that exists decides.
pub fn has_full_disk_access() -> bool {
    let home = home_dir();
    let probes: [PathBuf; 3] = [
        PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
        home.join("Library/Safari"),
        home.join("Library/Containers/com.apple.stocks"),
    ];
    for p in probes {
        let result = if p.is_dir() {
            fs::read_dir(&p).map(|_| ())
        } else {
            fs::File::open(&p).map(|_| ())
        };
        match result {
            Ok(()) => return true,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(_) => return false,
        }
    }
    false
}
