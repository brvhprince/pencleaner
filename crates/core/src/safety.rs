//! The last line of defence: every removal goes through `protection` regardless of
//! which module proposed it.

use std::path::{Component, Path};

/// Checks every platform shares: absolute and normalised.
fn basics(path: &Path) -> Option<&'static str> {
    if !path.is_absolute() {
        return Some("Only absolute paths can be removed");
    }
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Some("Path is not normalized");
    }
    None
}

/// Folders in a Windows profile that Windows creates and apps expect to exist.
#[cfg(windows)]
const WIN_HOME_STANDARD: &[&str] = &[
    "AppData", "Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos", "OneDrive", "Favorites", "Contacts", "Links",
    "Saved Games", "Searches", "3D Objects",
];

/// Trees under the system drive where nothing should ever be removed by a cleaner.
#[cfg(windows)]
const WIN_SYSTEM_TREES: &[&str] = &[
    "Windows", "Program Files", "Program Files (x86)", "ProgramData", "Recovery", "System Volume Information", "$Recycle.Bin", "Boot",
];

/// Windows system junk whose *contents* may be removed (with administrator rights), the
/// exceptions inside `WIN_SYSTEM_TREES`. Relative to %SystemRoot% or %ProgramData%.
#[cfg(windows)]
const WIN_ADMIN_TREES: &[(&str, &str)] = &[
    ("SystemRoot", r"SoftwareDistribution\Download"),
    ("SystemRoot", r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache"),
    ("SystemRoot", "Temp"),
    ("SystemRoot", "Minidump"),
    ("ProgramData", r"Microsoft\Windows\WER\ReportQueue"),
    ("ProgramData", r"Microsoft\Windows\WER\ReportArchive"),
];

/// Single system files that may be removed with administrator rights.
#[cfg(windows)]
const WIN_ADMIN_FILES: &[(&str, &str)] = &[("SystemRoot", "MEMORY.DMP")];

#[cfg(windows)]
fn lower(p: &Path) -> String {
    p.to_string_lossy().to_lowercase().replace('/', "\\").trim_end_matches('\\').to_owned()
}

#[cfg(windows)]
fn env_path(var: &str, rel: &str) -> Option<String> {
    std::env::var_os(var).map(|base| lower(&Path::new(&base).join(rel)))
}

/// Inside one of the system junk folders (not the folder itself), or one of the single
/// system files: the only system paths an elevated clean may touch.
#[cfg(windows)]
pub fn admin_cleanable(path: &Path) -> bool {
    // Text comparison below: "Temp\..\System32" would otherwise start with "Temp".
    if basics(path).is_some() {
        return false;
    }
    let p = lower(path);
    WIN_ADMIN_TREES.iter().filter_map(|(v, r)| env_path(v, r)).any(|t| p.starts_with(&format!("{t}\\")))
        || WIN_ADMIN_FILES.iter().filter_map(|(v, r)| env_path(v, r)).any(|f| p == f)
}

/// Windows rules: system folders, every profile, and the profile's own skeleton. Paths
/// compare case-insensitively, as Windows does.
#[cfg(windows)]
pub fn protection(path: &Path, home: &Path) -> Option<&'static str> {
    if let Some(why) = basics(path) {
        return Some(why);
    }
    if admin_cleanable(path) {
        return None;
    }
    let (p, h) = (lower(path), lower(home));
    let parts: Vec<&str> = p.trim_end_matches('\\').split('\\').collect();
    // ["c:", "windows", …]: a drive root, or anything directly on it, is off limits.
    if parts.len() <= 2 {
        return Some("Top-level system folder");
    }
    if WIN_SYSTEM_TREES.iter().any(|t| parts[1] == t.to_lowercase()) {
        return Some("Part of Windows");
    }
    if parts[1] == "users" && parts.len() == 3 {
        return Some("User profile folder");
    }
    if p.trim_end_matches('\\') == h.trim_end_matches('\\') {
        return Some("Your profile folder");
    }
    if let Some(rel) = p.strip_prefix(&format!("{}\\", h.trim_end_matches('\\'))) {
        let rel: Vec<&str> = rel.split('\\').filter(|s| !s.is_empty()).collect();
        if rel.len() == 1 && WIN_HOME_STANDARD.iter().any(|s| rel[0] == s.to_lowercase()) {
            return Some("Standard Windows folder");
        }
        // AppData, AppData\Local, and AppData\Local\<app> as a whole.
        if rel.first() == Some(&"appdata") && rel.len() <= 3 {
            return Some("AppData folder used by Windows and apps");
        }
    }
    None
}

/// Folders in a home directory that macOS creates and other apps expect to exist.
#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
const SYSTEM_ALLOWED: &[&str] = &["/usr/local"];

/// Returns why `path` must not be removed, or None when it may be.
#[cfg(target_os = "macos")]
pub fn protection(path: &Path, home: &Path) -> Option<&'static str> {
    if let Some(why) = basics(path) {
        return Some(why);
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
    #[cfg(target_os = "macos")]
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
    #[cfg(target_os = "macos")]
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

    #[test]
    #[cfg(windows)]
    fn windows_protects_system_and_profile_skeleton() {
        let home = p(r"C:\Users\me");
        for bad in [
            r"C:\",
            r"C:\Windows",
            r"C:\Windows\System32\drivers",
            r"c:\program files\App",
            r"C:\ProgramData\Microsoft",
            r"C:\Users",
            r"C:\Users\other",
            r"C:\Users\me",
            r"C:\Users\me\Documents",
            r"C:\Users\me\AppData\Local",
            r"C:\Users\me\AppData\Local\Google",
            r"D:\Games",
            r"relative\path",
        ] {
            assert!(protection(p(bad), home).is_some(), "{bad} should be protected");
        }
    }

    #[test]
    #[cfg(windows)]
    fn windows_allows_ordinary_content() {
        let home = p(r"C:\Users\me");
        for ok in [
            r"C:\Users\me\AppData\Local\Google\Chrome\User Data\Default\Cache\data_0",
            r"C:\Users\me\AppData\Local\Temp\setup123",
            r"C:\Users\me\Documents\old.zip",
            r"C:\Users\me\code\app\node_modules",
            r"D:\Games\old-mod",
        ] {
            assert!(protection(p(ok), home).is_none(), "{ok} should be removable");
        }
    }

    #[test]
    #[cfg(windows)]
    fn windows_admin_cleanup_is_narrow() {
        let win = std::env::var("SystemRoot").expect("SystemRoot");
        let data = std::env::var("ProgramData").expect("ProgramData");
        let at = |base: &str, rel: &str| std::path::PathBuf::from(format!(r"{base}\{rel}"));
        // Contents of the junk folders, and the memory dump: yes.
        for ok in [
            at(&win, r"SoftwareDistribution\Download\a1b2c3"),
            at(&win, r"Temp\setup.log"),
            at(&win, r"Minidump\102826-1.dmp"),
            at(&win, "MEMORY.DMP"),
            at(&data, r"Microsoft\Windows\WER\ReportArchive\Kernel_1"),
        ] {
            assert!(admin_cleanable(&ok), "{} should be cleanable", ok.display());
            assert!(protection(&ok, p(r"C:\Users\me")).is_none(), "{} should pass the gate", ok.display());
        }
        // The folders themselves, and anything else in Windows: no.
        for bad in [
            at(&win, r"SoftwareDistribution\Download"),
            at(&win, "Temp"),
            at(&win, r"System32\drivers\etc\hosts"),
            at(&win, r"SoftwareDistribution\DataStore\DataStore.edb"),
            at(&win, r"Temp\..\System32"),
            at(&data, r"Microsoft\Windows\WER"),
            p(r"C:\Program Files\App\cache").to_path_buf(),
        ] {
            assert!(!admin_cleanable(&bad), "{} must not be cleanable", bad.display());
            assert!(protection(&bad, p(r"C:\Users\me")).is_some(), "{} must stay protected", bad.display());
        }
    }
}
