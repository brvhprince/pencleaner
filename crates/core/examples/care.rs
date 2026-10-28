//! `cargo run -p pencleaner-core --example care`: what scheduled Smart Care would clean
//! right now (read-only).
use pencleaner_core::{bytes_label, cleanup, progress::ScanControl, settings};

fn main() {
    let t = std::time::Instant::now();
    let mut r = cleanup::scan(&[cleanup::Category::System, cleanup::Category::Developer, cleanup::Category::Ai], &ScanControl::new());
    let dir = pencleaner_core::protection::watch::data_dir();
    r.apply_ignore(&settings::load(&dir).matcher());
    let ids = r.auto_ids();
    let mut total = 0;
    for g in &r.groups {
        let picked: Vec<_> = g.items.iter().filter(|i| ids.contains(&i.id)).collect();
        let size: u64 = picked.iter().map(|i| i.size).sum();
        total += size;
        println!("{:<16} {:>3} of {:>3} items  {:>10}", g.id, picked.len(), g.items.len(), bytes_label(size));
    }
    println!("would free {} (scan took {:?})", bytes_label(total), t.elapsed());
}
