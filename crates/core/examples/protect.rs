//! Read-only: `cargo run --release -p pencleaner-core --example protect`. Removes nothing.
use pencleaner_core::progress::ScanControl;
use pencleaner_core::protection::{malware, permissions, persistence, privacy};
use std::time::Instant;

fn main() {
    if std::env::args().any(|a| a == "--privacy") {
        for i in privacy::scan() {
            println!("  {:<14} {:<26} {:>10} {}{}", i.owner, i.title, pencleaner_core::bytes_label(i.size), i.detail, if i.running { "  [open]" } else { "" });
        }
        match permissions::scan() {
            Ok(g) => {
                println!("\n{} permission grants, {} from removed apps", g.len(), g.iter().filter(|x| x.orphaned).count());
                for x in g.iter().filter(|x| x.orphaned || x.label == "Screen Recording" || x.label == "Full Disk Access") {
                    println!("  {:<18} {:<34} {}{}", x.label, x.app_name, if x.orphaned { "REMOVED APP " } else { "" }, if x.revocable { "" } else { "(path grant)" });
                }
            }
            Err(e) => println!("permissions: {e}"),
        }
        return;
    }
    let t = Instant::now();
    let findings = persistence::scan();
    println!("Background items: {} flagged in {:?}", findings.len(), t.elapsed());
    for f in &findings {
        println!("  {:?} {:<30} {:?}\n      program {}\n      {}", f.severity, f.item.name, f.signer, f.program, f.reasons.join("; "));
    }
    let t = Instant::now();
    let engine = malware::Engine::load().expect("rules");
    println!("\nXProtect v{:?}: {} rules, compiled in {:?}", engine.info.version, engine.info.rules, t.elapsed());
    let t = Instant::now();
    let files = malware::targets(&[]);
    let gathered = t.elapsed();
    let ctl = ScanControl::new();
    let hits = engine.scan(&files, &ctl);
    let p = ctl.snapshot();
    println!("Scanned {} files ({} MB) in {:?} (gathering {:?})", files.len(), p.bytes / 1_000_000, t.elapsed(), gathered);
    for h in &hits { println!("  MATCH {} {} → remove {}", h.rule, h.file, h.target); }
    if hits.is_empty() { println!("  No malware found."); }
}
