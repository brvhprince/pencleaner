//! Perceptual fingerprints through macOS ImageIO, so HEIC, RAW-embedded previews and
//! everything else Photos understands works without bundling decoders.

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::{CFURLRef, CFURL};
use core_graphics::base::kCGImageAlphaNone;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::{CGContext, CGInterpolationQuality};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::image::CGImage;
use foreign_types::ForeignType;
use std::ffi::c_void;
use std::path::Path;

#[repr(C)]
struct OpaqueSource(c_void);
type CGImageSourceRef = *const OpaqueSource;

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageSourceCreateWithURL(url: CFURLRef, options: CFDictionaryRef) -> CGImageSourceRef;
    fn CGImageSourceCreateThumbnailAtIndex(src: CGImageSourceRef, index: usize, options: CFDictionaryRef) -> *mut core_graphics::sys::CGImage;
    fn CGImageSourceCopyPropertiesAtIndex(src: CGImageSourceRef, index: usize, options: CFDictionaryRef) -> CFDictionaryRef;
    fn CGImageSourceGetCount(src: CGImageSourceRef) -> usize;
    fn CGImageDestinationCreateWithData(
        data: core_foundation_sys::data::CFMutableDataRef,
        kind: CFStringRef,
        count: usize,
        options: CFDictionaryRef,
    ) -> *const c_void;
    fn CGImageDestinationAddImage(dest: *const c_void, image: *mut core_graphics::sys::CGImage, props: CFDictionaryRef);
    fn CGImageDestinationFinalize(dest: *const c_void) -> bool;
    static kCGImageSourceCreateThumbnailFromImageAlways: CFStringRef;
    static kCGImageSourceThumbnailMaxPixelSize: CFStringRef;
    static kCGImageSourceCreateThumbnailWithTransform: CFStringRef;
    static kCGImageSourceShouldCache: CFStringRef;
    static kCGImagePropertyPixelWidth: CFStringRef;
    static kCGImagePropertyPixelHeight: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

/// Rendered by WebKit, so the review grid can show them.
pub const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "tif", "tiff", "bmp"];

/// At or under this many differing bits, two images count as similar. Calibrated on real
/// screenshots: near-identical ones land at 1–8, same layout with other content at 11+,
/// unrelated images at 45+.
pub const SIMILAR_BITS: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub struct Fingerprint {
    /// Horizontal and vertical gradient hashes of a 9×9 grayscale thumbnail.
    pub hash: u128,
    pub width: u32,
    pub height: u32,
}

impl Fingerprint {
    pub fn distance(&self, other: &Fingerprint) -> u32 {
        (self.hash ^ other.hash).count_ones()
    }
}

fn key(k: CFStringRef) -> CFString {
    unsafe { CFString::wrap_under_get_rule(k) }
}

fn dimension(props: &CFDictionary<CFString, CFType>, k: CFStringRef) -> u32 {
    props
        .find(key(k))
        .and_then(|v| v.downcast::<CFNumber>())
        .and_then(|n| n.to_i64())
        .unwrap_or(0) as u32
}

pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let url = CFURL::from_path(path, false)?;
    unsafe {
        let src = CGImageSourceCreateWithURL(url.as_concrete_TypeRef(), std::ptr::null());
        if src.is_null() {
            return None;
        }
        let no_cache: CFDictionary<CFString, CFType> =
            CFDictionary::from_CFType_pairs(&[(key(kCGImageSourceShouldCache), CFBoolean::false_value().as_CFType())]);
        let props_ref = CGImageSourceCopyPropertiesAtIndex(src, 0, no_cache.as_concrete_TypeRef());
        let (width, height) = if props_ref.is_null() {
            (0, 0)
        } else {
            let props = CFDictionary::<CFString, CFType>::wrap_under_create_rule(props_ref);
            (dimension(&props, kCGImagePropertyPixelWidth), dimension(&props, kCGImagePropertyPixelHeight))
        };

        let opts: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
            (key(kCGImageSourceCreateThumbnailFromImageAlways), CFBoolean::true_value().as_CFType()),
            (key(kCGImageSourceCreateThumbnailWithTransform), CFBoolean::true_value().as_CFType()),
            (key(kCGImageSourceThumbnailMaxPixelSize), CFNumber::from(64i32).as_CFType()),
            (key(kCGImageSourceShouldCache), CFBoolean::false_value().as_CFType()),
        ]);
        let thumb = CGImageSourceCreateThumbnailAtIndex(src, 0, opts.as_concrete_TypeRef());
        CFRelease(src as *const c_void);
        if thumb.is_null() {
            return None;
        }
        let image = CGImage::from_ptr(thumb);

        let mut ctx = CGContext::create_bitmap_context(None, 9, 9, 8, 9, &CGColorSpace::create_device_gray(), kCGImageAlphaNone);
        ctx.set_interpolation_quality(CGInterpolationQuality::CGInterpolationQualityHigh);
        ctx.draw_image(CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(9.0, 9.0)), &image);
        let px = ctx.data();

        let mut hash = 0u128;
        let mut bit = 0;
        for y in 0..8 {
            for x in 0..8 {
                let here = px[y * 9 + x];
                if here < px[y * 9 + x + 1] {
                    hash |= 1 << bit;
                }
                if here < px[(y + 1) * 9 + x] {
                    hash |= 1 << (bit + 64);
                }
                bit += 1;
            }
        }
        Some(Fingerprint { hash, width, height })
    }
}

/// The app's icon as PNG bytes, about `px` wide. Reads the .icns named in Info.plist,
/// picking the smallest representation that is at least `px` so it stays sharp.
pub fn app_icon_png(app: &Path, px: u32) -> Option<Vec<u8>> {
    let info = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let dict = info.as_dictionary()?;
    let named = |k: &str| dict.get(k).and_then(|v| v.as_string()).map(str::to_owned);
    let resources = app.join("Contents/Resources");
    let candidates = [named("CFBundleIconFile"), named("CFBundleIconName"), Some("AppIcon".into())];
    let icns = candidates.into_iter().flatten().map(|n| {
        let p = resources.join(&n);
        if p.extension().is_some() { p } else { p.with_extension("icns") }
    }).find(|p| p.is_file())?;

    let url = CFURL::from_path(&icns, false)?;
    unsafe {
        let src = CGImageSourceCreateWithURL(url.as_concrete_TypeRef(), std::ptr::null());
        if src.is_null() {
            return None;
        }
        let count = CGImageSourceGetCount(src);
        let mut best: Option<(usize, u32)> = None;
        for i in 0..count {
            let props = CGImageSourceCopyPropertiesAtIndex(src, i, std::ptr::null());
            if props.is_null() {
                continue;
            }
            let props = CFDictionary::<CFString, CFType>::wrap_under_create_rule(props);
            let w = dimension(&props, kCGImagePropertyPixelWidth);
            let better = match best {
                None => true,
                Some((_, bw)) if bw < px => w > bw,
                Some((_, bw)) => w >= px && w < bw,
            };
            if better {
                best = Some((i, w));
            }
        }
        let opts: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
            (key(kCGImageSourceCreateThumbnailFromImageAlways), CFBoolean::true_value().as_CFType()),
            (key(kCGImageSourceThumbnailMaxPixelSize), CFNumber::from(px as i32).as_CFType()),
        ]);
        let thumb = CGImageSourceCreateThumbnailAtIndex(src, best.map_or(0, |b| b.0), opts.as_concrete_TypeRef());
        CFRelease(src as *const c_void);
        if thumb.is_null() {
            return None;
        }
        let image = CGImage::from_ptr(thumb);

        let data = core_foundation_sys::data::CFDataCreateMutable(std::ptr::null(), 0);
        let png = CFString::from_static_string("public.png");
        let dest = CGImageDestinationCreateWithData(data, png.as_concrete_TypeRef(), 1, std::ptr::null());
        if dest.is_null() {
            CFRelease(data as *const c_void);
            return None;
        }
        CGImageDestinationAddImage(dest, image.as_ptr(), std::ptr::null());
        let ok = CGImageDestinationFinalize(dest);
        CFRelease(dest);
        let data = core_foundation::data::CFData::wrap_under_create_rule(data as core_foundation_sys::data::CFDataRef);
        ok.then(|| data.bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn renders_app_icons_as_png() {
        let png = app_icon_png(Path::new("/System/Applications/Calculator.app"), 64).expect("Calculator has an icon");
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn resized_copies_match_and_mirrored_images_dont() {
        let dir = tempfile::tempdir().unwrap();
        let Some(source) = std::fs::read_dir("/System/Library/Desktop Pictures")
            .ok()
            .and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "heic")))
        else {
            return; // no sample image on this machine
        };
        let sips = |args: &[&str], out: &Path| {
            let ok = Command::new("/usr/bin/sips").args(args).arg(&source).arg("--out").arg(out).output().unwrap();
            assert!(ok.status.success());
        };
        let small = dir.path().join("small.jpg");
        let mirrored = dir.path().join("mirrored.jpg");
        sips(&["-s", "format", "jpeg", "-Z", "300"], &small);
        sips(&["-s", "format", "jpeg", "-Z", "300", "-f", "horizontal"], &mirrored);

        let original = fingerprint(&source).expect("decodes HEIC");
        assert!(original.width > 0 && original.height > 0);
        let d_small = original.distance(&fingerprint(&small).expect("decodes JPEG"));
        let d_mirror = original.distance(&fingerprint(&mirrored).unwrap());
        assert!(d_small <= SIMILAR_BITS, "resized copy distance {d_small}");
        assert!(d_mirror > SIMILAR_BITS, "mirrored distance {d_mirror}");
    }
}
