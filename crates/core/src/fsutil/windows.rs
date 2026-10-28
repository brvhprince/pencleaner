//! Windows specifics of `fsutil`. NTFS needs less care than APFS here: no clones to
//! detect, hard links are rare, and OneDrive placeholders are marked by attributes.

use super::BulkEntry;
use std::fs::Metadata;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

/// Hard-link identity isn't on stable `Metadata` on Windows (and links are rare there),
/// so every file counts as its own.
pub fn link_identity(_md: &Metadata) -> (u64, u64, u64) {
    (0, 0, 1)
}

/// Scans stay on one drive by walking from a drive root; junctions and symlinks aren't
/// followed (`DirEntry::metadata` reports them as links), so there's nothing to compare.
pub fn device(_md: &Metadata) -> u64 {
    0
}

/// Size on disk, approximated as the logical size rounded up to a 4 KiB cluster.
/// Cloud-only OneDrive files take no space.
pub fn allocated(md: &Metadata) -> u64 {
    if is_dataless(md) {
        return 0;
    }
    md.len().div_ceil(4096) * 4096
}

const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

/// OneDrive "online-only" placeholder: no local data, and reading it would download it.
pub fn is_dataless(md: &Metadata) -> bool {
    md.file_attributes() & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0
}

/// Nothing to set on Windows: placeholders are skipped by attribute instead.
pub fn set_observer_io_policy() {}

/// Block sharing (ReFS cloning) is rare on Windows; copies are treated as separate.
pub fn physical_offset(_path: &Path) -> Option<u64> {
    None
}

/// No bulk listing call: callers fall back to `std::fs::read_dir`.
pub fn read_dir_bulk(_dir: &Path, _each: impl FnMut(BulkEntry)) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
