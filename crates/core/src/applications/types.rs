//! The Applications report, shared by the macOS and Windows engines so the screens work
//! the same on both.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedFile {
    pub path: String,
    /// Where it lives, e.g. "Preferences", "Launch Daemon".
    pub label: &'static str,
    pub size: u64,
    /// Needs the administrator password to remove.
    pub admin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    /// The bundle path: unique even when two copies share a bundle id
    /// (e.g. IDLE.app in two Python versions).
    pub id: String,
    pub bundle_id: String,
    pub name: String,
    pub version: Option<String>,
    pub path: String,
    pub size: u64,
    pub related: Vec<RelatedFile>,
    pub related_size: u64,
    pub last_used: Option<i64>,
    pub app_store: bool,
    /// Who made it, when the system says (Windows' installed-programs list).
    pub publisher: Option<String>,
    /// When it was installed (Windows); the Mac shows last use instead.
    pub installed: Option<i64>,
    pub running: bool,
    /// Another installed copy shares this bundle id, so its support files stay put.
    pub shares_support_files: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub id: String,
    /// The same vendor still has apps installed; these may be shared, so check first.
    pub vendor_installed: bool,
    pub files: Vec<RelatedFile>,
    pub size: u64,
    pub last_activity: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installer {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub added: Option<i64>,
    /// An installed app this image appears to be for.
    pub installed_app: Option<String>,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsReport {
    pub apps: Vec<AppEntry>,
    pub leftovers: Vec<Leftover>,
    pub installers: Vec<Installer>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "id")]
pub enum Target {
    App(String),
    Leftover(String),
    Installer(String),
}

impl AppsReport {
    /// Drops apps you've hidden, ignored leftovers (or their files) and installers.
    pub fn apply_ignore(&mut self, ig: &crate::settings::Ignore) {
        if ig.is_empty() {
            return;
        }
        self.apps.retain(|a| !ig.item(&format!("applications:app:{}", a.id)) && !ig.path_str(&a.path));
        for a in &mut self.apps {
            a.related.retain(|f| !ig.path_str(&f.path));
            a.related_size = a.related.iter().map(|f| f.size).sum();
        }
        self.leftovers.retain_mut(|l| {
            if ig.item(&format!("applications:leftover:{}", l.id)) {
                return false;
            }
            l.files.retain(|f| !ig.path_str(&f.path));
            l.size = l.files.iter().map(|f| f.size).sum();
            !l.files.is_empty()
        });
        self.installers.retain(|i| !ig.path_str(&i.path) && !ig.item(&format!("applications:installer:{}", i.path)));
    }
}
