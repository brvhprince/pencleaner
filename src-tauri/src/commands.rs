//! Thin IPC layer over pencleaner-core. Anything slow runs on the blocking pool so
//! the main thread (which owns the window) never stalls.

use parking_lot::{Mutex, RwLock};
use pencleaner_core::cleanup::{self, Category, CleanEvent, CleanSummary, Report};
use pencleaner_core::activity::{self, Event};
use pencleaner_core::applications::{self, AppsReport, Target};
use pencleaner_core::clutter::{self, ClutterReport};
use pencleaner_core::performance::{self, items::LaunchItem, memory, PerfReport, TaskResult};
use pencleaner_core::progress::{Progress, ScanControl};
use pencleaner_core::remove::{self, RemoveEvent, RemoveItem, RemoveMode, RemoveSummary};
use pencleaner_core::space_lens::{self, Listing, ScanSummary, ScanTree};
use pencleaner_core::volumes::{self, Volume};
use pencleaner_core::{fsutil, permissions};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

type CmdResult<T> = Result<T, String>;

fn activity_log(app: &AppHandle) -> Option<activity::Log> {
    app.path().app_data_dir().ok().map(|d| activity::Log::new(&d))
}

/// Notes an action in My Activity. Never fails the action itself.
fn record(app: &AppHandle, module: &str, action: &str, bytes: u64, items: usize, detail: Option<String>) {
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
    cleanup: RwLock<Option<Report>>,
    cleanup_ctl: Mutex<Option<Arc<ScanControl>>>,
    clutter: RwLock<Option<ClutterReport>>,
    clutter_ctl: Mutex<Option<Arc<ScanControl>>>,
    apps: RwLock<Option<AppsReport>>,
    icons: Mutex<HashMap<String, Option<String>>>,
    perf: RwLock<Option<PerfReport>>,
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

#[tauri::command]
pub fn volumes_list() -> Vec<Volume> {
    volumes::list()
}

#[tauri::command]
pub fn open_full_disk_access_settings() -> CmdResult<()> {
    open(&[permissions::FULL_DISK_ACCESS_SETTINGS])
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> CmdResult<()> {
    open(&["-R", &path])
}

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

    let report = result?;
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
    let result = tauri::async_runtime::spawn_blocking(move || clutter::scan(&root, &ctl))
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
pub async fn apps_scan(app: AppHandle, on_progress: Channel<Progress>) -> CmdResult<AppsReport> {
    let ctl = Arc::new(ScanControl::new());
    let done = stream_progress(ctl.clone(), on_progress);
    let result = tauri::async_runtime::spawn_blocking(move || applications::scan(&ctl))
        .await
        .map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    let report = result?;
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

#[tauri::command]
pub async fn perf_scan(app: AppHandle) -> CmdResult<PerfReport> {
    // Includes "Open at Login" apps, which asks once for permission to read them.
    let report = tauri::async_runtime::spawn_blocking(|| performance::scan(true)).await.map_err(|e| e.to_string())?;
    *app.state::<AppState>().perf.write() = Some(report.clone());
    Ok(report)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySnapshot {
    memory: memory::MemoryStats,
    processes: Vec<memory::ProcessMemory>,
}

/// Cheap live numbers for the memory gauge.
#[tauri::command]
pub async fn perf_memory() -> CmdResult<MemorySnapshot> {
    tauri::async_runtime::spawn_blocking(|| MemorySnapshot { memory: memory::stats(), processes: memory::top_processes(10) })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn perf_run_tasks(app: AppHandle, ids: Vec<String>, on_start: Channel<String>) -> CmdResult<Vec<TaskResult>> {
    tauri::async_runtime::spawn_blocking(move || {
        let results = performance::run_tasks(&ids, |id| {
            let _ = on_start.send(id.to_owned());
        });
        let done: Vec<&str> = results.iter().filter(|r| r.ok).map(|r| r.id.as_str()).collect();
        record(&app, "performance", "Ran maintenance", 0, done.len(), Some(done.join(", ")));
        results
    })
    .await
    .map_err(|e| e.to_string())
}

fn perf_item(app: &AppHandle, id: &str) -> CmdResult<LaunchItem> {
    let state = app.state::<AppState>();
    let guard = state.perf.read();
    let report = guard.as_ref().ok_or("Scan again first")?;
    report.items.iter().find(|i| i.id == id).cloned().ok_or_else(|| format!("Unknown item {id}"))
}

/// Re-reads items after a change so the UI shows what launchd actually did.
fn refresh_items(app: &AppHandle) -> Vec<LaunchItem> {
    let items = performance::items::list(true);
    if let Some(r) = app.state::<AppState>().perf.write().as_mut() {
        r.items = items.clone();
    }
    items
}

#[tauri::command]
pub async fn perf_set_item(app: AppHandle, id: String, enabled: bool) -> CmdResult<Vec<LaunchItem>> {
    tauri::async_runtime::spawn_blocking(move || {
        let item = perf_item(&app, &id)?;
        performance::items::set_enabled(&item, enabled)?;
        let action = if enabled { "Switched on a background item" } else { "Switched off a background item" };
        record(&app, "performance", action, 0, 1, Some(item.name.clone()));
        Ok(refresh_items(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn perf_remove_item(app: AppHandle, id: String) -> CmdResult<Vec<LaunchItem>> {
    tauri::async_runtime::spawn_blocking(move || {
        let item = perf_item(&app, &id)?;
        performance::items::remove(&item)?;
        record(&app, "performance", "Removed a background item", 0, 1, Some(item.name.clone()));
        Ok(refresh_items(&app))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotState {
    snapshots: Vec<performance::snapshots::Snapshot>,
    purgeable: Option<u64>,
}

/// Deletes the given Time Machine snapshots (all when empty) and reports what's left.
#[tauri::command]
pub async fn perf_delete_snapshots(app: AppHandle, ids: Vec<String>) -> CmdResult<SnapshotState> {
    tauri::async_runtime::spawn_blocking(move || {
        let (count, before) = (performance::snapshots::list().len(), performance::snapshots::purgeable().unwrap_or(0));
        performance::delete_snapshots(&ids)?;
        let after = SnapshotState { snapshots: performance::snapshots::list(), purgeable: performance::snapshots::purgeable() };
        let removed = count.saturating_sub(after.snapshots.len());
        record(&app, "performance", "Removed Time Machine snapshots", before.saturating_sub(after.purgeable.unwrap_or(0)), removed, None);
        Ok(after)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_login_items_settings() -> CmdResult<()> {
    open(&["x-apple.systempreferences:com.apple.LoginItems-Settings.extension"])
}

#[tauri::command]
pub fn activity_list(app: AppHandle) -> Vec<Event> {
    activity_log(&app).map(|l| l.load()).unwrap_or_default()
}

#[tauri::command]
pub fn activity_clear(app: AppHandle) -> CmdResult<()> {
    activity_log(&app).map_or(Ok(()), |l| l.clear().map_err(|e| e.to_string()))
}
