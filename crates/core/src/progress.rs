use parking_lot::Mutex;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Shared between a running scan and whoever is watching it: carries cancellation
/// and cheap progress counters that a UI thread can sample at its own pace.
#[derive(Default)]
pub struct ScanControl {
    cancelled: AtomicBool,
    items: AtomicU64,
    bytes: AtomicU64,
    current: Mutex<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub items: u64,
    pub bytes: u64,
    pub current: String,
}

impl ScanControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn add(&self, items: u64, bytes: u64) {
        self.items.fetch_add(items, Ordering::Relaxed);
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Best effort: skipped when another thread holds the lock, since this is only
    /// for display and must never slow the walk down.
    pub fn set_current(&self, path: &Path) {
        if let Some(mut cur) = self.current.try_lock() {
            cur.clear();
            cur.push_str(&path.to_string_lossy());
        }
    }

    pub fn snapshot(&self) -> Progress {
        Progress {
            items: self.items.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            current: self.current.lock().clone(),
        }
    }
}
