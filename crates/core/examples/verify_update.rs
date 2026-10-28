//! `cargo run --release -p pencleaner-core --example verify_update -- /Applications/Maccy.app …`
//! Downloads each app's newest Sparkle release and runs every installer check
//! (signature, unpack, code signature, Team ID, Gatekeeper) without installing anything.
use pencleaner_core::updater::{appcast, os_version_for_examples, prepare};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `--impersonate A B`: treat B's release as an update for A. Must be refused.
    if args.first().map(String::as_str) == Some("--impersonate") {
        let (victim, other) = (Path::new(&args[1]), Path::new(&args[2]));
        let info = |a: &Path| plist::Value::from_file(a.join("Contents/Info.plist")).unwrap().into_dictionary().unwrap();
        let o = info(other);
        let feed = o.get("SUFeedURL").and_then(|v| v.as_string()).unwrap().to_owned();
        let bundle = o.get("CFBundleIdentifier").and_then(|v| v.as_string()).unwrap().to_owned();
        let xml = String::from_utf8(std::process::Command::new("/usr/bin/curl").args(["-fsSL", &feed]).output().unwrap().stdout).unwrap();
        let releases = appcast::parse(&xml);
        let best = appcast::best(&releases, &os_version_for_examples(), std::env::consts::ARCH).unwrap();
        match prepare(victim, best, None, &bundle, |_| {}) {
            Ok(_) => println!("ACCEPTED (bad!)"),
            Err(e) => println!("REFUSED: {e}"),
        }
        return;
    }
    for arg in args {
        let app = Path::new(&arg);
        let plist = plist::Value::from_file(app.join("Contents/Info.plist")).expect("Info.plist");
        let d = plist.as_dictionary().unwrap();
        let get = |k: &str| d.get(k).and_then(|v| v.as_string()).map(str::to_owned);
        let (bundle, feed, key) = (get("CFBundleIdentifier").unwrap(), get("SUFeedURL").expect("no feed"), get("SUPublicEDKey"));
        let xml = String::from_utf8(std::process::Command::new("/usr/bin/curl").args(["-fsSL", &feed]).output().unwrap().stdout).unwrap();
        let releases = appcast::parse(&xml);
        let Some(best) = appcast::best(&releases, &os_version_for_examples(), std::env::consts::ARCH) else {
            println!("{arg}: no usable release");
            continue;
        };
        let t = std::time::Instant::now();
        match prepare(app, best, key.as_deref(), &bundle, |s| eprint!("{s} ")) {
            Ok(p) => println!("\n{arg}: OK {} · team {:?} · EdDSA {} · {:.1}s", best.short_version.as_deref().unwrap_or(&best.version), p.team, if key.is_some() { "verified" } else { "n/a" }, t.elapsed().as_secs_f32()),
            Err(e) => println!("\n{arg}: REFUSED: {e}"),
        }
    }
}
