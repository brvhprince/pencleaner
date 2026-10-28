//! Time Machine's local APFS snapshots. macOS takes one an hour while Time Machine is on
//! and releases them under disk pressure, which is why they show up as "purgeable".

use core_foundation::base::{CFType, TCFType};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use serde::Serialize;
use std::ffi::c_void;
use std::path::Path;
use std::process::Command;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFURLCopyResourcePropertyForKey(
        url: core_foundation::url::CFURLRef,
        key: core_foundation::string::CFStringRef,
        value: *mut *const c_void,
        error: *mut *const c_void,
    ) -> u8;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Date stamp tmutil uses to address it, e.g. "2026-10-22-065608".
    pub id: String,
    pub created: Option<i64>,
}

/// "com.apple.TimeMachine.2026-10-22-065608.local" → "2026-10-22-065608"
fn stamp(name: &str) -> Option<&str> {
    name.strip_prefix("com.apple.TimeMachine.")?.strip_suffix(".local")
}

/// Days since the epoch for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Snapshot stamps are local time; close enough for "3 days ago".
fn parse_stamp(s: &str) -> Option<i64> {
    let p: Vec<&str> = s.split('-').collect();
    let [y, mo, d, hms] = p.as_slice() else { return None };
    let (h, mi, se) = (hms.get(0..2)?, hms.get(2..4)?, hms.get(4..6)?);
    let n = |v: &str| v.parse::<i64>().ok();
    Some(days_from_civil(n(y)?, n(mo)?, n(d)?) * 86400 + n(h)? * 3600 + n(mi)? * 60 + n(se)?)
}

pub fn list() -> Vec<Snapshot> {
    let out = Command::new("/usr/bin/tmutil").args(["listlocalsnapshots", "/"]).output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    text.lines()
        .filter_map(|l| stamp(l.trim()))
        .map(|s| Snapshot { id: s.to_owned(), created: parse_stamp(s) })
        .collect()
}

/// Space macOS will free on demand (snapshots, caches it manages): the gap between
/// what's available for important use and what's plainly free.
pub fn purgeable() -> Option<u64> {
    let url = CFURL::from_path(Path::new("/"), true)?;
    let read = |key: &str| -> Option<i64> {
        let key = CFString::new(key);
        let mut value: *const c_void = std::ptr::null();
        let ok = unsafe { CFURLCopyResourcePropertyForKey(url.as_concrete_TypeRef(), key.as_concrete_TypeRef(), &mut value, std::ptr::null_mut()) };
        if ok == 0 || value.is_null() {
            return None;
        }
        let v = unsafe { CFType::wrap_under_create_rule(value) };
        v.downcast::<CFNumber>()?.to_i64()
    };
    let important = read("NSURLVolumeAvailableCapacityForImportantUsageKey")?;
    let plain = read("NSURLVolumeAvailableCapacityKey")?;
    Some((important - plain).max(0) as u64)
}

/// tmutil arguments that delete the given snapshots (all of them when empty).
pub fn delete_commands(ids: &[String]) -> Vec<Vec<String>> {
    if ids.is_empty() {
        return vec![vec!["/usr/bin/tmutil".into(), "deletelocalsnapshots".into(), "/".into()]];
    }
    ids.iter()
        .filter(|id| id.chars().all(|c| c.is_ascii_digit() || c == '-'))
        .map(|id| vec!["/usr/bin/tmutil".into(), "deletelocalsnapshots".into(), id.clone()])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_snapshot_names() {
        assert_eq!(stamp("com.apple.TimeMachine.2026-10-22-065608.local"), Some("2026-10-22-065608"));
        assert_eq!(parse_stamp("1970-01-02-000001"), Some(86401));
        assert_eq!(delete_commands(&["2026-10-22-065608".into(), "x; rm -rf /".into()]).len(), 1, "only date stamps pass");
        assert!(purgeable().is_some());
    }
}
