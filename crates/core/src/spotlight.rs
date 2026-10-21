//! Spotlight metadata for single files. Access times are unreliable on macOS (and we turn
//! their updates off), so "last opened" comes from LaunchServices via Spotlight instead.

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType};
use core_foundation::date::{CFDate, CFDateRef};
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;
use std::path::Path;

#[repr(C)]
struct OpaqueMDItem(c_void);
type MDItemRef = *const OpaqueMDItem;

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn MDItemCreate(allocator: *const c_void, path: CFStringRef) -> MDItemRef;
    fn MDItemCopyAttribute(item: MDItemRef, name: CFStringRef) -> CFTypeRef;
    static kMDItemLastUsedDate: CFStringRef;
    static kMDItemDateAdded: CFStringRef;
    static kMDItemWhereFroms: CFStringRef;
}

/// Seconds between the Unix epoch and Core Foundation's (2001-01-01).
const CF_EPOCH: f64 = 978_307_200.0;

#[derive(Debug, Default, Clone)]
pub struct Meta {
    pub last_used: Option<i64>,
    pub added: Option<i64>,
    /// Where a download came from, e.g. "github.com".
    pub source: Option<String>,
}

fn copy_date(item: MDItemRef, key: CFStringRef) -> Option<i64> {
    unsafe {
        let v = MDItemCopyAttribute(item, key);
        if v.is_null() {
            return None;
        }
        if CFGetTypeID(v) != CFDate::type_id() {
            CFRelease(v);
            return None;
        }
        let date = CFDate::wrap_under_create_rule(v as CFDateRef);
        Some((date.abs_time() + CF_EPOCH) as i64)
    }
}

fn copy_source(item: MDItemRef) -> Option<String> {
    unsafe {
        let v = MDItemCopyAttribute(item, kMDItemWhereFroms);
        if v.is_null() {
            return None;
        }
        if CFGetTypeID(v) != CFArray::<CFString>::type_id() {
            CFRelease(v);
            return None;
        }
        let arr = CFArray::<CFString>::wrap_under_create_rule(v as CFArrayRef);
        // The first entry is the file URL; later ones are the referring page.
        let url = arr.iter().map(|s| s.to_string()).find(|s| s.contains("://"))?;
        Some(host_of(&url))
    }
}

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_owned()
}

pub fn meta(path: &Path) -> Meta {
    let cf_path = CFString::new(&path.to_string_lossy());
    unsafe {
        let item = MDItemCreate(std::ptr::null(), cf_path.as_concrete_TypeRef());
        if item.is_null() {
            return Meta::default();
        }
        let meta = Meta {
            last_used: copy_date(item, kMDItemLastUsedDate),
            added: copy_date(item, kMDItemDateAdded),
            source: copy_source(item),
        };
        CFRelease(item as CFTypeRef);
        meta
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_metadata_without_crashing() {
        // Calculator has been opened at some point on any Mac, but Spotlight may be off,
        // so only check that the call is safe and the host parser works.
        let _ = super::meta(std::path::Path::new("/System/Applications/Calculator.app"));
        let _ = super::meta(std::path::Path::new("/definitely/not/here"));
        assert_eq!(super::host_of("https://www.github.com/a/b?x"), "github.com");
    }
}
