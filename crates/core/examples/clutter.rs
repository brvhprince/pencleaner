//! Dry run: `cargo run --release -p pencleaner-core --example clutter [folder]`. Never removes anything.
use pencleaner_core::{clutter, fsutil, progress::ScanControl};

fn mb(b: u64) -> String {
    if b >= 1_000_000_000 { format!("{:.2} GB", b as f64 / 1e9) } else { format!("{:.1} MB", b as f64 / 1e6) }
}

fn main() {
    let root = std::env::args().nth(1).map(std::path::PathBuf::from).unwrap_or_else(fsutil::home_dir);
    let ctl = ScanControl::new();
    let r = clutter::scan(&root, &ctl);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let age = |t: Option<i64>| t.map_or("never opened".into(), |t| format!("{}d ago", (now - t) / 86400));
    println!("{} files in {} ms", r.files_scanned, r.elapsed_ms);
    println!("\n== Large ({}), total {}", r.large.len(), mb(r.large.iter().map(|f| f.size).sum()));
    for f in r.large.iter().take(10) {
        println!("  {:>9}  {:<45} used {}", mb(f.size), f.name.chars().take(45).collect::<String>(), age(f.last_used));
    }
    println!("\n== Duplicates ({} sets), reclaimable {}", r.duplicates.len(), mb(r.duplicates.iter().map(|s| s.reclaimable).sum()));
    for s in r.duplicates.iter().take(8) {
        println!("  {:>9} reclaimable, {} copies, keep {}", mb(s.reclaimable), s.files.len(), s.keep.replace(&*fsutil::home_dir().to_string_lossy(), "~"));
    }
    println!("\n== Similar images ({} sets), reclaimable {}", r.similar.len(), mb(r.similar.iter().map(|s| s.reclaimable).sum()));
    for s in r.similar.iter().take(6) {
        let names: Vec<_> = s.files.iter().map(|f| f.name.chars().take(28).collect::<String>()).collect();
        println!("  {:>9}  {}", mb(s.reclaimable), names.join(" | "));
    }
    println!("\n== Downloads ({}), total {}", r.downloads.len(), mb(r.downloads.iter().map(|f| f.size).sum()));
    for f in r.downloads.iter().take(8) {
        println!("  {:>9}  {:?} {:<38} from {:?}", mb(f.size), f.kind, f.name.chars().take(38).collect::<String>(), f.source);
    }
}
