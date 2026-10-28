//! Dry run: `cargo run --release -p pencleaner-core --example apps`. Never removes anything.
// Uses macOS-only parts of the engine.
#[cfg(target_os = "macos")]
mod imp {
    use pencleaner_core::{applications, progress::ScanControl};

    fn mb(b: u64) -> String {
        if b >= 1_000_000_000 { format!("{:.2} GB", b as f64 / 1e9) } else { format!("{:.1} MB", b as f64 / 1e6) }
    }

    pub fn main() {
        let ctl = ScanControl::new();
        let r = applications::scan(&ctl);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let days = |t: Option<i64>| t.map_or("never".into(), |t| format!("{}d", (now - t) / 86400));
        println!("{} apps, {} leftovers, {} installers in {} ms", r.apps.len(), r.leftovers.len(), r.installers.len(), r.elapsed_ms);
        println!("\n== Apps (app + support files)");
        for a in r.apps.iter().take(14) {
            let admin = a.related.iter().filter(|f| f.admin).count();
            println!("  {:>9} + {:>9}  {:<28} used {:<6} {} files{}{}", mb(a.size), mb(a.related_size), a.name.chars().take(28).collect::<String>(), days(a.last_used), a.related.len(),
                if admin > 0 { format!(", {admin} need admin") } else { String::new() }, if a.running { " [running]" } else { "" });
        }
        let zoom = r.apps.iter().find(|a| a.bundle_id == "us.zoom.xos");
        if let Some(z) = zoom { println!("\n  zoom.us files:"); for f in &z.related { println!("    {:<20} {:>9} {}{}", f.label, mb(f.size), f.path, if f.admin { " [admin]" } else { "" }); } }
        println!("\n== Leftovers, total {}", mb(r.leftovers.iter().map(|l| l.size).sum()));
        for l in r.leftovers.iter().take(25) {
            let labels: Vec<_> = l.files.iter().map(|f| f.label).collect();
            println!("  {:>9}  {:<45} touched {:<6} {}", mb(l.size), l.id, days(l.last_activity), labels.join(", "));
        }
        println!("\n== Installers, total {}", mb(r.installers.iter().map(|i| i.size).sum()));
        for i in r.installers.iter().take(10) {
            println!("  {:>9}  {:<40} {} {}", mb(i.size), i.name, i.installed_app.as_deref().map(|a| format!("[{a} installed]")).unwrap_or_default(), if i.recommended { "*" } else { "" });
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    imp::main()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example is macOS-only.");
}
