//! System notifications ("Smart Care ran", "Smart Care is due").

#[cfg(target_os = "macos")]
pub fn post(subtitle: &str, body: &str) {
    pencleaner_core::protection::watch::post_notification(subtitle, body);
}

/// Windows toasts come with the Windows tray work; until then the schedule still runs and
/// logs to My Activity, it just doesn't announce itself.
#[cfg(windows)]
pub fn post(_subtitle: &str, _body: &str) {}
