//! The Recycle Bin on every drive, through the shell (it spans per-drive hidden folders).

use windows_sys::Win32::UI::Shell::{SHEmptyRecycleBinW, SHQueryRecycleBinW, SHQUERYRBINFO};

/// (bytes, items) in the Recycle Bin across all drives.
pub fn size() -> (u64, u64) {
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, i64Size: 0, i64NumItems: 0 };
    // A null root means "all drives".
    if unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) } == 0 {
        (info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64)
    } else {
        (0, 0)
    }
}

/// Empties it for good, without Explorer's confirmation, progress window or sound.
/// Returns the bytes freed.
pub fn empty() -> Result<u64, String> {
    const SHERB_NOCONFIRMATION: u32 = 1;
    const SHERB_NOPROGRESSUI: u32 = 2;
    const SHERB_NOSOUND: u32 = 4;
    let (before, _) = size();
    let hr = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) };
    // E_UNEXPECTED is what it returns for an already-empty bin.
    const E_UNEXPECTED: i32 = 0x8000_FFFFu32 as i32;
    if hr >= 0 || hr == E_UNEXPECTED { Ok(before.saturating_sub(size().0)) } else { Err(format!("Couldn't empty the Recycle Bin (0x{:08X})", hr as u32)) }
}
