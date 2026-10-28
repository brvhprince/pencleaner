//! App permissions: who may use the camera, microphone, screen, files and so on, read
//! from macOS's TCC databases (needs Full Disk Access) and revoked with `tccutil`.

use crate::apps;
use crate::fsutil::home_dir;
use crate::privileged;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// PenCleaner's bundle id.
const SELF_ID: &str = "com.pen.cleaner";
const SYSTEM_DB: &str = "/Library/Application Support/com.apple.TCC/TCC.db";

/// (TCC service, label, System Settings anchor). Internal iCloud services are left out.
const SERVICES: &[(&str, &str, &str)] = &[
    ("kTCCServiceCamera", "Camera", "Privacy_Camera"),
    ("kTCCServiceMicrophone", "Microphone", "Privacy_Microphone"),
    ("kTCCServiceScreenCapture", "Screen Recording", "Privacy_ScreenCapture"),
    ("kTCCServiceAccessibility", "Accessibility", "Privacy_Accessibility"),
    ("kTCCServiceListenEvent", "Input Monitoring", "Privacy_ListenEvent"),
    ("kTCCServiceSystemPolicyAllFiles", "Full Disk Access", "Privacy_AllFiles"),
    ("kTCCServiceAppleEvents", "Automation", "Privacy_Automation"),
    ("kTCCServiceSystemPolicyAppBundles", "App Management", "Privacy_AppBundles"),
    ("kTCCServicePhotos", "Photos", "Privacy_Photos"),
    ("kTCCServiceAddressBook", "Contacts", "Privacy_Contacts"),
    ("kTCCServiceCalendar", "Calendars", "Privacy_Calendars"),
    ("kTCCServiceReminders", "Reminders", "Privacy_Reminders"),
    ("kTCCServiceSystemPolicyDocumentsFolder", "Documents Folder", "Privacy_FilesAndFolders"),
    ("kTCCServiceSystemPolicyDesktopFolder", "Desktop Folder", "Privacy_FilesAndFolders"),
    ("kTCCServiceSystemPolicyDownloadsFolder", "Downloads Folder", "Privacy_FilesAndFolders"),
    ("kTCCServiceSystemPolicyRemovableVolumes", "Removable Volumes", "Privacy_FilesAndFolders"),
    ("kTCCServiceSystemPolicyNetworkVolumes", "Network Volumes", "Privacy_FilesAndFolders"),
    ("kTCCServiceBluetoothAlways", "Bluetooth", "Privacy_Bluetooth"),
    ("kTCCServiceMediaLibrary", "Media & Apple Music", "Privacy_Media"),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    pub id: String,
    /// TCC service name without the "kTCCService" prefix, as tccutil wants it.
    pub service: String,
    pub label: String,
    pub settings_anchor: String,
    /// Bundle id, or a program path for command-line tools.
    pub client: String,
    pub app_name: String,
    pub app_path: Option<String>,
    /// For Automation: the app it may control.
    pub target: Option<String>,
    pub granted: Option<i64>,
    /// The app isn't on this Mac any more.
    pub orphaned: bool,
    /// Revocable from PenCleaner: bundle-id grants of installed apps. (tccutil can't reset
    /// an app that's gone, and path grants need System Settings.)
    pub revocable: bool,
    /// Bound to the app's code signature, so no other app can inherit it. True for
    /// almost every grant; what makes a removed app's leftover harmless.
    pub locked_to_signer: bool,
    /// PenCleaner's own grant: revoking it would cripple the app, so it isn't offered.
    pub is_self: bool,
    /// Lives in the system database: revoking asks for the administrator password.
    pub system: bool,
}

#[derive(Deserialize)]
struct Row {
    service: String,
    client: String,
    client_type: i64,
    auth_value: i64,
    last_modified: Option<i64>,
    indirect_object_identifier: Option<String>,
    has_csreq: i64,
}

fn read(db: &Path) -> Result<Vec<Row>, String> {
    let out = Command::new("/usr/bin/sqlite3")
        .args(["-readonly", "-json"])
        .arg(db)
        .arg("SELECT service, client, client_type, auth_value, last_modified, indirect_object_identifier, csreq IS NOT NULL AS has_csreq FROM access")
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("macOS's permission database isn't readable. Grant Full Disk Access.".into());
    }
    if out.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(vec![]);
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

pub fn scan() -> Result<Vec<Grant>, String> {
    let user_db = home_dir().join("Library/Application Support/com.apple.TCC/TCC.db");
    let mut rows: Vec<(Row, bool)> = read(&user_db)?.into_iter().map(|r| (r, false)).collect();
    rows.extend(read(Path::new(SYSTEM_DB)).unwrap_or_default().into_iter().map(|r| (r, true)));

    let mut paths: HashMap<String, PathBuf> = HashMap::new();
    for a in apps::spotlight_apps().into_iter().chain(apps::installed()) {
        paths.entry(a.bundle_id.to_ascii_lowercase()).or_insert(a.path);
    }
    let running = apps::running_bundle_ids();
    let name_of = |id: &str| -> Option<String> {
        let p = paths.get(&id.to_ascii_lowercase())?;
        apps::read_bundle(p).map(|a| a.name)
    };

    let mut seen = HashSet::new();
    let mut grants: Vec<Grant> = rows
        .into_iter()
        .filter(|(r, _)| r.auth_value == 2 || r.auth_value == 3)
        .filter_map(|(r, system)| {
            let (_, label, anchor) = SERVICES.iter().find(|(s, _, _)| *s == r.service)?;
            let by_path = r.client_type == 1;
            let target = r.indirect_object_identifier.filter(|t| t != "UNUSED" && !t.is_empty());
            let id = format!("{}|{}|{}", r.service, r.client, target.as_deref().unwrap_or(""));
            if !seen.insert(id.clone()) {
                return None;
            }
            let app_path = if by_path {
                let p = PathBuf::from(&r.client);
                let app = p.ancestors().filter(|a| a.extension().is_some_and(|e| e == "app")).last().map(Path::to_path_buf);
                Some(app.unwrap_or(p))
            } else {
                paths.get(&r.client.to_ascii_lowercase()).cloned()
            };
            let installed = app_path.as_ref().is_some_and(|p| p.exists()) || running.contains(&r.client.to_ascii_lowercase());
            let app_name = if by_path {
                app_path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| r.client.clone())
            } else {
                name_of(&r.client).unwrap_or_else(|| r.client.clone())
            };
            Some(Grant {
                id,
                service: r.service.trim_start_matches("kTCCService").to_owned(),
                label: (*label).into(),
                settings_anchor: (*anchor).into(),
                target: target.map(|t| name_of(&t).unwrap_or(t)),
                app_path: app_path.filter(|p| p.exists()).map(|p| p.to_string_lossy().into_owned()),
                granted: r.last_modified,
                orphaned: !installed,
                revocable: !by_path && installed && !r.client.eq_ignore_ascii_case(SELF_ID),
                is_self: r.client.eq_ignore_ascii_case(SELF_ID),
                locked_to_signer: r.has_csreq == 1,
                system,
                client: r.client,
                app_name,
            })
        })
        .collect();
    let order = |g: &Grant| SERVICES.iter().position(|(s, _, _)| s.trim_start_matches("kTCCService") == g.service).unwrap_or(99);
    grants.sort_by_key(|g| (order(g), g.app_name.to_lowercase()));
    Ok(grants)
}

/// Revokes with `tccutil reset <Service> <bundle id>`. That clears the app's choice, so
/// macOS asks again next time it wants access. System-wide grants need the password.
pub fn revoke(grant: &Grant) -> Result<(), String> {
    if grant.is_self {
        return Err("That's PenCleaner's own permission; it needs it to work.".into());
    }
    if grant.orphaned {
        return Err("The app is gone, so macOS can't reset this. It's harmless: the permission is locked to that app's signature.".into());
    }
    if !grant.revocable {
        return Err("Granted to a command-line tool; switch it off in System Settings.".into());
    }
    let argv = vec!["/usr/bin/tccutil".to_string(), "reset".into(), grant.service.clone(), grant.client.clone()];
    let out = Command::new(&argv[0]).args(&argv[1..]).output().map_err(|e| e.to_string())?;
    let still_there = || scan().is_ok_and(|all| all.iter().any(|g| g.id == grant.id));
    if out.status.success() && !still_there() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    // A password can't fix an unknown app, and user-level grants never need one.
    if err.contains("No such bundle identifier") || !grant.system {
        return Err(if err.is_empty() { "macOS kept the permission. Remove it in System Settings.".into() } else { err });
    }
    privileged::run_as_admin(&[argv], "PenCleaner needs your password to remove a permission granted for all users.")?;
    if still_there() {
        Err("macOS kept the permission. Remove it in System Settings.".into())
    } else {
        Ok(())
    }
}

pub fn settings_url(anchor: &str) -> String {
    format!("x-apple.systempreferences:com.apple.preference.security?{anchor}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_grants_when_permitted() {
        // Needs Full Disk Access; on machines without it the error must be readable.
        match super::scan() {
            Ok(grants) => assert!(grants.iter().all(|g| !g.label.is_empty())),
            Err(e) => assert!(e.contains("Full Disk Access")),
        }
    }
}
