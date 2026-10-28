// Prevents an additional console window on Windows in release; harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pencleaner_core::protection::watch;

fn main() {
    // launchd's background watcher: a quick headless check, no window, then exit.
    if std::env::args().any(|a| a == watch::FLAG) {
        watch::run_check(&watch::data_dir());
        return;
    }
    pencleaner_lib::run()
}
