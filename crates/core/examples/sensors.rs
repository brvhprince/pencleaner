//! `cargo run -p pencleaner-core --example sensors`: CPU temperature and fans, five readings.
// Uses macOS-only parts of the engine.
#[cfg(target_os = "macos")]
mod imp {
    pub fn main() {
        #[cfg(target_arch = "x86_64")]
        println!("{:?}", pencleaner_core::sensors::cpu_keys());
        for _ in 0..5 {
            println!("{:?}", pencleaner_core::sensors::read());
            std::thread::sleep(std::time::Duration::from_secs(1));
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
