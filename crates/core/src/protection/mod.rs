//! Protection: on-demand malware scan (Apple's XProtect rules), suspicious background
//! items, privacy cleanup, app permissions and an opt-in watcher. Fully local; scans
//! never ask for a password (only removals of root-owned items do).

pub mod malware;
pub mod permissions;
pub mod persistence;
pub mod privacy;
pub mod watch;

use crate::fsutil::home_dir;
use crate::performance::items;
use crate::privileged;
use crate::progress::ScanControl;
use crate::remove::{RemoveMode, Remover};
use crate::safety;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionReport {
    pub rules: Option<malware::RulesInfo>,
    pub files_scanned: u64,
    pub detections: Vec<malware::Detection>,
    pub suspicious: Vec<persistence::Finding>,
    pub privacy: Vec<privacy::PrivacyItem>,
    pub permissions: Vec<permissions::Grant>,
    /// Why permissions couldn't be read (usually Full Disk Access).
    pub permissions_error: Option<String>,
    pub watcher: bool,
    pub alerts: Vec<watch::Alert>,
    pub errors: Vec<String>,
    pub elapsed_ms: u64,
    pub cancelled: bool,
}

pub fn scan(ctl: &ScanControl) -> ProtectionReport {
    let started = Instant::now();
    crate::fsutil::set_observer_io_policy();
    let mut errors = Vec::new();

    let ((malware_part, suspicious), (privacy, perms)) = rayon::join(
        || {
            rayon::join(
                || {
                    let engine = malware::Engine::load();
                    match engine {
                        Ok(e) => {
                            ctl.set_current(Path::new("Collecting files to check…"));
                            let files = malware::targets(&[]);
                            let hits = e.scan(&files, ctl);
                            (Some(e.info.clone()), files.len() as u64, hits, None)
                        }
                        Err(err) => (None, 0, vec![], Some(err)),
                    }
                },
                persistence::scan,
            )
        },
        || rayon::join(privacy::scan, permissions::scan),
    );
    let (rules, files_scanned, mut detections, err) = malware_part;
    errors.extend(err);
    detections.sort_by(|a, b| a.target.cmp(&b.target));
    detections.dedup_by(|a, b| a.target == b.target && a.rule == b.rule);
    let (permissions, permissions_error) = match perms {
        Ok(p) => (p, None),
        Err(e) => (vec![], Some(e)),
    };
    let dir = watch::data_dir();
    ProtectionReport {
        rules,
        files_scanned,
        detections,
        suspicious,
        privacy,
        permissions,
        permissions_error,
        watcher: watch::is_enabled(),
        alerts: watch::alerts(&dir),
        errors,
        elapsed_ms: started.elapsed().as_millis() as u64,
        cancelled: ctl.is_cancelled(),
    }
}

/// Moves a path to the Trash; root-owned ones through one password prompt. System and
/// protected paths are refused either way.
fn trash(path: &Path) -> Result<(), String> {
    let home = home_dir();
    if let Some(why) = safety::protection(path, &home) {
        return Err(format!("Protected: {why}"));
    }
    match Remover::default().remove(path, RemoveMode::Trash) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            let dest = home.join(".Trash").join(path.file_name().unwrap_or_default());
            privileged::run_as_admin(
                &[vec!["/bin/mv".into(), "-n".into(), path.to_string_lossy().into_owned(), dest.to_string_lossy().into_owned()]],
                "PenCleaner needs your password to remove a threat installed for all users.",
            )?;
            if path.exists() { Err("It's still there.".into()) } else { Ok(()) }
        }
        Err(e) => Err(e),
    }
}

impl ProtectionReport {
    /// Removes a detected threat: first every launch item that would restart it, then the
    /// file or app itself (to the Trash).
    pub fn remove_detection(&mut self, target: &str) -> Result<(), String> {
        let t = PathBuf::from(target);
        if !self.detections.iter().any(|d| d.target == target) {
            return Err("Not in the last scan".into());
        }
        let none = std::collections::HashSet::new();
        for item in items::from_plists(&none, &none) {
            if item.executable.as_deref().is_some_and(|e| Path::new(e).starts_with(&t)) {
                items::remove(&item)?;
            }
        }
        trash(&t)?;
        self.detections.retain(|d| d.target != target);
        Ok(())
    }

    /// Stops a suspicious background item and moves its plist to the Trash. The program
    /// stays unless it lives somewhere no installer should use.
    pub fn remove_suspicious(&mut self, id: &str) -> Result<(), String> {
        let f = self.suspicious.iter().find(|f| f.item.id == id).cloned().ok_or("Not in the last scan")?;
        items::remove(&f.item)?;
        if f.reasons.iter().any(|r| r.starts_with("Runs from")) {
            let _ = trash(Path::new(&f.program));
        }
        self.suspicious.retain(|x| x.item.id != id);
        Ok(())
    }
}
