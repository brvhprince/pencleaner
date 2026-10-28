//! Notices apps dropped into the Trash and offers to remove what they left behind,
//! like CleanMyMac's "Uninstall applications correctly". Runs while PenCleaner runs.

use crate::commands::AppState;
use pencleaner_core::applications::leftovers_of_trashed;
use pencleaner_core::{fsutil, settings};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

fn trashed_apps() -> Option<HashSet<PathBuf>> {
    let rd = std::fs::read_dir(fsutil::home_dir().join(".Trash")).ok()?;
    Some(rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "app")).collect())
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // Whatever is in the Trash at launch is old news.
        let mut seen = trashed_apps().unwrap_or_default();
        loop {
            std::thread::sleep(Duration::from_secs(3));
            // Unreadable without Full Disk Access: then this simply never fires.
            let Some(now) = trashed_apps() else { continue };
            let fresh: Vec<PathBuf> = now.difference(&seen).cloned().collect();
            seen = now;
            if fresh.is_empty() || !settings::load(&pencleaner_core::protection::watch::data_dir()).offer_trashed_app_cleanup {
                continue;
            }
            // One card at a time: the biggest leftover pile wins.
            let best = fresh.iter().filter_map(|p| leftovers_of_trashed(p)).max_by_key(|t| t.size);
            if let Some(t) = best {
                *app.state::<AppState>().trashed.lock() = Some(t.clone());
                if crate::menubar::pop_up(&app, crate::menubar::COMPACT_HEIGHT).is_some() {
                    let _ = tauri::Emitter::emit_to(&app, crate::menubar::WINDOW, "trashed-app", t);
                }
            }
        }
    });
}
