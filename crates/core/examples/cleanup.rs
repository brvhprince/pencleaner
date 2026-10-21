//! Dry run: `cargo run --release -p pencleaner-core --example cleanup [system|developer|ai|mail|trash]`
//! Prints what Cleanup would offer. Never removes anything.

use pencleaner_core::cleanup::{self, Category};
use pencleaner_core::progress::ScanControl;

fn gb(b: u64) -> f64 {
    b as f64 / 1e9
}

fn main() {
    let cats: Vec<Category> = match std::env::args().nth(1).as_deref() {
        Some("system") => vec![Category::System],
        Some("developer") => vec![Category::Developer],
        Some("ai") => vec![Category::Ai],
        Some("mail") => vec![Category::Mail],
        Some("trash") => vec![Category::Trash],
        _ => cleanup::ALL.to_vec(),
    };
    let ctl = ScanControl::new();
    let report = cleanup::scan(&cats, &ctl);
    let (mut total, mut picked) = (0, 0);
    for g in &report.groups {
        let sel: u64 = g.items.iter().filter(|i| i.recommended).map(|i| i.size).sum();
        total += g.size;
        picked += sel;
        println!("\n== {:?} / {} — {:.2} GB ({:.2} GB pre-selected){}", g.category, g.title, gb(g.size), gb(sel), g.note.as_deref().map(|n| format!("  [{n}]")).unwrap_or_default());
        for i in g.items.iter().take(12) {
            println!(
                "  {} {:>8.2} GB  {:<44} {}{}",
                if i.recommended { "[x]" } else { "[ ]" },
                gb(i.size),
                i.name.chars().take(44).collect::<String>(),
                i.detail.as_deref().unwrap_or("").chars().take(70).collect::<String>(),
                i.warning.as_deref().map(|_| "  ⚠").unwrap_or("")
            );
        }
        if g.items.len() > 12 {
            println!("  … {} more", g.items.len() - 12);
        }
    }
    println!("\nTotal {:.2} GB, pre-selected {:.2} GB, {} ms, FDA {}", gb(total), gb(picked), report.elapsed_ms, report.full_disk_access);
}
