//! `cargo run -p pencleaner-core --example vitals`: two readings a second apart.
fn main() {
    let s = pencleaner_core::vitals::Sampler::default();
    s.read();
    std::thread::sleep(std::time::Duration::from_secs(1));
    println!("{:#?}", s.read());
}
