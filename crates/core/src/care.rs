//! Scheduled Smart Care: when it's due, and the unattended clean. The timer itself lives
//! in the app (it runs while PenCleaner is open, which Open at Login makes "always").

use crate::cleanup::{self, CleanSummary, Category};
use crate::progress::ScanControl;
use crate::settings::{CareMode, Ignore, Settings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const DAY: i64 = 86_400;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CareState {
    /// Last Smart Care, scheduled or by hand (Unix seconds).
    pub last_run: Option<i64>,
    /// Last "it's due" notification, so reminders come at most daily.
    pub last_reminded: Option<i64>,
}

fn file(dir: &Path) -> std::path::PathBuf {
    dir.join("care.json")
}

pub fn load(dir: &Path) -> CareState {
    fs::read(file(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(dir: &Path, s: &CareState) {
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(file(dir), serde_json::to_vec(s).unwrap_or_default());
}

/// When the next Smart Care is due, or None when scheduling is off. A schedule that has
/// never run counts from `now`, so switching it on doesn't fire straight away.
pub fn next_due(settings: &Settings, state: &CareState, now: i64) -> Option<i64> {
    if settings.care_mode == CareMode::Off {
        return None;
    }
    Some(state.last_run.unwrap_or(now) + settings.care_every_days.max(1) as i64 * DAY)
}

/// What the scheduler should do right now.
#[derive(Debug, PartialEq, Eq)]
pub enum Due {
    Nothing,
    Remind,
    Clean,
}

pub fn check(settings: &Settings, state: &CareState, now: i64) -> Due {
    match next_due(settings, state, now) {
        Some(at) if now >= at => match settings.care_mode {
            CareMode::Clean => Due::Clean,
            CareMode::Remind if state.last_reminded.is_none_or(|r| now - r >= DAY) => Due::Remind,
            _ => Due::Nothing,
        },
        _ => Due::Nothing,
    }
}

/// Scans and cleans the self-rebuilding junk, honouring the ignore list. No prompts.
pub fn auto_clean(ignore: &Ignore, ctl: &ScanControl) -> CleanSummary {
    let mut report = cleanup::scan(&[Category::System, Category::Developer, Category::Ai], ctl);
    if report.cancelled {
        return CleanSummary::default();
    }
    report.apply_ignore(ignore);
    let ids = report.auto_ids();
    report.clean(&ids, |_| {})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_counts_from_the_last_run_and_reminds_daily() {
        let now = 1_000 * DAY;
        let mut s = Settings::default();
        let mut st = CareState::default();
        assert_eq!(check(&s, &st, now), Due::Nothing, "off by default");

        s.care_mode = CareMode::Remind;
        assert_eq!(check(&s, &st, now), Due::Nothing, "never run: counts from now");
        st.last_run = Some(now - 7 * DAY);
        assert_eq!(check(&s, &st, now), Due::Remind);
        st.last_reminded = Some(now - 3_600);
        assert_eq!(check(&s, &st, now), Due::Nothing, "already reminded today");
        st.last_reminded = Some(now - DAY);
        assert_eq!(check(&s, &st, now), Due::Remind);

        s.care_mode = CareMode::Clean;
        assert_eq!(check(&s, &st, now), Due::Clean);
        st.last_run = Some(now - 6 * DAY);
        assert_eq!(check(&s, &st, now), Due::Nothing, "a day early");

        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &st);
        assert_eq!(load(dir.path()), st);
    }
}
