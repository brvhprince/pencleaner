//! Windows stand-in for `imaging`. Similar-image fingerprints need a decoder (ImageIO on
//! the Mac); until Windows has one, no fingerprints means My Clutter's Similar Images
//! section stays empty rather than guessing.

use std::path::Path;

pub const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "tif", "tiff", "bmp"];

/// See the macOS module: at or under this many differing bits, images count as similar.
pub const SIMILAR_BITS: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub struct Fingerprint {
    pub hash: u128,
    pub width: u32,
    pub height: u32,
}

impl Fingerprint {
    pub fn distance(&self, other: &Fingerprint) -> u32 {
        (self.hash ^ other.hash).count_ones()
    }
}

pub fn fingerprint(_path: &Path) -> Option<Fingerprint> {
    None
}

/// App icons come from the uninstaller's own source on Windows.
pub fn app_icon_png(_app: &Path, _px: u32) -> Option<Vec<u8>> {
    None
}
