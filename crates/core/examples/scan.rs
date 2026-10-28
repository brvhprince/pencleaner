//! `cargo run --release -p pencleaner-core --example scan -- ~/Sites`
//! Runs a Space Lens scan from the terminal and prints the biggest folders.

use pencleaner_core::{fsutil, progress::ScanControl, space_lens};
use std::path::PathBuf;

fn main() {
    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(fsutil::home_dir);
    let ctl = ScanControl::new();
    let tree = space_lens::scan(&root, &ctl).expect("scan failed");
    let s = tree.summary();
    println!(
        "{}: {:.2} GB, {} items, {} unreadable folders, {} ms",
        s.root,
        s.size as f64 / 1e9,
        s.items,
        s.denied,
        s.elapsed_ms
    );
    fn denied(n: &space_lens::DirNode, at: &std::path::Path) {
        let here = at.join(&*n.name);
        if n.denied {
            println!("unreadable: {}", here.display());
        }
        for c in &n.children {
            denied(c, &here);
        }
    }
    denied(&tree.root, root.parent().unwrap_or(&root));
    let listing = tree.list(&root, &fsutil::home_dir()).unwrap();
    for e in listing.entries.iter().take(15) {
        println!("{:>10.2} GB  {:?}  {}", e.size as f64 / 1e9, e.kind, e.name);
    }
}
