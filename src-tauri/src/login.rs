//! Open at Login, through macOS's own login-item system (SMAppService, macOS 13+). The
//! agent plist ships inside the app, so System Settings → Login Items lists it under
//! PenCleaner, and it starts PenCleaner with `--background`: menu bar only, no window.

#[cfg(target_os = "macos")]
use objc2_foundation::ns_string;
#[cfg(target_os = "macos")]
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use serde::Serialize;

pub const BACKGROUND_FLAG: &str = "--background";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStatus {
    /// Registered and allowed to run.
    pub enabled: bool,
    /// Registered, but switched off in System Settings → Login Items.
    pub needs_approval: bool,
    /// False on macOS 12 and in development builds (no bundled agent to register).
    pub available: bool,
}

#[cfg(target_os = "macos")]
fn service() -> Option<objc2::rc::Retained<SMAppService>> {
    // SMAppService arrived in macOS 13; on 12 the class doesn't exist.
    objc2::runtime::AnyClass::get(c"SMAppService")?;
    Some(unsafe { SMAppService::agentServiceWithPlistName(ns_string!("com.pen.cleaner.login.plist")) })
}

#[cfg(target_os = "macos")]
fn bundled() -> bool {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.parent()?.join("Library/LaunchAgents/com.pen.cleaner.login.plist"))).is_some_and(|p| p.exists())
}

#[cfg(target_os = "macos")]
pub fn status() -> LoginStatus {
    let Some(s) = service().filter(|_| bundled()) else {
        return LoginStatus { enabled: false, needs_approval: false, available: false };
    };
    let st = unsafe { s.status() };
    LoginStatus { enabled: st == SMAppServiceStatus::Enabled, needs_approval: st == SMAppServiceStatus::RequiresApproval, available: true }
}

#[cfg(target_os = "macos")]
pub fn set(on: bool) -> Result<LoginStatus, String> {
    let s = service().filter(|_| bundled()).ok_or("Open at Login works in the installed app on macOS 13 or later.")?;
    let r = unsafe { if on { s.registerAndReturnError() } else { s.unregisterAndReturnError() } };
    if let Err(e) = r {
        let st = status();
        // Unregistering something not registered, or registering what needs approval: not failures.
        if st.enabled == on || (on && st.needs_approval) {
            return Ok(st);
        }
        return Err(e.localizedDescription().to_string());
    }
    Ok(status())
}

#[cfg(target_os = "macos")]
pub fn open_settings() {
    if service().is_some() {
        unsafe { SMAppService::openSystemSettingsLoginItems() };
    }
}

/// Open at Login on Windows (a Run registry entry) comes with the Windows work.
#[cfg(windows)]
pub fn status() -> LoginStatus {
    LoginStatus { enabled: false, needs_approval: false, available: false }
}

#[cfg(windows)]
pub fn set(_on: bool) -> Result<LoginStatus, String> {
    Err("Open at Login isn't available on Windows yet.".into())
}

#[cfg(windows)]
pub fn open_settings() {}

/// launchd starts the agent right after registering; if PenCleaner is already open (it
/// is: the user just flipped the switch), that background copy must bow out. Holds an
/// exclusive lock for the life of the process; returns false if another copy has it.
pub fn claim_single_instance() -> bool {
    let dir = pencleaner_core::fsutil::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(f) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("instance.lock")) else { return true };
    match f.try_lock() {
        Ok(()) => {
            // Kept open on purpose: the lock lives as long as we do.
            std::mem::forget(f);
            true
        }
        Err(_) => false,
    }
}
