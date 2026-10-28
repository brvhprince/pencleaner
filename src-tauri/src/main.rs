// Prevents an additional console window on Windows in release; harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pencleaner_core::protection::watch;

fn main() {
    // launchd's background watcher: a quick headless check, no window, then exit.
    if std::env::args().any(|a| a == watch::FLAG) {
        watch::run_check(&watch::data_dir());
        return;
    }
    // Release check: proves the signed, hardened build can run Protection's engine.
    if std::env::args().any(|a| a == "--self-test") {
        let engine = pencleaner_core::protection::malware::Engine::load().expect("XProtect rules load");
        let mut scanner = engine.scanner();
        let hits = engine.scan_file(&mut scanner, std::path::Path::new("/usr/bin/true"));
        println!("self-test ok: {} XProtect rules, clean file → {} matches", engine.info.rules, hits.len());
        return;
    }
    pencleaner_lib::run()
}
