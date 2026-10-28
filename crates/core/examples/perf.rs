//! Read-only: `cargo run --release -p pencleaner-core --example perf`. Runs no tasks.
// Uses macOS-only parts of the engine.
#[cfg(target_os = "macos")]
mod imp {
    use pencleaner_core::{bytes_label as b, performance};

    pub fn main() {
        let r = performance::scan(false);
        let m = &r.memory;
        println!("Memory: {} of {} used (app {}, wired {}, compressed {}), cached {}, swap {}/{}, pressure {}",
            b(m.used), b(m.total), b(m.app), b(m.wired), b(m.compressed), b(m.cached), b(m.swap_used), b(m.swap_total), m.pressure);
        println!("\nTop processes:");
        for p in &r.processes { println!("  {:>9}  {} ({} proc)", b(p.bytes), p.name, p.processes); }
        println!("\nSnapshots: {} · purgeable {:?}", r.snapshots.len(), r.purgeable.map(b));
        println!("\nTasks:");
        for t in &r.tasks { println!("  [{}] {:<28} {}{}", if t.recommended { "x" } else { " " }, t.title, if t.admin { "(admin) " } else { "" }, t.reason.clone().unwrap_or_default()); }
        println!("\nItems ({}):", r.items.len());
        let mut ids: Vec<&str> = r.items.iter().map(|i| i.id.as_str()).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        println!("  unique ids: {} of {}", ids.len(), n);
        for i in &r.items {
            println!("  {:?} {:<34} {:<8} {:<8} {}{}{}", i.kind, i.name.chars().take(34).collect::<String>(),
                if i.enabled { "on" } else { "off" }, if i.running { "running" } else { "" },
                i.app_name.clone().or(i.developer.clone()).unwrap_or_default(),
                if i.controllable { "" } else { "  [settings only]" }, if i.broken { "  [BROKEN]" } else { "" });
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
