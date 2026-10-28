//! PenCleaner engine: everything that touches the filesystem lives here, free of any
//! Tauri dependency so it can be unit-tested and driven from a CLI.

pub mod activity;
pub mod applications;
pub mod apps;
pub mod cleanup;
pub mod clutter;
pub mod error;
pub mod fsutil;
pub mod imaging;
pub mod performance;
pub mod permissions;
pub mod privileged;
pub mod protection;
pub mod progress;
pub mod remove;
pub mod safety;
pub mod space_lens;
pub mod spotlight;
pub mod updater;
pub mod volumes;

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
