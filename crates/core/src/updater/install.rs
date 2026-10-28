//! Installing updates. Sparkle downloads are only put in place after every check passes:
//! the developer's EdDSA signature (when the app publishes a key), a valid code signature
//! from the same Team ID as the installed copy, the same bundle id, and Gatekeeper.
//! The old copy goes to the Trash, so an update can be rolled back by hand.

use super::{appcast::Release, brew_cmd, enough_space, fetch, Plan, Update};
use crate::apps;
use crate::privileged;
use crate::remove::{RemoveMode, Remover};
use base64::Engine;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| e.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text)
    } else {
        Err(text.trim().lines().last().unwrap_or("failed").to_owned())
    }
}

/// The signing team, from `codesign -dv` ("TeamIdentifier=ABCDE12345").
pub(crate) fn team_id(app: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(app).output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    text.lines()
        .find_map(|l| l.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "not set")
        .map(str::to_owned)
}

pub(crate) fn verify_ed25519(public_key_b64: &str, signature_b64: &str, data: &[u8]) -> Result<(), String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let key: [u8; 32] = b64.decode(public_key_b64.trim()).ok().and_then(|k| k.try_into().ok()).ok_or("The app's public key is malformed")?;
    let sig: [u8; 64] = b64.decode(signature_b64.trim()).ok().and_then(|s| s.try_into().ok()).ok_or("The update's signature is malformed")?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&key).map_err(|_| "The app's public key is invalid")?;
    key.verify_strict(data, &ed25519_dalek::Signature::from_bytes(&sig))
        .map_err(|_| "The download doesn't match the developer's signature".into())
}

/// Removes its directory when dropped, whatever happened.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn find_app(dir: &Path, bundle_id: &str, depth: usize) -> Option<PathBuf> {
    for e in fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "app") {
            if apps::read_bundle(&p).is_some_and(|a| a.bundle_id.eq_ignore_ascii_case(bundle_id)) {
                return Some(p);
            }
        } else if depth > 0 && e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()) {
            if let Some(found) = find_app(&p, bundle_id, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn unpack(archive: &Path, url: &str, into: &Path) -> Result<(), String> {
    let lower = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    let a = archive.to_str().ok_or("bad path")?;
    let d = into.to_str().ok_or("bad path")?;
    if lower.ends_with(".zip") {
        run("/usr/bin/ditto", &["-x", "-k", a, d])?;
    } else if lower.ends_with(".dmg") {
        let mount = into.join(".mount");
        fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
        let m = mount.to_str().ok_or("bad path")?;
        run("/usr/bin/hdiutil", &["attach", "-nobrowse", "-readonly", "-noautoopen", "-noverify", "-mountpoint", m, a])?;
        // Copy the .app out, then always detach.
        let copied = (|| {
            for e in fs::read_dir(&mount).map_err(|e| e.to_string())?.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "app") && !e.file_type().is_ok_and(|t| t.is_symlink()) {
                    let dest = into.join(p.file_name().unwrap_or_default());
                    run("/usr/bin/ditto", &[p.to_str().ok_or("bad path")?, dest.to_str().ok_or("bad path")?])?;
                }
            }
            Ok::<(), String>(())
        })();
        let _ = run("/usr/bin/hdiutil", &["detach", "-force", m]);
        copied?;
    } else {
        run("/usr/bin/tar", &["-xf", a, "-C", d])?;
    }
    Ok(())
}

/// Moves `new` into `old`'s place. The old copy is first renamed beside itself (so a
/// failure can be undone), then handed to `dispose` (the Trash, outside tests).
fn replace(old: &Path, new: &Path, dispose: impl Fn(&Path)) -> Result<(), String> {
    let parent = old.parent().ok_or("bad path")?;
    let backup = parent.join(format!(".{}.pencleaner-old", old.file_name().unwrap_or_default().to_string_lossy()));
    let direct = (|| -> std::io::Result<()> {
        fs::rename(old, &backup)?;
        if let Err(e) = fs::rename(new, old) {
            // Cross-volume or permission trouble: put the original back.
            let _ = fs::rename(&backup, old);
            return Err(e);
        }
        Ok(())
    })();
    match direct {
        Ok(()) => {
            dispose(&backup);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Root-owned app: one password prompt does the same dance.
            let s = |p: &Path| p.to_string_lossy().into_owned();
            let trash = crate::fsutil::home_dir().join(".Trash").join(format!("{} (old)", old.file_name().unwrap_or_default().to_string_lossy()));
            privileged::run_as_admin(
                &[
                    vec!["/bin/mv".into(), s(old), s(&backup)],
                    vec!["/usr/bin/ditto".into(), s(new), s(old)],
                    vec!["/bin/mv".into(), "-n".into(), s(&backup), s(&trash)],
                ],
                "PenCleaner needs your password to update an app installed for all users.",
            )?;
            if old.exists() {
                Ok(())
            } else {
                Err("The update didn't land; the original was kept.".into())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// A downloaded, verified, unpacked update, ready to swap in. Its scratch folder is
/// deleted when this is dropped.
pub struct Prepared {
    _scratch: Scratch,
    pub app: PathBuf,
    pub team: Option<String>,
}

/// Everything short of installing: download, signature, unpack, code signature, same
/// Team ID, Gatekeeper. Safe to run against any installed app.
pub fn prepare(app: &Path, release: &Release, ed_key: Option<&str>, bundle_id: &str, mut step: impl FnMut(&str)) -> Result<Prepared, String> {
    if !enough_space(release.length.unwrap_or(0)) {
        return Err("Not enough free space to download and unpack the update.".into());
    }
    let old_team = team_id(app);
    if ed_key.is_none() && old_team.is_none() {
        return Err("This app is neither signed by a developer nor publishes an update key, so its updates can't be verified.".into());
    }

    let dir = std::env::temp_dir().join(format!("pencleaner-update-{}-{}", std::process::id(), bundle_id));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let scratch = Scratch(dir);

    step("Downloading…");
    let data = fetch(&release.url, 1800)?;
    if release.length.is_some_and(|n| n != data.len() as u64) {
        return Err("The download is incomplete.".into());
    }
    if let Some(key) = ed_key {
        step("Checking the developer's signature…");
        let sig = release.ed_signature.as_deref().ok_or("The app requires signed updates, but this one isn't signed")?;
        verify_ed25519(key, sig, &data)?;
    }
    let archive = scratch.0.join("download");
    fs::write(&archive, &data).map_err(|e| e.to_string())?;
    drop(data);

    step("Unpacking…");
    let unpacked = scratch.0.join("unpacked");
    fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    unpack(&archive, &release.url, &unpacked)?;
    let new = find_app(&unpacked, bundle_id, 2).ok_or("The download doesn't contain this app")?;

    step("Verifying…");
    let new_path = new.to_str().ok_or("bad path")?;
    run("/usr/bin/codesign", &["--verify", "--deep", "--strict", new_path]).map_err(|e| format!("Code signature check failed: {e}"))?;
    let new_team = team_id(&new);
    if old_team.is_some() && new_team != old_team {
        return Err(format!(
            "Signed by a different developer ({} instead of {}). Not installed.",
            new_team.as_deref().unwrap_or("nobody"),
            old_team.as_deref().unwrap_or("?")
        ));
    }
    if new_team.is_some() {
        run("/usr/sbin/spctl", &["--assess", "--type", "execute", new_path]).map_err(|e| format!("Gatekeeper rejected it: {e}"))?;
    }

    Ok(Prepared { _scratch: scratch, app: new, team: new_team })
}

fn sparkle(u: &Update, release: &Release, ed_key: Option<&str>, bundle_id: &str, mut step: impl FnMut(&str)) -> Result<String, String> {
    let app = PathBuf::from(u.app_path.as_deref().ok_or("No app to update")?);
    if !u.one_click {
        run("/usr/bin/open", &[app.to_str().ok_or("bad path")?])?;
        return Ok("Opened the app so it can update itself.".into());
    }
    if apps::running_bundle_ids().contains(&bundle_id.to_ascii_lowercase()) {
        return Err(format!("{} is running. Quit it, then update.", u.name));
    }
    let ready = prepare(&app, release, ed_key, bundle_id, &mut step)?;
    step("Installing…");
    replace(&app, &ready.app, |backup| {
        if Remover::default().remove(backup, RemoveMode::Trash).is_err() {
            let _ = fs::remove_dir_all(backup);
        }
    })?;
    Ok(format!("Updated to {}. The old version is in the Trash.", u.latest))
}

/// Installs one update, reporting steps. Returns what happened, for people.
pub fn install(u: &Update, step: impl FnMut(&str)) -> Result<String, String> {
    if let Some(r) = &u.requires {
        return Err(r.clone());
    }
    match &u.plan {
        Plan::AppStore { track_id } => {
            run("/usr/bin/open", &[&format!("macappstore://apps.apple.com/app/id{track_id}")])?;
            Ok("Opened in the App Store. Click Update there.".into())
        }
        Plan::Sparkle { release, ed_key, bundle_id } => sparkle(u, release, ed_key.as_deref(), bundle_id, step),
        Plan::Homebrew { token } => {
            if !u.one_click {
                return Err(u.action_note.clone().unwrap_or_default());
            }
            if u.running {
                return Err(format!("{} is running. Quit it, then update.", u.name));
            }
            let mut step = step;
            step("Running brew upgrade…");
            brew_cmd(&["upgrade", "--cask", token])?;
            Ok(format!("Updated to {} with Homebrew.", u.latest))
        }
        Plan::Open { url } => {
            let target = url.clone().or_else(|| u.app_path.clone()).ok_or("Nothing to open")?;
            run("/usr/bin/open", &[&target])?;
            Ok(if url.is_some() { "Opened the developer's website.".into() } else { format!("Opened {}. Use its Check for Updates.", u.name) })
        }
        Plan::None => Err("Nothing to install".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn verifies_sparkle_signatures() {
        let b64 = base64::engine::general_purpose::STANDARD;
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let data = b"the update archive";
        let sig = b64.encode(key.sign(data).to_bytes());
        let public = b64.encode(key.verifying_key().to_bytes());
        assert!(verify_ed25519(&public, &sig, data).is_ok());
        assert!(verify_ed25519(&public, &sig, b"tampered archive").is_err());
        assert!(verify_ed25519("bad", &sig, data).is_err());
    }

    #[test]
    fn reads_team_ids() {
        // Apple's own apps report no team id; that's the "not set" path.
        assert_eq!(team_id(Path::new("/System/Applications/Calculator.app")), None);
    }

    #[test]
    fn replace_swaps_and_keeps_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("Thing.app");
        let new = dir.path().join("new/Thing.app");
        fs::create_dir_all(old.join("Contents")).unwrap();
        fs::write(old.join("Contents/v"), "1").unwrap();
        fs::create_dir_all(new.join("Contents")).unwrap();
        fs::write(new.join("Contents/v"), "2").unwrap();
        // Delete rather than trash, so tests never touch the user's Trash.
        replace(&old, &new, |b| fs::remove_dir_all(b).unwrap()).unwrap();
        assert_eq!(fs::read_to_string(old.join("Contents/v")).unwrap(), "2");
        assert!(!dir.path().join(".Thing.app.pencleaner-old").exists());
    }
}
