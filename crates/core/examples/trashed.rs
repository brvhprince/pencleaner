//! `cargo run -p pencleaner-core --example trashed [folder]`: leftovers of each app in the
//! Trash, or in `folder` (read-only).
fn main() {
    let trash = std::env::args().nth(1).map(Into::into).unwrap_or_else(|| pencleaner_core::fsutil::home_dir().join(".Trash"));
    for e in std::fs::read_dir(trash).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "app")) {
        let t0 = std::time::Instant::now();
        match pencleaner_core::applications::leftovers_of_trashed(&e.path()) {
            Some(t) => println!("{}: {} files, {} bytes ({:?})", t.name, t.files.len(), t.size, t0.elapsed()),
            None => println!("{}: nothing to offer ({:?})", e.path().display(), t0.elapsed()),
        }
    }
}
