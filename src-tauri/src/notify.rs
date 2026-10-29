//! System notifications ("Smart Care is due", "Smart Care ran").

use tauri::AppHandle;

#[cfg(target_os = "macos")]
pub fn post(_app: &AppHandle, subtitle: &str, body: &str) {
    pencleaner_core::protection::watch::post_notification(subtitle, body);
}

/// A Windows toast, attributed to PenCleaner (its installer registers the app's id).
#[cfg(windows)]
pub fn post(app: &AppHandle, subtitle: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(subtitle).body(body).show();
}
