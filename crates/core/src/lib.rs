//! PenCleaner engine: everything that touches the filesystem lives here, free of any
//! Tauri dependency so it can be unit-tested and driven from a CLI.

// Shared by every platform.
pub mod activity;
pub mod care;
pub mod cleanup;
pub mod clutter;
pub mod error;
pub mod fsutil;
pub mod progress;
pub mod remove;
pub mod safety;
pub mod settings;
pub mod space_lens;
pub mod volumes;

// macOS only: app bundles, launchd, codesign, XProtect, TCC, Time Machine, IOKit.
#[cfg(target_os = "macos")]
pub mod applications;
#[cfg(target_os = "macos")]
pub mod performance;
#[cfg(target_os = "macos")]
pub mod privileged;
#[cfg(target_os = "macos")]
pub mod protection;
#[cfg(target_os = "macos")]
pub mod sensors;
#[cfg(target_os = "macos")]
pub mod updater;
#[cfg(target_os = "macos")]
pub mod vitals;

// Used by shared code; Windows has stand-ins with the same names (src/windows/).
#[cfg(target_os = "macos")]
pub mod apps;
#[cfg(windows)]
#[path = "windows/apps.rs"]
pub mod apps;
#[cfg(target_os = "macos")]
pub mod imaging;
#[cfg(windows)]
#[path = "windows/imaging.rs"]
pub mod imaging;
#[cfg(target_os = "macos")]
pub mod permissions;
#[cfg(windows)]
#[path = "windows/permissions.rs"]
pub mod permissions;
#[cfg(target_os = "macos")]
pub mod spotlight;
#[cfg(windows)]
#[path = "windows/spotlight.rs"]
pub mod spotlight;

pub use error::{Error, Result};

/// Decimal sizes like Finder: "5.9 GB", "412 MB".
pub fn bytes_label(n: u64) -> String {
    const UNITS: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];
    if n < 1000 {
        return format!("{n} bytes");
    }
    let mut v = n as f64;
    let mut i = 0;
    v /= 1000.0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if v >= 100.0 { format!("{v:.0} {}", UNITS[i]) } else { format!("{v:.1} {}", UNITS[i]) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_bytes() {
        assert_eq!(super::bytes_label(5_884_500_000), "5.9 GB");
        assert_eq!(super::bytes_label(412_000_000), "412 MB");
        assert_eq!(super::bytes_label(999), "999 bytes");
    }
}
