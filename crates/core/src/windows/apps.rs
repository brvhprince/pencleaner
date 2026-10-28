//! Windows stand-in for `apps`: what Cleanup asks of installed apps. Bundle ids are a
//! macOS idea; the Windows uninstaller (registry-based) will fill these in.

use std::collections::{HashMap, HashSet};

/// Display names by app id.
#[derive(Default)]
pub struct AppIndex {
    names: HashMap<String, String>,
}

impl AppIndex {
    pub fn load() -> Self {
        Self::default()
    }

    pub fn name(&self, id: &str) -> Option<&str> {
        self.names.get(&id.to_ascii_lowercase()).map(String::as_str)
    }
}

/// Running programs by executable name, lower-case ("chrome.exe"), so their caches
/// aren't pulled out from under them.
pub fn running_bundle_ids() -> HashSet<String> {
    let Some(out) = crate::fsutil::run_with_timeout("tasklist", &["/fo", "csv", "/nh"], std::time::Duration::from_secs(10)) else {
        return HashSet::new();
    };
    // "chrome.exe","1234","Console","1","123,456 K"
    out.lines().filter_map(|l| l.split(',').next()).map(|n| n.trim_matches('"').to_ascii_lowercase()).filter(|n| !n.is_empty()).collect()
}
