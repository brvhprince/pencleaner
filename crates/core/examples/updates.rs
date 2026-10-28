//! Read-only: `cargo run --release -p pencleaner-core --example updates`. Installs nothing.
// Uses macOS-only parts of the engine.
#[cfg(target_os = "macos")]
mod imp {
    use pencleaner_core::{bytes_label, updater};

    pub fn main() {
        let cache = std::env::temp_dir().join("pencleaner-catalog");
        let r = updater::scan(Some(&cache), |_| {});
        println!("{} updates · {} apps checked · {} unchecked · {} ms", r.updates.len(), r.checked, r.unchecked.len(), r.elapsed_ms);
        for u in &r.updates {
            println!(
                "  {:<9?} {:<28} {:>12} → {:<12} {:>9} {}{}{}",
                u.source, u.name.chars().take(28).collect::<String>(), u.installed, u.latest,
                u.size.map(bytes_label).unwrap_or_default(),
                if u.one_click { "one-click" } else { "manual" },
                if u.signed { " signed" } else { "" },
                u.requires.as_deref().map(|r| format!(" [{r}]")).unwrap_or_default()
            );
        }
        for (src, e) in &r.errors { println!("  ERROR {src}: {e}"); }
        println!("unchecked: {}", r.unchecked.join(", "));
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
