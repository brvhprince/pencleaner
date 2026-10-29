//! Thin IPC layer over pencleaner-core. Anything slow runs on the blocking pool so
//! the main thread (which owns the window) never stalls.

use parking_lot::{Mutex, RwLock};
use pencleaner_core::cleanup::{self, Category, CleanEvent, CleanSummary, Report};
use pencleaner_core::activity::{self, Event};
use pencleaner_core::applications::{self, AppsReport, Target};
use pencleaner_core::clutter::{self, ClutterReport};
use pencleaner_core::settings::{self as user_settings, IgnoreKind, Settings};
use pencleaner_core::progress::{Progress, ScanControl};
use pencleaner_core::remove::{self, RemoveEvent, RemoveItem, RemoveMode, RemoveSummary};
use pencleaner_core::space_lens::{self, Listing, ScanSummary, ScanTree};
use pencleaner_core::volumes::{self, Volume};
use pencleaner_core::{fsutil, permissions};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

// Commands for macOS-only modules (Applications, Performance, Protection, updates,
// the menu bar's vitals, Open at Login).
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

type CmdResult<T> = Result<T, String>;

/// Settings live beside the activity log, in PenCleaner's data folder.
fn load_settings() -> Settings {
    user_settings::load(&fsutil::data_dir())
}

fn activity_log(app: &AppHandle) -> Option<activity::Log> {
    app.path().app_data_dir().ok().map(|d| activity::Log::new(&d))
}

/// Notes an action in My Activity. Never fails the action itself.
pub(crate) fn record(app: &AppHandle, module: &str, action: &str, bytes: u64, items: usize, detail: Option<String>) {
    if items == 0 && bytes == 0 {
        return;
    }
    if let Some(log) = activity_log(app) {
        let _ = log.append(&Event::now(module, action, bytes, items as u32, detail));
    }
}

#[derive(Default)]
pub struct AppState {
    lens: RwLock<Option<ScanTree>>,
    lens_ctl: Mutex<Option<Arc<ScanControl>>>,
    pub(crate) cleanup: RwLock<Option<Report>>,
    pub(crate) cleanup_ctl: Mutex<Option<Arc<ScanControl>>>,
    clutter: RwLock<Option<ClutterReport>>,
    clutter_ctl: Mutex<Option<Arc<ScanControl>>>,
    apps: RwLock<Option<AppsReport>>,
    icons: Mutex<std::collections::HashMap<String, Option<String>>>,
    #[cfg(target_os = "macos")]
    perf: RwLock<Option<pencleaner_core::performance::PerfReport>>,
    #[cfg(target_os = "macos")]
    updates: RwLock<Option<pencleaner_core::updater::UpdatesReport>>,
    #[cfg(target_os = "macos")]
    protection: RwLock<Option<pencleaner_core::protection::ProtectionReport>>,
    #[cfg(target_os = "macos")]
    protection_ctl: Mutex<Option<Arc<ScanControl>>>,
    vitals: pencleaner_core::vitals::Sampler,
    #[cfg(target_os = "macos")]
    pub trashed: Mutex<Option<pencleaner_core::applications::TrashedApp>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    full_disk_access: bool,
    home: String,
    user: String,
    /// In `tauri dev` the app runs as a child of the terminal, so Full Disk Access
    /// has to be granted to the terminal app instead.
    dev_build: bool,
}

#[tauri::command]
pub fn system_info() -> SystemInfo {
    let home = fsutil::home_dir();
    SystemInfo {
        full_disk_access: permissions::has_full_disk_access(),
        user: home.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        home: home.to_string_lossy().into_owned(),
        dev_build: cfg!(debug_assertions),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    version: String,
    /// Short commit hash; "+" when built with uncommitted changes.
    commit: &'static str,
    built_at: u64,
    macos: String,
    arch: &'static str,
    full_disk_access: bool,
    xprotect: Option<String>,
}

#[tauri::command]
pub fn about_info(app: AppHandle) -> AboutInfo {
    #[cfg(target_os = "macos")]
    let (macos, xprotect) = (
        Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default(),
        plist::Value::from_file("/Library/Apple/System/Library/CoreServices/XProtect.bundle/Contents/version.plist")
            .ok()
            .and_then(|v| v.as_dictionary()?.get("CFBundleShortVersionString")?.as_string().map(str::to_owned)),
    );
    // "Microsoft Windows [Version 10.0.26100.2314]": the build number tells 10 from 11.
    #[cfg(windows)]
    let (macos, xprotect) = (
        fsutil::command("cmd").args(["/C", "ver"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default(),
        None::<String>,
    );
    AboutInfo {
        version: app.package_info().version.to_string(),
        commit: env!("PENCLEANER_COMMIT"),
        built_at: env!("PENCLEANER_BUILT_AT").parse().unwrap_or(0),
        macos,
        arch: match (cfg!(target_os = "macos"), cfg!(target_arch = "aarch64")) {
            (true, true) => "Apple Silicon",
            (true, false) => "Intel",
            (false, true) => "ARM64",
            (false, false) => "x64",
        },
        full_disk_access: permissions::has_full_disk_access(),
        xprotect,
    }
}

#[tauri::command]
pub fn volumes_list() -> Vec<Volume> {
    volumes::list()
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    return open(&["-R", &path]);
    // Explorer exits non-zero even when it worked, so only a failure to start counts.
    #[cfg(windows)]
    {
        Command::new("explorer").arg(format!("/select,{path}")).spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn open(args: &[&str]) -> CmdResult<()> {
    Command::new("/usr/bin/open").args(args).status().map_err(|e| e.to_string())?;
    Ok(())
}

/// Starts a sampler thread that pushes `ctl` snapshots until the returned flag is set.
fn stream_progress(ctl: Arc<ScanControl>, channel: Channel<Progress>) -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            if channel.send(ctl.snapshot()).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    });
    done
}

#[tauri::command]
pub async fn space_lens_scan(
    app: AppHandle,
    root: String,
    on_progress: Channel<Progress>,
) -> CmdResult<ScanSummary> {
    let state = app.state::<AppState>();
    // Free the previous tree before building a new one.
    *state.lens.write() = None;
    let ctl = Arc::new(ScanControl::new());
    *state.lens_ctl.lock() = Some(ctl.clone());

    let done = stream_progress(ctl.clone(), on_progress);
    let root = PathBuf::from(root);
    let result = tauri::async_runtime::spawn_blocking(move || space_lens::scan(&root, &ctl))
        .await
        .map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    *state.lens_ctl.lock() = None;

    let tree = result?.map_err(|e| e.to_string())?;
    let summary = tree.summary();
    if !summary.cancelled {
        *state.lens.write() = Some(tree);
    }
    Ok(summary)
}

#[tauri::command]
pub fn space_lens_cancel(state: State<'_, AppState>) {
    if let Some(ctl) = state.lens_ctl.lock().as_ref() {
        ctl.cancel();
    }
}

#[tauri::command]
pub fn space_lens_discard(state: State<'_, AppState>) {
    *state.lens.write() = None;
}

#[tauri::command]
pub async fn space_lens_list(app: AppHandle, path: String) -> CmdResult<Listing> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let lens = state.lens.read();
        let tree = lens.as_ref().ok_or("Nothing has been scanned yet")?;
        tree.list(Path::new(&path), &fsutil::home_dir()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_items(
    app: AppHandle,
    items: Vec<RemoveItem>,
    mode: RemoveMode,
    on_event: Channel<RemoveEvent>,
) -> CmdResult<RemoveSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let summary = remove::remove_items(&items, mode, |ev| {
            let _ = on_event.send(ev);
        });
        record(&app, "spacelens", "Moved to the Trash", summary.bytes_freed, summary.removed.len(), None);
        // Keep Space Lens sizes truthful without a rescan.
        let state = app.state::<AppState>();
        if let Some(tree) = state.lens.write().as_mut() {
            for (path, bytes) in &summary.removed {
                tree.forget(Path::new(path), *bytes);
            }
        }
        summary
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cleanup_scan(
    app: AppHandle,
    categories: Option<Vec<Category>>,
    on_progress: Channel<Progress>,
) -> CmdResult<Report> {
    let state = app.state::<AppState>();
    let ctl = Arc::new(ScanControl::new());
    *state.cleanup_ctl.lock() = Some(ctl.clone());
    let done = stream_progress(ctl.clone(), on_progress);
    let cats = categories.unwrap_or_else(|| cleanup::ALL.to_vec());
    let result = tauri::async_runtime::spawn_blocking(move || cleanup::scan(&cats, &ctl))
        .await
        .map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    *state.cleanup_ctl.lock() = None;

    let mut report = result?;
    report.apply_ignore(&load_settings().matcher());
    *state.cleanup.write() = (!report.cancelled).then(|| report.clone());
    Ok(report)
}

#[tauri::command]
pub fn cleanup_cancel(state: State<'_, AppState>) {
    if let Some(ctl) = state.cleanup_ctl.lock().as_ref() {
        ctl.cancel();
    }
}

/// Removes the items with these ids from the last report. Paths never cross IPC.
#[tauri::command]
pub async fn cleanup_clean(
    app: AppHandle,
    ids: Vec<String>,
    on_event: Channel<CleanEvent>,
) -> CmdResult<CleanSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.cleanup.write();
        let report = guard.as_mut().ok_or("Scan again before cleaning")?;
        let ids: HashSet<String> = ids.into_iter().collect();
        let summary = report.clean(&ids, |ev| {
            let _ = on_event.send(ev);
        });
        let mut cats: Vec<&String> = summary.by_category.keys().collect();
        cats.sort();
        let detail = cats.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ");
        record(&app, "cleanup", "Cleaned junk", summary.bytes_freed, summary.cleaned.len(), Some(detail));
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clutter_scan(app: AppHandle, root: String, on_progress: Channel<Progress>) -> CmdResult<ClutterReport> {
    let state = app.state::<AppState>();
    let ctl = Arc::new(ScanControl::new());
    *state.clutter_ctl.lock() = Some(ctl.clone());
    let done = stream_progress(ctl.clone(), on_progress);
    let root = PathBuf::from(root);
    let prefs = load_settings();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let ig = prefs.matcher();
        let mut r = clutter::scan_with(&root, prefs.large_file_mb * 1_000_000, &ig, &ctl);
        r.apply_ignore(&ig);
        r
    })
        .await
        .map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    *state.clutter_ctl.lock() = None;

    let report = result?;
    *state.clutter.write() = (!report.cancelled).then(|| report.clone());
    Ok(report)
}

#[tauri::command]
pub fn clutter_cancel(state: State<'_, AppState>) {
    if let Some(ctl) = state.clutter_ctl.lock().as_ref() {
        ctl.cancel();
    }
}

/// Moves files from the last My Clutter report to the Trash. Unknown paths are ignored.
#[tauri::command]
pub async fn clutter_remove(
    app: AppHandle,
    ids: Vec<String>,
    on_event: Channel<RemoveEvent>,
) -> CmdResult<RemoveSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.clutter.write();
        let report = guard.as_mut().ok_or("Scan again before removing files")?;
        let summary = report.remove(&ids, |ev| {
            let _ = on_event.send(ev);
        });
        record(&app, "clutter", "Moved files to the Trash", summary.bytes_freed, summary.removed.len(), None);
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn activity_list(app: AppHandle) -> Vec<Event> {
    activity_log(&app).map(|l| l.load()).unwrap_or_default()
}

#[tauri::command]
pub fn activity_clear(app: AppHandle) -> CmdResult<()> {
    activity_log(&app).map_or(Ok(()), |l| l.clear().map_err(|e| e.to_string()))
}

#[tauri::command]
pub fn settings_get() -> Settings {
    load_settings()
}

/// Saves everything except the ignore list, which has its own commands.
#[tauri::command]
pub fn settings_save(settings: Settings) -> CmdResult<Settings> {
    let mut s = settings;
    s.ignore = load_settings().ignore;
    user_settings::save(&fsutil::data_dir(), &s)?;
    Ok(s)
}

/// Prunes the cached results right away, so a clean or remove can't reach the item.
fn apply_ignore_to_cached(app: &AppHandle, s: &Settings) {
    let ig = s.matcher();
    let state = app.state::<AppState>();
    if let Some(r) = state.cleanup.write().as_mut() {
        r.apply_ignore(&ig);
    }
    if let Some(r) = state.clutter.write().as_mut() {
        r.apply_ignore(&ig);
    };
    if let Some(r) = state.apps.write().as_mut() {
        r.apply_ignore(&ig);
    };
    #[cfg(target_os = "macos")]
    {
        if let Some(r) = state.updates.write().as_mut() {
            r.apply_ignore(&ig);
        }
        if let Some(r) = state.protection.write().as_mut() {
            r.apply_ignore(&ig);
        };
    }
}

#[tauri::command]
pub fn ignore_add(app: AppHandle, kind: IgnoreKind, value: String, label: String, module: String) -> CmdResult<Settings> {
    if kind == IgnoreKind::Path && !Path::new(&value).is_absolute() {
        return Err("Only absolute paths can be ignored".into());
    }
    let mut s = load_settings();
    s.add_ignore(kind, &value, &label, &module);
    user_settings::save(&fsutil::data_dir(), &s)?;
    apply_ignore_to_cached(&app, &s);
    Ok(s)
}

/// Un-ignoring shows the item again from the next scan.
#[tauri::command]
pub fn ignore_remove(kind: IgnoreKind, value: String) -> CmdResult<Settings> {
    let mut s = load_settings();
    s.remove_ignore(kind, &value);
    user_settings::save(&fsutil::data_dir(), &s)?;
    Ok(s)
}

/// Brings the main window forward, optionally on a module.
#[tauri::command]
pub fn show_main(app: AppHandle, module: Option<String>) {
    if let Some(p) = app.get_webview_window(crate::menubar::WINDOW) {
        let _ = p.hide();
    }
    crate::menubar::show_main(&app);
    if let Some(m) = module {
        let _ = tauri::Emitter::emit_to(&app, "main", "navigate", m);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CareStatus {
    mode: pencleaner_core::settings::CareMode,
    every_days: u32,
    last_run: Option<i64>,
    next_due: Option<i64>,
    due: bool,
}

/// Scheduled Smart Care: when it last ran and when it's next due.
#[tauri::command]
pub fn care_status() -> CareStatus {
    let settings = load_settings();
    let state = pencleaner_core::care::load(&fsutil::data_dir());
    let now = crate::care::now();
    let next_due = pencleaner_core::care::next_due(&settings, &state, now);
    CareStatus { mode: settings.care_mode, every_days: settings.care_every_days, last_run: state.last_run, next_due, due: next_due.is_some_and(|d| now >= d) }
}

/// Smart Care was run by hand: the schedule starts counting again.
#[tauri::command]
pub fn care_done() {
    let dir = fsutil::data_dir();
    let mut state = pencleaner_core::care::load(&dir);
    state.last_run = Some(crate::care::now());
    state.last_reminded = None;
    pencleaner_core::care::save(&dir, &state);
}

#[tauri::command]
pub fn login_item_status() -> crate::login::LoginStatus {
    crate::login::status()
}

#[tauri::command]
pub fn login_item_set(on: bool) -> CmdResult<crate::login::LoginStatus> {
    crate::login::set(on)
}

/// System Settings → Login Items, where a switched-off login item is approved again.
#[tauri::command]
pub fn open_login_items_for_app() {
    crate::login::open_settings();
}

/// Live numbers for the menu bar popover (cheap; polled every couple of seconds).
#[tauri::command]
pub async fn vitals(app: AppHandle) -> CmdResult<pencleaner_core::vitals::Vitals> {
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().vitals.read()).await.map_err(|e| e.to_string())
}

/// Empties the Trash on the startup disk / the Recycle Bin (permanently). Returns the
/// bytes freed.
#[tauri::command]
pub async fn empty_trash(app: AppHandle) -> CmdResult<u64> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(move || {
        let freed = pencleaner_core::windows::recycle_bin::empty()?;
        record(&app, "cleanup", "Emptied the Recycle Bin", freed, 1, None);
        Ok(freed)
    })
    .await
    .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    tauri::async_runtime::spawn_blocking(move || {
        let trash = fsutil::home_dir().join(".Trash");
        let entries: Vec<PathBuf> = std::fs::read_dir(&trash).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
        let items: Vec<RemoveItem> = entries.into_iter().map(|path| RemoveItem { path, size: None }).collect();
        let summary = remove::remove_items(&items, RemoveMode::Delete, |_| {});
        record(&app, "cleanup", "Emptied the Trash", summary.bytes_freed, summary.removed.len(), None);
        if summary.failed.is_empty() { Ok(summary.bytes_freed) } else { Err(summary.failed[0].1.clone()) }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens a Windows cleanup tool a Cleanup guide points to (Disk Cleanup, Storage settings).
#[tauri::command]
pub fn open_system_tool(tool: pencleaner_core::cleanup::Tool) -> CmdResult<()> {
    #[cfg(windows)]
    {
        use pencleaner_core::cleanup::Tool;
        let mut cmd = match tool {
            Tool::DiskCleanup => fsutil::command("cleanmgr.exe"),
            Tool::StorageSettings => {
                let mut c = fsutil::command("explorer.exe");
                c.arg("ms-settings:storagesense");
                c
            }
        };
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = tool;
        Err("Only available on Windows".into())
    }
}

#[tauri::command]
pub async fn apps_scan(app: AppHandle, on_progress: Channel<Progress>) -> CmdResult<AppsReport> {
    let ctl = Arc::new(ScanControl::new());
    let done = stream_progress(ctl.clone(), on_progress);
    let result = tauri::async_runtime::spawn_blocking(move || applications::scan(&ctl))
        .await
        .map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    let mut report = result?;
    report.apply_ignore(&load_settings().matcher());
    *app.state::<AppState>().apps.write() = Some(report.clone());
    Ok(report)
}

/// Uninstalls apps / removes leftovers and installers from the last report, by id.
#[tauri::command]
pub async fn apps_remove(
    app: AppHandle,
    targets: Vec<Target>,
    on_event: Channel<CleanEvent>,
) -> CmdResult<CleanSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.apps.write();
        let report = guard.as_mut().ok_or("Scan again before removing apps")?;
        let apps = targets.iter().filter(|t| matches!(t, Target::App(_))).count();
        let summary = report.remove(&targets, |ev| {
            let _ = on_event.send(ev);
        });
        let action = if apps > 0 { "Uninstalled apps" } else { "Removed app leftovers" };
        record(&app, "applications", action, summary.bytes_freed, summary.cleaned.len(), None);
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// An app's icon as a PNG data URL (cached), or null when it has none we can read.
#[tauri::command]
pub async fn app_icon(app: AppHandle, path: String) -> CmdResult<Option<String>> {
    use base64::Engine;
    if let Some(hit) = app.state::<AppState>().icons.lock().get(&path) {
        return Ok(hit.clone());
    }
    let key = path.clone();
    let url = tauri::async_runtime::spawn_blocking(move || {
        pencleaner_core::imaging::app_icon_png(Path::new(&path), 96)
            .map(|png| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
    })
    .await
    .map_err(|e| e.to_string())?;
    app.state::<AppState>().icons.lock().insert(key, url.clone());
    Ok(url)
}
