//! Windows stand-in for `permissions`: there's no Full Disk Access gate; an ordinary user
//! can read their own profile, and system folders simply report as unreadable.

pub fn has_full_disk_access() -> bool {
    true
}
