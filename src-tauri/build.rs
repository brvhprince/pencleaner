use std::process::Command;

fn main() {
    // Shown in the About dialog: which commit this is, and when it was built.
    let git = |args: &[&str]| {
        Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    println!("cargo:rustc-env=PENCLEANER_COMMIT={hash}{}", if dirty { "+" } else { "" });
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    println!("cargo:rustc-env=PENCLEANER_BUILT_AT={secs}");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    tauri_build::build()
}
