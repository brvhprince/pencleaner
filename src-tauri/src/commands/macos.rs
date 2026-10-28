//! Commands for the macOS-only parts of the engine.

use super::*;
use std::collections::HashMap;
use pencleaner_core::applications::{self, AppsReport, Target};
use pencleaner_core::performance::{self, items::LaunchItem, memory, PerfReport, TaskResult};
use pencleaner_core::protection::{self, permissions as app_permissions, privacy, watch, ProtectionReport};
use pencleaner_core::updater::{self, UpdateEvent, UpdatesReport};

#[tauri::command]
pub fn open_full_disk_access_settings() -> CmdResult<()> {
    open(&[permissions::FULL_DISK_ACCESS_SETTINGS])
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

/// The full System Settings list (asks for the password). Replaces the scan's items.
#[tauri::command]
pub async fn perf_full_items(app: AppHandle) -> CmdResult<Vec<LaunchItem>> {
    tauri::async_runtime::spawn_blocking(move || {
        let items = performance::items::full_list(true)?;
        if let Some(r) = app.state::<AppState>().perf.write().as_mut() {
            r.items = items.clone();
        }
        Ok(items)
    })
    .await
    .map_err(|e| e.to_string())?
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

/// Checks App Store, Sparkle feeds and Homebrew for newer versions. Network-bound.
#[tauri::command]
pub async fn updates_scan(app: AppHandle, on_step: Channel<String>) -> CmdResult<UpdatesReport> {
    let cache = app.path().app_cache_dir().ok();
    let mut report = tauri::async_runtime::spawn_blocking(move || {
        updater::scan(cache.as_deref(), |step| {
            let _ = on_step.send(step.to_owned());
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    report.apply_ignore(&load_settings().matcher());
    *app.state::<AppState>().updates.write() = Some(report.clone());
    Ok(report)
}

/// Installs updates from the last check, by id, one at a time.
#[tauri::command]
pub async fn updates_install(app: AppHandle, ids: Vec<String>, on_event: Channel<UpdateEvent>) -> CmdResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.updates.write();
        let report = guard.as_mut().ok_or("Check for updates first")?;
        let names: HashMap<String, String> = report.updates.iter().map(|u| (u.id.clone(), u.name.clone())).collect();
        let done = report.install(&ids, |ev| {
            let _ = on_event.send(ev);
        });
        let list: Vec<&str> = done.iter().filter_map(|id| names.get(id).map(String::as_str)).collect();
        record(&app, "applications", "Updated apps", 0, done.len(), Some(list.join(", ")));
        Ok(done)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_app_store_updates() -> CmdResult<()> {
    open(&["macappstore://showUpdatesPage"])
}

#[tauri::command]
pub async fn protection_scan(app: AppHandle, on_progress: Channel<Progress>) -> CmdResult<ProtectionReport> {
    let state = app.state::<AppState>();
    let ctl = Arc::new(ScanControl::new());
    *state.protection_ctl.lock() = Some(ctl.clone());
    let done = stream_progress(ctl.clone(), on_progress);
    let prefs = load_settings();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut r = protection::scan(&ctl, prefs.online_notarization);
        r.apply_ignore(&prefs.matcher());
        r
    })
    .await
    .map_err(|e| e.to_string());
    done.store(true, Ordering::Relaxed);
    *state.protection_ctl.lock() = None;
    let report = result?;
    *state.protection.write() = (!report.cancelled).then(|| report.clone());
    Ok(report)
}

#[tauri::command]
pub fn protection_cancel(state: State<'_, AppState>) {
    if let Some(ctl) = state.protection_ctl.lock().as_ref() {
        ctl.cancel();
    }
}

/// Removes a malware detection (by its target path) or a suspicious item (by label).
#[tauri::command]
pub async fn protection_remove(app: AppHandle, kind: String, id: String) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.protection.write();
        let report = guard.as_mut().ok_or("Scan again first")?;
        let (result, action) = match kind.as_str() {
            "detection" => (report.remove_detection(&id), "Removed malware"),
            _ => (report.remove_suspicious(&id), "Removed a suspicious item"),
        };
        result?;
        record(&app, "protection", action, 0, 1, Some(id));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyResult {
    cleaned: Vec<String>,
    failed: Vec<(String, String)>,
    bytes: u64,
}

#[tauri::command]
pub async fn privacy_clean(app: AppHandle, ids: Vec<String>) -> CmdResult<PrivacyResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut guard = state.protection.write();
        let report = guard.as_mut().ok_or("Scan again first")?;
        let wanted: HashSet<String> = ids.into_iter().collect();
        let r = privacy::clean(&report.privacy, &wanted);
        report.privacy.retain(|i| !r.cleaned.contains(&i.id));
        record(&app, "protection", "Cleared privacy traces", r.bytes, r.cleaned.len(), None);
        Ok(PrivacyResult { cleaned: r.cleaned, failed: r.failed, bytes: r.bytes })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Revokes one permission and returns the refreshed list.
#[tauri::command]
pub async fn permission_revoke(app: AppHandle, id: String) -> CmdResult<Vec<app_permissions::Grant>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let grant = {
            let guard = state.protection.read();
            guard.as_ref().and_then(|r| r.permissions.iter().find(|g| g.id == id).cloned()).ok_or("Not in the last scan")?
        };
        app_permissions::revoke(&grant)?;
        record(&app, "protection", "Revoked a permission", 0, 1, Some(format!("{}: {}", grant.label, grant.app_name)));
        let fresh = app_permissions::scan()?;
        if let Some(r) = state.protection.write().as_mut() {
            r.permissions = fresh.clone();
        }
        Ok(fresh)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_privacy_settings(anchor: String) -> CmdResult<()> {
    if !anchor.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("bad anchor".into());
    }
    open(&[&app_permissions::settings_url(&anchor)])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherState {
    enabled: bool,
    alerts: Vec<watch::Alert>,
}

#[tauri::command]
pub fn watcher_set(enabled: bool) -> CmdResult<WatcherState> {
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        watch::enable(&exe, &watch::data_dir())?;
    } else {
        watch::disable()?;
    }
    Ok(WatcherState { enabled: watch::is_enabled(), alerts: watch::alerts(&watch::data_dir()) })
}

#[tauri::command]
pub fn watcher_status() -> bool {
    watch::is_enabled()
}

#[tauri::command]
pub fn watcher_clear_alerts() {
    watch::clear_alerts(&watch::data_dir());
}

/// Live numbers for the menu bar popover (cheap; polled every couple of seconds).
#[tauri::command]
pub async fn vitals(app: AppHandle) -> CmdResult<pencleaner_core::vitals::Vitals> {
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().vitals.read()).await.map_err(|e| e.to_string())
}

/// Empties the Trash on the startup disk (permanently). Returns the bytes freed.
#[tauri::command]
pub async fn empty_trash(app: AppHandle) -> CmdResult<u64> {
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

/// Removes what the just-trashed app left behind (to the Trash).
#[tauri::command]
pub async fn trashed_app_remove(app: AppHandle) -> CmdResult<CleanSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let t = app.state::<AppState>().trashed.lock().take().ok_or("Nothing to remove")?;
        let name = t.name.clone();
        let summary = t.remove(|_| {});
        record(&app, "applications", "Removed a trashed app's leftovers", summary.bytes_freed, summary.cleaned.len(), Some(name));
        match summary.failed.first() {
            Some((_, e)) => Err(e.clone()),
            None => Ok(summary),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn trashed_app_dismiss(app: AppHandle) {
    *app.state::<AppState>().trashed.lock() = None;
    if let Some(w) = app.get_webview_window(crate::menubar::WINDOW) {
        let _ = w.hide();
    }
}
