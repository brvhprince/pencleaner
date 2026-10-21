//! Thin IPC layer over pencleaner-core. Anything slow runs on the blocking pool so
//! the main thread (which owns the window) never stalls.

use parking_lot::{Mutex, RwLock};
use pencleaner_core::cleanup::{self, Category, CleanEvent, CleanSummary, Report};
use pencleaner_core::clutter::{self, ClutterReport};
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

type CmdResult<T> = Result<T, String>;

#[derive(Default)]
pub struct AppState {
    lens: RwLock<Option<ScanTree>>,
    lens_ctl: Mutex<Option<Arc<ScanControl>>>,
    cleanup: RwLock<Option<Report>>,
    cleanup_ctl: Mutex<Option<Arc<ScanControl>>>,
    clutter: RwLock<Option<ClutterReport>>,
    clutter_ctl: Mutex<Option<Arc<ScanControl>>>,
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
        Ok(report.clean(&ids, |ev| {
            let _ = on_event.send(ev);
        }))
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
        Ok(report.remove(&ids, |ev| {
            let _ = on_event.send(ev);
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}
