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

/// Ids of running apps, so their caches aren't pulled out from under them.
pub fn running_bundle_ids() -> HashSet<String> {
    HashSet::new()
}
