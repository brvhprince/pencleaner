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
        admin: false,
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
    let (bytes, n) = crate::windows::recycle_bin::size();
    let mut items = Vec::new();
    if bytes >= MIN_ITEM {
        items.push(Item {
            id: "recycle-bin".into(),
            name: "Recycle Bin".into(),
            detail: Some(if n == 1 { "1 item".into() } else { format!("{n} items") }),
            path: None,
            size: bytes,
            kind: ItemKind::Trash,
            mode: RemoveMode::Delete,
            last_activity: None,
            recommended: false,
            warning: Some("Emptying the Recycle Bin can't be undone.".into()),
            admin: false,
            action: Action::Command(
                ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "Clear-RecycleBin -Force -ErrorAction SilentlyContinue"]
                    .map(String::from)
                    .to_vec(),
            ),
        });
    }
    vec![group("trash", Category::Trash, "Recycle Bin", "Everything you've deleted on this PC's drives.", items)]
}

/// One kind of system junk: folders whose contents go.
struct Spot {
    name: &'static str,
    dirs: Vec<PathBuf>,
    recommended: bool,
    warning: Option<&'static str>,
    /// Only items untouched for this long (seconds).
    older: Option<i64>,
}

/// Windows' own leftovers in system folders. Deleting them needs administrator rights:
/// one UAC prompt when you clean. Measuring some of them does too, so without it those
/// are left out and the group says so.
pub fn system_files(cx: &Cx) -> Vec<Group> {
    let root = |var: &str| std::env::var_os(var).map(PathBuf::from);
    let (Some(win), Some(data)) = (root("SystemRoot"), root("ProgramData")) else { return Vec::new() };
    let elevated = crate::windows::elevate::is_elevated();
    let mut items = Vec::new();
    let mut unmeasured = 0;

    let spots = [
        Spot { name: "Windows Update downloads", dirs: vec![win.join(r"SoftwareDistribution\Download")], recommended: true, warning: None, older: None },
        Spot {
            name: "Delivery Optimization files",
            dirs: vec![win.join(r"ServiceProfiles\NetworkService\AppData\Local\Microsoft\Windows\DeliveryOptimization\Cache")],
            recommended: true,
            warning: None,
            older: None,
        },
        Spot { name: "Windows temporary files", dirs: vec![win.join("Temp")], recommended: true, warning: None, older: Some(3 * DAY) },
        Spot {
            name: "System error reports",
            dirs: vec![data.join(r"Microsoft\Windows\WER\ReportQueue"), data.join(r"Microsoft\Windows\WER\ReportArchive")],
            recommended: true,
            warning: None,
            older: None,
        },
        Spot {
            name: "System crash dumps",
            dirs: vec![win.join("Minidump")],
            recommended: false,
            warning: Some("Only useful if you're diagnosing crashes."),
            older: None,
        },
    ];
    for Spot { name, dirs, recommended, warning, older } in spots {
        let present: Vec<&PathBuf> = dirs.iter().filter(|d| d.exists()).collect();
        if present.is_empty() || cx.ctl.is_cancelled() {
            continue;
        }
        if present.iter().any(|d| fs::read_dir(d).is_err()) {
            unmeasured += 1;
            continue;
        }
        let cutoff = older.map(|age| cx.now - age);
        let paths: Vec<PathBuf> = present
            .iter()
            .flat_map(|d| children(d))
            .filter(|p| cutoff.is_none_or(|c| mtime_of(p).is_some_and(|t| t < c)))
            .collect();
        let size: u64 = paths.iter().map(|p| fsutil::disk_usage(p)).sum();
        cx.ctl.add(paths.len() as u64, size);
        if size < MIN_ITEM {
            continue;
        }
        items.push(Item {
            id: present[0].to_string_lossy().into_owned(),
            name: name.into(),
            detail: Some(present[0].display().to_string()),
            path: Some(present[0].to_string_lossy().into_owned()),
            size,
            kind: ItemKind::Folder,
            mode: RemoveMode::Delete,
            last_activity: present.iter().filter_map(|d| mtime_of(d)).max(),
            recommended,
            warning: warning.map(str::to_owned),
            admin: true,
            action: Action::Paths(paths),
        });
    }
    // The full memory dump Windows writes after a blue screen: often gigabytes.
    let dump = win.join("MEMORY.DMP");
    if let Ok(md) = fs::metadata(&dump) {
        items.push(Item {
            id: dump.to_string_lossy().into_owned(),
            name: "Full memory dump".into(),
            detail: Some(dump.display().to_string()),
            path: Some(dump.to_string_lossy().into_owned()),
            size: fsutil::allocated(&md),
            kind: ItemKind::File,
            mode: RemoveMode::Delete,
            last_activity: mtime(&md),
            recommended: false,
            warning: Some("Only useful if you're diagnosing a crash.".into()),
            admin: true,
            action: Action::Paths(vec![dump]),
        });
    }
    let mut g = group(
        "windows-system",
        Category::System,
        "Windows System Files",
        "Update downloads, temporary files and crash reports Windows keeps. Cleaning asks for administrator access once.",
        items,
    );
    g.guides = guides(&win);
    if unmeasured > 0 && !elevated {
        g.note = Some(format!(
            "{unmeasured} more system {} can only be measured with administrator access. Run PenCleaner as administrator to include {}.",
            if unmeasured == 1 { "folder" } else { "folders" },
            if unmeasured == 1 { "it" } else { "them" }
        ));
    }
    vec![g]
}

/// What only Windows' own Disk Cleanup removes properly: its files belong to the system
/// (TrustedInstaller), so even an administrator's plain delete fails partway.
fn guides(win: &Path) -> Vec<Guide> {
    let mut out = Vec::new();
    let drive = win.parent().unwrap_or(win);
    let old = drive.join("Windows.old");
    if old.exists() {
        out.push(Guide {
            id: "windows-old",
            title: "Previous Windows installation (Windows.old)",
            size: Some(fsutil::disk_usage(&old)).filter(|s| *s > 0),
            why: "Kept after a Windows upgrade so you can go back. After removing it you can't roll back to the earlier version.",
            steps: vec![
                "Open Disk Cleanup and choose the system drive.",
                "Click \u{201c}Clean up system files\u{201d} and approve the administrator prompt.",
                "Tick \u{201c}Previous Windows installation(s)\u{201d}, then OK.",
            ],
            tool: Some(Tool::DiskCleanup),
        });
    }
    out.push(Guide {
        id: "component-store",
        title: "Old Windows updates (component store)",
        size: None,
        why: "Windows keeps superseded system components after updates, often a few gigabytes in WinSxS. Only Windows can safely tell which ones are still needed.",
        steps: vec![
            "Open Disk Cleanup and choose the system drive.",
            "Click \u{201c}Clean up system files\u{201d} and approve the administrator prompt.",
            "Tick \u{201c}Windows Update Cleanup\u{201d}, then OK. It can take a few minutes.",
        ],
        tool: Some(Tool::DiskCleanup),
    });
    out
}

