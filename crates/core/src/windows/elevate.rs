//! Administrator rights on demand. Scans never ask; when a clean includes Windows system
//! junk, one UAC prompt starts an elevated copy of PenCleaner (`--elevated-job`) that
//! deletes those paths and exits. If PenCleaner already runs as administrator, it just
//! does the work.
//!
//! The elevated copy trusts nothing it's handed: every path must sit inside the system
//! junk folders `safety::admin_cleanable` allows, and passes the safety gate again, so a
//! tampered job file can't steer it anywhere else.

use crate::remove::{RemoveMode, Remover};
use crate::{fsutil, safety};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Command-line flag for the elevated copy: `--elevated-job <job.json> <result.json>`.
pub const FLAG: &str = "--elevated-job";

#[derive(Debug, Serialize, Deserialize)]
pub struct Job {
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct JobResult {
    pub removed: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
    pub bytes: u64,
}

/// Whether this process already has administrator rights.
pub fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut e = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(token, TokenElevation, (&mut e as *mut TOKEN_ELEVATION).cast(), std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut len);
        CloseHandle(token);
        ok != 0 && e.TokenIsElevated != 0
    }
}

/// Deletes the allowed paths among `paths` (needs administrator rights to succeed).
pub fn remove_paths(paths: &[PathBuf]) -> JobResult {
    let remover = Remover::default();
    let mut out = JobResult::default();
    for p in paths {
        if !safety::admin_cleanable(p) {
            out.failed.push((p.clone(), "Not a system folder PenCleaner cleans".into()));
            continue;
        }
        let size = fsutil::disk_usage(p);
        match remover.remove(p, RemoveMode::Delete) {
            Ok(()) => {
                out.bytes += size;
                out.removed.push(p.clone());
            }
            Err(e) => out.failed.push((p.clone(), e)),
        }
    }
    out
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Runs `job` with administrator rights: directly when already elevated, otherwise in an
/// elevated copy behind one UAC prompt. Err when the prompt is declined.
pub fn run(job: &Job) -> Result<JobResult, String> {
    if is_elevated() {
        return Ok(remove_paths(&job.paths));
    }
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir();
    let job_file = dir.join(format!("pencleaner-job-{}-{stamp}.json", std::process::id()));
    let result_file = job_file.with_extension("result.json");
    std::fs::write(&job_file, serde_json::to_vec(job).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    let params = format!("{FLAG} \"{}\" \"{}\"", job_file.display(), result_file.display());
    let (verb, file, params) = (wide("runas"), wide(&exe.to_string_lossy()), wide(&params));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = 0; // SW_HIDE: the elevated copy has no window anyway

    let started = unsafe { ShellExecuteExW(&mut info) } != 0;
    if !started {
        let _ = std::fs::remove_file(&job_file);
        const ERROR_CANCELLED: u32 = 1223;
        return Err(match unsafe { GetLastError() } {
            ERROR_CANCELLED => "Administrator access wasn't granted, so system files were left alone.".into(),
            code => format!("Couldn't ask for administrator access (error {code})."),
        });
    }
    // Large caches can take a while; half an hour is generous.
    let mut code = 1u32;
    unsafe {
        WaitForSingleObject(info.hProcess, 30 * 60 * 1000);
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
    let _ = std::fs::remove_file(&job_file);
    let result = std::fs::read(&result_file).ok().and_then(|b| serde_json::from_slice::<JobResult>(&b).ok());
    let _ = std::fs::remove_file(&result_file);
    result.ok_or_else(|| format!("The administrator step didn't finish (exit code {code})."))
}

/// Entry point of the elevated copy. Returns the process exit code, or None when this
/// isn't an elevated job.
pub fn child_main() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    let at = args.iter().position(|a| a == FLAG)?;
    let (Some(job), Some(result)) = (args.get(at + 1), args.get(at + 2)) else { return Some(2) };
    let job: Job = match std::fs::read(job).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
        Some(j) => j,
        None => return Some(3),
    };
    let out = remove_paths(&job.paths);
    let ok = std::fs::write(Path::new(result), serde_json::to_vec(&out).unwrap_or_default()).is_ok();
    Some(if ok { 0 } else { 4 })
}
