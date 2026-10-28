//! `cargo run -p pencleaner-core --example sensors`: CPU temperature and fans, five readings.
fn main() {
    #[cfg(target_arch = "x86_64")]
    println!("{:?}", pencleaner_core::sensors::cpu_keys());
    for _ in 0..5 {
        println!("{:?}", pencleaner_core::sensors::read());
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
