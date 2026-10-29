// The macOS rules here aren't called on Windows (it has its own in windows.rs); only
// `old_temp_files` is shared.
#![cfg_attr(windows, allow(dead_code))]

use super::*;

/// Caches that hold state rather than throwaway data (iCloud, Find My, Wallet…), or ours.
const KEEP: &[&str] = &[
    "CloudKit",
    "FamilyCircle",
    "com.apple.bird",
    "com.apple.cloudd",
    "com.apple.HomeKit",
    "com.apple.homed",
    "com.apple.passd",
    "com.apple.akd",
    "com.apple.containermanagerd",
    "com.apple.findmy.fmipcore",
    "com.apple.findmy.fmfcore",
    "com.apple.icloud.fmfd",
    "com.apple.ap.adprivacyd",
    "com.pen.cleaner",
    "pencleaner",
];

/// ~/Library/Caches entries reported under Developer Junk instead.
pub(super) const DEV_CACHES: &[&str] = &[
    "Homebrew",
    "Yarn",
    "pip",
    "pypoetry",
    "go-build",
    "CocoaPods",
    "deno",
    "node-gyp",
    "typescript",
    "electron",
    "electron-builder",
    "ms-playwright",
    "Cypress",
    "org.carthage.CarthageKit",
    "JetBrains",
    "pnpm",
    "cargo-xwin",
    "composer",
    "com.apple.dt.Xcode",
];

/// Vendor folders holding several products' caches, listed per product.
const VENDOR_SPLIT: &[&str] = &["Adobe", "Google", "Microsoft"];

fn looks_like_bundle_id(name: &str) -> bool {
    name.split('.').count() >= 3 && !name.contains(' ')
}

/// Friendly name and running state for a cache folder named after an app.
fn describe(cx: &Cx, name: &str) -> (String, bool) {
    if looks_like_bundle_id(name) {
        let pretty = cx.apps.name(name).map(str::to_owned).unwrap_or_else(|| name.to_owned());
        return (pretty, cx.is_running(name));
    }
    (name.to_owned(), false)
}

fn cache_item(cx: &Cx, path: &Path, name: String, running: bool) -> Option<Item> {
    let mut item = ItemSpec { path, name, kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: !running }.build(cx)?;
    if running {
        item.warning = Some(format!("{} is running. Quit it first so it doesn't rebuild the cache while it's being cleared.", item.name));
    }
    Some(item)
}

pub fn user_caches(cx: &Cx) -> Vec<Group> {
    let root = cx.p("Library/Caches");
    let mut items = Vec::new();
    for path in children(&root) {
        let name = file_name(&path);
        if KEEP.contains(&name.as_str()) || DEV_CACHES.contains(&name.as_str()) {
            continue;
        }
        if VENDOR_SPLIT.contains(&name.as_str()) && path.is_dir() {
            for sub in children(&path) {
                let sub_name = file_name(&sub);
                // Android Studio lives under Google/ but belongs with the IDE caches.
                if sub_name.starts_with("AndroidStudio") {
                    continue;
                }
                items.extend(cache_item(cx, &sub, format!("{name} {sub_name}"), false));
            }
            continue;
        }
        let (pretty, running) = describe(cx, &name);
        items.extend(cache_item(cx, &path, pretty, running));
    }

    // Sandboxed apps keep their caches inside their container. The Caches folder itself
    // is part of the container's structure, so only its contents are removed.
    for container in children(&cx.p("Library/Containers")) {
        let caches = container.join("Data/Library/Caches");
        let entries = children(&caches);
        if entries.is_empty() {
            continue;
        }
        let id = file_name(&container);
        let (pretty, running) = describe(cx, &id);
        let Some(size) = cx.measure(&caches) else { continue };
        items.push(Item {
            id: caches.to_string_lossy().into_owned(),
            name: pretty.clone(),
            detail: Some(cx.tilde(&caches)),
            path: Some(caches.to_string_lossy().into_owned()),
            size,
            kind: ItemKind::Folder,
            mode: RemoveMode::Delete,
            last_activity: mtime_of(&caches),
            recommended: !running,
            warning: running.then(|| format!("{pretty} is running. Quit it first for a complete clean.")),
            admin: false,
            action: Action::Paths(entries),
        });
    }

    // Premiere Pro and After Effects keep their media cache outside ~/Library/Caches.
    for rel in ["Library/Application Support/Adobe/Common/Media Cache Files", "Library/Application Support/Adobe/Common/Media Cache"] {
        let path = cx.p(rel);
        items.extend(cache_item(cx, &path, format!("Adobe {}", file_name(&path)), false));
    }

    let mut g = group(
        "user-caches",
        Category::System,
        "User Cache Files",
        "Data apps keep around to start and load faster. They rebuild it as needed.",
        items,
    );
    if unreadable(&cx.p("Library/Containers/com.apple.mail")) {
        g.note = Some("Some app containers are hidden without Full Disk Access.".into());
    }
    vec![g]
}

pub fn logs(cx: &Cx) -> Vec<Group> {
    let items = children(&cx.p("Library/Logs"))
        .iter()
        .filter_map(|path| {
            let (name, _) = describe(cx, &file_name(path));
            ItemSpec { path, name, kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: true }.build(cx)
        })
        .collect();
    vec![group("user-logs", Category::System, "User Log Files", "Activity and crash logs written by apps and macOS.", items)]
}

/// The temp folder is in use by running apps; anything untouched for 3 days is
/// abandoned. Shared with the Windows rules (%TEMP%).
pub(super) fn old_temp_files(cx: &Cx) -> Option<Item> {
    let tmp = std::env::temp_dir();
    let cutoff = cx.now - 3 * DAY;
    let old: Vec<PathBuf> = children(&tmp).into_iter().filter(|p| mtime_of(p).is_some_and(|t| t < cutoff)).collect();
    let size: u64 = old.iter().map(|p| fsutil::disk_usage(p)).sum();
    if size < MIN_ITEM {
        return None;
    }
    cx.ctl.add(old.len() as u64, size);
    Some(Item {
        id: format!("{}::old", tmp.display()),
        name: "Old temporary files".into(),
        detail: Some(format!("{} items untouched for 3+ days", old.len())),
        path: Some(tmp.to_string_lossy().into_owned()),
        size,
        kind: ItemKind::Folder,
        mode: RemoveMode::Delete,
        last_activity: None,
        recommended: true,
        warning: None,
        admin: false,
        action: Action::Paths(old),
    })
}

pub fn temporary(cx: &Cx) -> Vec<Group> {
    let mut items: Vec<Item> = old_temp_files(cx).into_iter().collect();

    let saved = cx.p("Library/Saved Application State");
    let entries = children(&saved);
    if !entries.is_empty() {
        if let Some(size) = cx.measure(&saved) {
            items.push(Item {
                id: saved.to_string_lossy().into_owned(),
                name: "Saved application states".into(),
                detail: Some("Window positions apps restore on launch".into()),
                path: Some(saved.to_string_lossy().into_owned()),
                size,
                kind: ItemKind::Folder,
                mode: RemoveMode::Delete,
                last_activity: None,
                recommended: true,
                warning: None,
                admin: false,
                action: Action::Paths(entries),
            });
        }
    }
    vec![group("temporary", Category::System, "Temporary Files", "Leftovers from apps that didn't clean up after themselves.", items)]
}

pub fn ios(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();
    let backups = cx.p("Library/Application Support/MobileSync/Backup");
    for b in children(&backups) {
        let info = plist::Value::from_file(b.join("Info.plist")).ok();
        let dict = info.as_ref().and_then(|v| v.as_dictionary());
        let device = dict.and_then(|d| d.get("Device Name")).and_then(|v| v.as_string()).unwrap_or("iOS device");
        let date = dict.and_then(|d| d.get("Last Backup Date")).and_then(|v| v.as_date()).map(|d| {
            let t: SystemTime = d.into();
            t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
        });
        if let Some(mut item) = (ItemSpec {
            path: &b,
            name: format!("{device} backup"),
            kind: ItemKind::Device,
            mode: RemoveMode::Trash,
            recommended: false,
        })
        .build(cx)
        {
            item.last_activity = date.or(item.last_activity);
            item.warning = Some("Your only copy of this device's data if it isn't backed up to iCloud.".into());
            items.push(item);
        }
    }
    for dir in ["iPhone Software Updates", "iPad Software Updates", "iPod Software Updates"] {
        for f in children(&cx.p(&format!("Library/iTunes/{dir}"))) {
            if f.extension().is_some_and(|e| e == "ipsw") {
                items.extend(ItemSpec { path: &f, name: file_name(&f), kind: ItemKind::File, mode: RemoveMode::Delete, recommended: true }.build(cx));
            }
        }
    }
    let mut g = group("ios", Category::System, "iOS Backups & Firmware", "Old iPhone and iPad backups and downloaded system updates.", items);
    if unreadable(&backups) {
        g.note = Some("Device backups are hidden without Full Disk Access.".into());
    }
    vec![g]
}
