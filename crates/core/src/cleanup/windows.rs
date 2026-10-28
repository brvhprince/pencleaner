//! Cleanup rules for Windows: app and browser caches, logs and crash reports, temporary
//! files and the Recycle Bin. Same group ids as on the Mac, so Smart Care and its
//! schedule treat them the same way. Nothing here needs administrator rights.

use super::*;

fn local() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

fn roaming() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// Chromium's cache folders inside a profile (or an Electron app's data folder).
const CHROMIUM_CACHES: &[&str] = &["Cache", "Code Cache", "GPUCache", "Service Worker\\CacheStorage", "CachedData"];

/// (folder under %LOCALAPPDATA%, name, process) for Chromium browsers.
const BROWSERS: &[(&str, &str, &str)] = &[
    ("Google\\Chrome\\User Data", "Chrome", "chrome.exe"),
    ("Microsoft\\Edge\\User Data", "Edge", "msedge.exe"),
    ("BraveSoftware\\Brave-Browser\\User Data", "Brave", "brave.exe"),
    ("Vivaldi\\User Data", "Vivaldi", "vivaldi.exe"),
];

/// (folder under %APPDATA%, name, process) for Electron apps.
const ELECTRON_APPS: &[(&str, &str, &str)] = &[
    ("Code", "Visual Studio Code", "code.exe"),
    ("Slack", "Slack", "slack.exe"),
    ("discord", "Discord", "discord.exe"),
    ("Microsoft\\Teams", "Teams (classic)", "teams.exe"),
    ("Notion", "Notion", "notion.exe"),
    ("Figma", "Figma", "figma.exe"),
];

/// (folder under %LOCALAPPDATA%, name) for caches that rebuild on their own.
const OTHER_CACHES: &[(&str, &str)] = &[
    ("D3DSCache", "DirectX shader cache"),
    ("NVIDIA\\DXCache", "NVIDIA shader cache"),
    ("NVIDIA\\GLCache", "NVIDIA OpenGL cache"),
    ("AMD\\DxCache", "AMD shader cache"),
    ("Microsoft\\Windows\\INetCache", "Internet cache"),
    ("Spotify\\Data", "Spotify cache"),
];

/// One item covering several cache folders of an app, measured together.
fn app_item(cx: &Cx, id: String, name: String, dirs: Vec<PathBuf>, running: bool) -> Option<Item> {
    let dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| d.exists()).collect();
    if dirs.is_empty() || cx.ctl.is_cancelled() {
        return None;
    }
    let size: u64 = dirs.iter().map(|d| fsutil::disk_usage(d)).sum();
    cx.ctl.add(dirs.len() as u64, size);
    if size < MIN_ITEM {
        return None;
    }
    // Empty the folders but keep them: apps (and the safety rules) expect them to exist.
    let contents: Vec<PathBuf> = dirs.iter().flat_map(|d| children(d)).collect();
    Some(Item {
        detail: Some(cx.tilde(&dirs[0])),
        path: Some(dirs[0].to_string_lossy().into_owned()),
        last_activity: dirs.iter().filter_map(|d| mtime_of(d)).max(),
        id,
        name,
        size,
        kind: ItemKind::Folder,
        mode: RemoveMode::Delete,
        // A running app holds its cache open: Windows won't let much of it go.
        recommended: !running,
        warning: running.then(|| "It's running; quit it first to free all of this.".to_owned()),
        action: Action::Paths(contents),
    })
}

pub fn user_caches(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();
    if let Some(local) = local() {
        for (rel, name, exe) in BROWSERS {
            let data = local.join(rel);
            // Every profile ("Default", "Profile 1", …) has its own caches.
            let dirs = children(&data)
                .into_iter()
                .filter(|p| p.join("Preferences").exists())
                .flat_map(|profile| CHROMIUM_CACHES.iter().map(move |c| profile.join(c)))
                .collect();
            items.extend(app_item(cx, data.to_string_lossy().into_owned(), format!("{name} cache"), dirs, cx.is_running(exe)));
        }
        let firefox = local.join("Mozilla\\Firefox\\Profiles");
        let dirs = children(&firefox).into_iter().map(|p| p.join("cache2")).collect();
        items.extend(app_item(cx, firefox.to_string_lossy().into_owned(), "Firefox cache".into(), dirs, cx.is_running("firefox.exe")));
        for (rel, name) in OTHER_CACHES {
            let dir = local.join(rel);
            items.extend(app_item(cx, dir.to_string_lossy().into_owned(), (*name).into(), vec![dir], false));
        }
    }
    if let Some(roaming) = roaming() {
        for (rel, name, exe) in ELECTRON_APPS {
            let data = roaming.join(rel);
            let dirs = CHROMIUM_CACHES.iter().map(|c| data.join(c)).collect();
            items.extend(app_item(cx, data.to_string_lossy().into_owned(), format!("{name} cache"), dirs, cx.is_running(exe)));
        }
    }
    vec![group("user-caches", Category::System, "App & Browser Caches", "Files apps and browsers keep to load faster. They rebuild themselves as needed.", items)]
}

pub fn logs(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();
    if let Some(local) = local() {
        let spots = [
            ("CrashDumps", "Crash dumps"),
            ("Microsoft\\Windows\\WER\\ReportArchive", "Sent error reports"),
            ("Microsoft\\Windows\\WER\\ReportQueue", "Unsent error reports"),
        ];
        for (rel, name) in spots {
            let dir = local.join(rel);
            items.extend(app_item(cx, dir.to_string_lossy().into_owned(), name.into(), vec![dir], false));
        }
    }
    vec![group("user-logs", Category::System, "Logs & Crash Reports", "Crash dumps and error reports Windows and apps keep after something goes wrong.", items)]
}

pub fn temporary(cx: &Cx) -> Vec<Group> {
    let items = system::old_temp_files(cx).into_iter().collect();
    vec![group("temporary", Category::System, "Temporary Files", "Leftovers from apps and installers that didn't clean up after themselves.", items)]
}

/// The Recycle Bin on every drive, as Windows itself counts it.
pub fn recycle_bin(_cx: &Cx) -> Vec<Group> {
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, i64Size: 0, i64NumItems: 0 };
    // A null root means "all drives".
    let ok = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) } == 0;
    let mut items = Vec::new();
    if ok && info.i64Size as u64 >= MIN_ITEM {
        let n = info.i64NumItems.max(0) as u64;
        items.push(Item {
            id: "recycle-bin".into(),
            name: "Recycle Bin".into(),
            detail: Some(if n == 1 { "1 item".into() } else { format!("{n} items") }),
            path: None,
            size: info.i64Size as u64,
            kind: ItemKind::Trash,
            mode: RemoveMode::Delete,
            last_activity: None,
            recommended: false,
            warning: Some("Emptying the Recycle Bin can't be undone.".into()),
            action: Action::Command(
                ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "Clear-RecycleBin -Force -ErrorAction SilentlyContinue"]
                    .map(String::from)
                    .to_vec(),
            ),
        });
    }
    vec![group("trash", Category::Trash, "Recycle Bin", "Everything you've deleted on this PC's drives.", items)]
}
