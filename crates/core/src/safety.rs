//! The last line of defence: every removal goes through `protection` regardless of
//! which module proposed it.

use std::path::{Component, Path};

/// Folders in a home directory that macOS creates and other apps expect to exist.
const HOME_STANDARD: &[&str] = &[
    "Applications",
    "Desktop",
    "Documents",
    "Downloads",
    "Library",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Sites",
];

/// Trees where nothing should ever be removed by a cleaner.
const SYSTEM_TREES: &[&str] = &[
    "/System",
    "/bin",
    "/sbin",
    "/usr",
    "/dev",
    "/private/etc",
    "/private/var/db",
    "/private/var/root",
    "/Library/Apple",
    "/Library/Application Support/com.apple.TCC",
];

/// Exceptions inside `SYSTEM_TREES`.
const SYSTEM_ALLOWED: &[&str] = &["/usr/local"];

/// Returns why `path` must not be removed, or None when it may be.
pub fn protection(path: &Path, home: &Path) -> Option<&'static str> {
    if !path.is_absolute() {
        return Some("Only absolute paths can be removed");
    }
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Some("Path is not normalized");
    }
    let depth = path.components().count(); // "/" counts as one component
    if depth <= 2 {
        return Some("Top-level system folder");
    }
    let inside_allowed = SYSTEM_ALLOWED
        .iter()
        .any(|a| path.starts_with(a) && path != Path::new(a));
    if !inside_allowed && SYSTEM_TREES.iter().any(|t| path.starts_with(t)) {
        return Some("Part of macOS");
    }
    if path.parent() == Some(Path::new("/Users")) {
        return Some("User home folder");
    }
    if path == home {
        return Some("Your home folder");
    }
    if let Ok(rel) = path.strip_prefix(home) {
        let parts: Vec<_> = rel.components().collect();
        if parts.len() == 1 && HOME_STANDARD.iter().any(|s| rel == Path::new(s)) {
            return Some("Standard macOS folder");
        }
        // ~/Library/<anything> as a whole (Caches, Application Support, Preferences…).
        if parts.len() == 2 && rel.starts_with("Library") {
            return Some("Library folder used by macOS and apps");
        }
        if rel.starts_with("Library/Mobile Documents") && parts.len() <= 3 {
            return Some("iCloud Drive folder");
        }
    }
    if path.starts_with("/Library") && depth <= 3 {
        return Some("Library folder used by macOS and apps");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> &Path {
        Path::new(s)
    }

    #[test]
    fn protects_system_and_home_roots() {
        let home = p("/Users/me");
        for bad in [
            "/",
            "/Applications",
            "/Users",
            "/Users/me",
            "/Users/other",
            "/Users/me/Library",
            "/Users/me/Library/Caches",
            "/Users/me/Documents",
            "/System/Library/Fonts",
            "/usr/bin/true",
            "/usr/local",
            "/Library/Caches",
            "relative/path",
            "/Users/me/../other/x",
        ] {
            assert!(protection(p(bad), home).is_some(), "{bad} should be protected");
        }
    }

    #[test]
    fn allows_ordinary_content() {
        let home = p("/Users/me");
        for ok in [
            "/Users/me/.colima",
            "/Users/me/Library/Caches/com.foo.bar",
            "/Users/me/Documents/old.zip",
            "/Users/me/Sites/js/app/node_modules",
            "/Applications/Xcode.app",
            "/usr/local/share/dotnet",
            "/Library/Caches/com.foo",
        ] {
            assert!(protection(p(ok), home).is_none(), "{ok} should be removable");
        }
    }
}
