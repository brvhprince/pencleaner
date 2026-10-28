//! The Smart Care timer. Checks every 15 minutes while PenCleaner runs; reminders come at
//! most once a day, and the unattended clean only takes junk that rebuilds itself.

use crate::commands::{record, AppState};
use pencleaner_core::care::{self, Due};
use pencleaner_core::progress::ScanControl;
use pencleaner_core::protection::watch::{self, post_notification};
use pencleaner_core::{bytes_label, settings};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // Let login settle before doing anything.
        std::thread::sleep(Duration::from_secs(5 * 60));
        loop {
            tick(&app);
            std::thread::sleep(Duration::from_secs(15 * 60));
        }
    });
}

fn tick(app: &AppHandle) {
    let dir = watch::data_dir();
    let settings = settings::load(&dir);
    let mut state = care::load(&dir);
    match care::check(&settings, &state, now()) {
        Due::Nothing => {}
        Due::Remind => {
            let days = state.last_run.map_or(settings.care_every_days as i64, |l| (now() - l) / 86_400);
            post_notification("Smart Care is due", &format!("It's been {days} days. Open PenCleaner from the menu bar to run it."));
            state.last_reminded = Some(now());
            care::save(&dir, &state);
        }
        Due::Clean => {
            let st = app.state::<AppState>();
            // Someone's using Cleanup right now: try again next tick.
            if st.cleanup_ctl.lock().is_some() {
                return;
            }
            let ctl = Arc::new(ScanControl::new());
            *st.cleanup_ctl.lock() = Some(ctl.clone());
            let summary = care::auto_clean(&settings.matcher(), &ctl);
            *st.cleanup_ctl.lock() = None;
            // Whatever Cleanup showed before is out of date now.
            *st.cleanup.write() = None;
            state.last_run = Some(now());
            state.last_reminded = None;
            care::save(&dir, &state);
            record(app, "smartcare", "Scheduled Smart Care", summary.bytes_freed, summary.cleaned.len(), Some("Caches, logs and temporary files".into()));
            if summary.bytes_freed > 0 {
                post_notification("Smart Care ran", &format!("Freed {} of caches, logs and temporary files.", bytes_label(summary.bytes_freed)));
            }
        }
    }
}
