//! `cargo run --release -p pencleaner-core --example imgdist -- a.jpg b.heic …`
//! Prints pairwise perceptual-hash distances; used to calibrate imaging::SIMILAR_BITS.
use pencleaner_core::imaging::fingerprint;
fn main() {
    let paths: Vec<_> = std::env::args().skip(1).map(std::path::PathBuf::from).collect();
    let fps: Vec<_> = paths.iter().map(|p| fingerprint(p)).collect();
    for (i, a) in fps.iter().enumerate() {
        let row: Vec<String> = fps.iter().map(|b| match (a, b) { (Some(a), Some(b)) => format!("{:3}", a.distance(b)), _ => "  -".into() }).collect();
        println!("{:40} {}", paths[i].file_name().unwrap().to_string_lossy(), row.join(""));
    }
}
