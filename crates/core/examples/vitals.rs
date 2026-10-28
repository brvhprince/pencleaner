//! `cargo run -p pencleaner-core --example vitals`: two readings a second apart.
// Uses macOS-only parts of the engine.
#[cfg(target_os = "macos")]
mod imp {
    pub fn main() {
        let s = pencleaner_core::vitals::Sampler::default();
        s.read();
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("{:#?}", s.read());
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
