//! Applications on Windows: installed programs from the registry's uninstall lists, their
//! data in AppData, data left by programs that are gone, and installers in Downloads.
//!
//! Removing a program runs its own uninstaller (Windows has no other safe way); once it
//! has really left the list, its AppData goes to the Recycle Bin. Leftovers and installers
//! go to the Recycle Bin directly.

#[path = "../applications/types.rs"]
mod types;
pub use types::*;

use crate::cleanup::{mtime_of, CleanEvent, CleanSummary};
use crate::program_names::{self, normalize, normalize_publisher, parse_date};
use crate::fsutil::{self, home_dir};
use crate::progress::ScanControl;
use crate::remove::{RemoveMode, Remover};
use crate::windows::registry::{self, Root, CURRENT_USER, LOCAL_MACHINE};
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DAY: i64 = 86_400;
const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_32: &str = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// One program from an uninstall list.
struct Program {
    root: Root,
    key: String,
    /// The subkey's own name: a product GUID for MSI installs.
    key_name: String,
    name: String,
    version: Option<String>,
    publisher: Option<String>,
    location: Option<PathBuf>,
    uninstall: String,
    msi: bool,
    size_kb: Option<u32>,
    installed: Option<i64>,
}

/// The id the UI sends back: which list, and which entry in it.
fn id_of(root: Root, key: &str) -> String {
    let hive = match root {
        Root::CurrentUser => "HKCU",
        Root::LocalMachine => "HKLM",
    };
    format!(r"{hive}\{key}")
}

fn root_of(id: &str) -> Option<(Root, &str)> {
    let (hive, key) = id.split_once('\\')?;
    match hive {
        "HKCU" => Some((CURRENT_USER, key)),
        "HKLM" => Some((LOCAL_MACHINE, key)),
        _ => None,
    }
}

fn programs() -> Vec<Program> {
    let mut out: Vec<Program> = Vec::new();
    let mut seen = HashSet::new();
    for (root, list) in [(LOCAL_MACHINE, UNINSTALL), (LOCAL_MACHINE, UNINSTALL_32), (CURRENT_USER, UNINSTALL)] {
        for sub in registry::subkeys(root, list) {
            let key = format!(r"{list}\{sub}");
            let get = |v: &str| registry::get_string(root, &key, v).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
            let dword = |v: &str| registry::get_dword(root, &key, v);
            let (Some(name), Some(uninstall)) = (get("DisplayName"), get("UninstallString")) else { continue };
            // Windows components, and updates/patches that belong to another entry.
            if dword("SystemComponent") == Some(1) || get("ParentKeyName").is_some() {
                continue;
            }
            if get("ReleaseType").is_some_and(|t| t.contains("Update") || t.contains("Hotfix")) {
                continue;
            }
            if hidden(&name, get("InstallLocation").as_deref()) {
                continue;
            }
            let version = get("DisplayVersion");
            // The same program can appear in two lists (32- and 64-bit parts).
            if !seen.insert((name.to_lowercase(), version.clone())) {
                continue;
            }
            let location = get("InstallLocation").map(|l| PathBuf::from(l.trim_matches('"'))).filter(|p| p.is_dir());
            let msi = dword("WindowsInstaller") == Some(1) || uninstall.to_ascii_lowercase().contains("msiexec");
            let (publisher, size_kb) = (get("Publisher"), dword("EstimatedSize"));
            let installed = get("InstallDate").and_then(|d| parse_date(&d));
            out.push(Program { root, key: key.clone(), key_name: sub, name, version, publisher, location, uninstall, msi, size_kb, installed });
        }
    }
    out
}

/// Data folders: Roaming, Local and LocalLow.
fn data_roots() -> Vec<(PathBuf, &'static str)> {
    let mut v = Vec::new();
    if let Some(r) = std::env::var_os("APPDATA") {
        v.push((PathBuf::from(r), "AppData (Roaming)"));
    }
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        let l = PathBuf::from(l);
        if let Some(parent) = l.parent() {
            v.push((parent.join("LocalLow"), "AppData (LocalLow)"));
        }
        v.push((l, "AppData (Local)"));
    }
    v
}

/// Folders in AppData that belong to Windows, runtimes or tools, never to one program.
const NEVER_LEFTOVER: &[&str] = &[
    "microsoft", "packages", "temp", "programs", "comms", "connecteddevicesplatform", "d3dscache", "crashdumps", "publishers",
    "peerdistrepub", "virtualstore", "placeholdertilelogofolder", "isolatedstorage", "assembly", "apps", "cache", "package cache",
    "squirreltemp", "history", "application data", "elevateddiagnostics", "diagnostics", "npm", "npm-cache", "pip", "pnpm",
    "pnpm-cache", "yarn", "nuget", "go-build", "deno", "node-gyp", "ms-playwright", "cypress", "electron", "electron-builder",
    "jetbrains", "google", "mozilla", "intel", "nvidia", "nvidia corporation", "amd", "realtek", "hp", "dell", "lenovo", "asus",
    "acer", "adobe", "apple", "oracle", "sun", "java", "python", "fontconfig", "gtk-3.0", "github", "github desktop",
    "windowsapps", "identities", "tile data", "nuget.org", "dotnet", ".net",
];

struct Folder {
    path: PathBuf,
    name: String,
    label: &'static str,
}

/// Top-level AppData folders, and one level down under publisher folders.
fn data_folders() -> Vec<Folder> {
    let mut out = Vec::new();
    for (root, label) in data_roots() {
        for e in fs::read_dir(&root).into_iter().flatten().flatten() {
            let path = e.path();
            if !path.is_dir() || e.file_type().is_ok_and(|t| t.is_symlink()) {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            out.push(Folder { path, name, label });
        }
    }
    out
}

fn related_for(p: &Program, folders: &[Folder]) -> Vec<RelatedFile> {
    let name = normalize(&p.name);
    if name.len() < 3 {
        return Vec::new();
    }
    let publisher = p.publisher.as_deref().map(normalize_publisher);
    let mut paths: Vec<(PathBuf, &'static str)> = Vec::new();
    for f in folders {
        let folder = f.name.to_lowercase();
        if NEVER_LEFTOVER.contains(&folder.as_str()) {
            // A vendor folder (e.g. "Google"): look one level down for this program.
            if publisher.as_deref().is_some_and(|pb| pb.starts_with(&folder) || folder.starts_with(pb)) {
                for e in fs::read_dir(&f.path).into_iter().flatten().flatten() {
                    if normalize(&e.file_name().to_string_lossy()) == name && e.path().is_dir() {
                        paths.push((e.path(), f.label));
                    }
                }
            }
            continue;
        }
        if normalize(&f.name) == name {
            paths.push((f.path.clone(), f.label));
        } else if publisher.as_deref().is_some_and(|pb| normalize_publisher(&f.name) == pb) {
            // "<Publisher>\<Program>"
            let inner = f.path.join(&p.name);
            if inner.is_dir() {
                paths.push((inner, f.label));
            }
        }
    }
    // The program's own folder goes with its uninstaller, not here.
    if let Some(loc) = &p.location {
        paths.retain(|(path, _)| !path.starts_with(loc));
    }
    let mut files: Vec<RelatedFile> = paths
        .into_par_iter()
        .map(|(path, label)| RelatedFile { size: fsutil::disk_usage(&path), path: path.to_string_lossy().into_owned(), label, admin: false })
        .collect();
    files.sort_unstable_by_key(|f| std::cmp::Reverse(f.size));
    files
}

/// Programs the list leaves out: PenCleaner itself, and Microsoft Edge with its WebView2
/// runtime and updater, whose uninstallers Windows makes refuse (outside the EU), like
/// Apple's own apps on the Mac.
fn hidden(name: &str, location: Option<&str>) -> bool {
    let me = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_string_lossy().to_lowercase()));
    let at_me = location.is_some_and(|l| me.as_deref() == Some(l.trim_matches('"').trim_end_matches('\\').to_lowercase().as_str()));
    at_me || name.eq_ignore_ascii_case("PenCleaner") || name.starts_with("Microsoft Edge")
}

/// Full paths of running programs, lower-case. Some system processes can't be asked
/// without administrator rights; those are simply missing, which only means a "Running"
/// tag might not show.
fn running_paths() -> Vec<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::K32EnumProcesses;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    let mut pids = vec![0u32; 4096];
    let mut used = 0u32;
    if unsafe { K32EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut used) } == 0 {
        return Vec::new();
    }
    pids.truncate(used as usize / 4);
    let mut out = Vec::new();
    let mut buf = [0u16; 1024];
    for pid in pids.into_iter().filter(|p| *p > 4) {
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if h.is_null() {
            continue;
        }
        let mut len = buf.len() as u32;
        if unsafe { QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) } != 0 {
            out.push(String::from_utf16_lossy(&buf[..len as usize]).to_lowercase());
        }
        unsafe { CloseHandle(h) };
    }
    out
}

/// Executable names of running programs ("slack.exe"), for the leftovers check.
fn running_names(paths: &[String]) -> HashSet<String> {
    paths.iter().filter_map(|p| p.rsplit('\\').next().map(str::to_owned)).collect()
}

/// A program is running if a process runs from inside its own folder (by full path: a
/// shared name like node.exe says nothing).
fn is_running(p: &Program, running: &[String]) -> bool {
    let Some(loc) = &p.location else { return false };
    let dir = format!("{}\\", loc.to_string_lossy().trim_end_matches('\\').to_lowercase());
    running.iter().any(|r| r.starts_with(&dir))
}

/// Data folders of programs that are no longer installed: untouched for 6+ months, not
/// matching anything installed or running, and never Windows' or a tool's own.
fn leftovers(programs: &[Program], folders: &[Folder], claimed: &HashSet<PathBuf>, running: &HashSet<String>) -> Vec<Leftover> {
    // `running`: executable names, e.g. "slack.exe".
    let names: HashSet<String> = programs.iter().map(|p| normalize(&p.name)).collect();
    let publishers: HashSet<String> = programs.iter().filter_map(|p| p.publisher.as_deref().map(normalize_publisher)).collect();
    let running_names: HashSet<String> = running.iter().map(|e| normalize(e.trim_end_matches(".exe"))).collect();
    let cutoff = now() - 180 * DAY;
    let mut out: Vec<Leftover> = folders
        .par_iter()
        .filter(|f| !claimed.contains(&f.path))
        .filter_map(|f| {
            let key = f.name.to_lowercase();
            let norm = normalize(&f.name);
            if key.starts_with('.') || NEVER_LEFTOVER.contains(&key.as_str()) || norm.len() < 3 {
                return None;
            }
            // Anything an installed or running program might be using.
            let matches = |set: &HashSet<String>| set.iter().any(|n| !n.is_empty() && (n == &norm || n.starts_with(&format!("{norm} ")) || norm.starts_with(&format!("{n} "))));
            if matches(&names) || matches(&running_names) {
                return None;
            }
            let last = newest_mtime(&f.path, 2);
            if last.is_none_or(|t| t > cutoff) {
                return None;
            }
            let size = fsutil::disk_usage(&f.path);
            (size > 0).then(|| Leftover {
                id: f.path.to_string_lossy().into_owned(),
                vendor_installed: publishers.contains(&normalize_publisher(&f.name)),
                files: vec![RelatedFile { path: f.path.to_string_lossy().into_owned(), label: f.label, size, admin: false }],
                size,
                last_activity: last,
            })
        })
        .collect();
    out.sort_unstable_by_key(|l| std::cmp::Reverse(l.size));
    out
}

/// The newest modification time in a folder, looking `depth` levels down.
fn newest_mtime(dir: &Path, depth: usize) -> Option<i64> {
    let mut newest = mtime_of(dir);
    if depth > 0 {
        for e in fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            let t = if p.is_dir() { newest_mtime(&p, depth - 1) } else { mtime_of(&p) };
            newest = newest.max(t);
        }
    }
    newest
}

const INSTALLER_EXTS: &[&str] = &["msi", "msix", "msixbundle", "appx", "appxbundle", "iso"];

fn installers(home: &Path, apps: &[AppEntry], ctl: &ScanControl) -> Vec<Installer> {
    fn walk(dir: &Path, depth: usize, exes: bool, out: &mut Vec<PathBuf>) {
        for p in fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()) {
            let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if INSTALLER_EXTS.contains(&ext.as_str()) || (exes && ext == "exe") {
                out.push(p);
            } else if depth > 0 && p.is_dir() {
                walk(&p, depth - 1, exes, out);
            }
        }
    }
    let mut found = Vec::new();
    // Downloads and the Desktop: an .exe there is almost always a setup program. Elsewhere
    // an .exe may be a portable tool someone uses, so only packages count.
    walk(&home.join("Downloads"), 3, true, &mut found);
    walk(&home.join("Desktop"), 1, true, &mut found);
    walk(&home.join("Documents"), 3, false, &mut found);
    let cutoff = now() - 30 * DAY;
    let mut out: Vec<Installer> = found
        .par_iter()
        .map(|p| {
            ctl.set_current(p);
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            let installed_app = apps
                .iter()
                .find(|a| {
                    let n = normalize(&a.name);
                    n.len() >= 3 && n.split(' ').next().is_some_and(|first| first.len() >= 3 && lower.contains(first))
                })
                .map(|a| a.name.clone());
            let added = mtime_of(p);
            Installer {
                path: p.to_string_lossy().into_owned(),
                size: fsutil::disk_usage(p),
                recommended: installed_app.is_some() || added.is_some_and(|t| t < cutoff),
                name,
                added,
                installed_app,
            }
        })
        .collect();
    out.sort_unstable_by_key(|i| std::cmp::Reverse(i.size));
    out
}

pub fn scan(ctl: &ScanControl) -> AppsReport {
    let started = Instant::now();
    let programs = programs();
    let folders = data_folders();
    let running = running_paths();

    let apps: Vec<AppEntry> = programs
        .par_iter()
        .map(|p| {
            ctl.set_current(Path::new(&p.name));
            let related = related_for(p, &folders);
            let size = match (p.size_kb, &p.location) {
                (Some(kb), _) if kb > 0 => kb as u64 * 1024,
                (_, Some(loc)) => fsutil::disk_usage(loc),
                _ => 0,
            };
            ctl.add(1, size);
            AppEntry {
                id: id_of(p.root, &p.key),
                bundle_id: p.key_name.clone(),
                name: p.name.clone(),
                version: p.version.clone(),
                path: p.location.as_ref().map(|l| l.to_string_lossy().into_owned()).unwrap_or_default(),
                size,
                related_size: related.iter().map(|f| f.size).sum(),
                related,
                last_used: None,
                app_store: false,
                publisher: p.publisher.clone(),
                installed: p.installed,
                running: is_running(p, &running),
                shares_support_files: false,
            }
        })
        .collect();
    let claimed: HashSet<PathBuf> = apps.iter().flat_map(|a| a.related.iter().map(|f| PathBuf::from(&f.path))).collect();
    let leftovers = if ctl.is_cancelled() { Vec::new() } else { leftovers(&programs, &folders, &claimed, &running_names(&running)) };
    let installers = if ctl.is_cancelled() { Vec::new() } else { installers(&home_dir(), &apps, ctl) };
    let mut apps = apps;
    apps.sort_by_key(|a| std::cmp::Reverse(a.size + a.related_size));
    AppsReport { apps, leftovers, installers, elapsed_ms: started.elapsed().as_millis() as u64, cancelled: ctl.is_cancelled() }
}

/// The uninstaller to start for a program: (program, arguments).
fn uninstall_command(program: &Program) -> (String, String) {
    program_names::uninstall_command(&program.uninstall, program.msi.then_some(program.key_name.as_str()))
}

/// Starts a program the way Explorer would (so a UAC prompt appears if it needs one) and
/// waits for it to exit.
fn run_and_wait(file: &str, params: &str, limit: Duration) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (file_w, params_w) = (wide(file), wide(params));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpFile = file_w.as_ptr();
    info.lpParameters = params_w.as_ptr();
    info.nShow = 1; // SW_SHOWNORMAL: the uninstaller's own window
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        const ERROR_CANCELLED: u32 = 1223;
        return Err(match unsafe { GetLastError() } {
            ERROR_CANCELLED => "Administrator access wasn't granted, so the uninstaller didn't run.".into(),
            code => format!("Couldn't start the uninstaller (error {code})."),
        });
    }
    if !info.hProcess.is_null() {
        unsafe {
            WaitForSingleObject(info.hProcess, limit.as_millis().min(u32::MAX as u128) as u32);
            CloseHandle(info.hProcess);
        }
    }
    Ok(())
}

/// Runs a program's uninstaller and waits until its entry leaves the list. Many
/// uninstallers hand over to a copy of themselves and exit early, so the list is the
/// truth, not the process.
fn uninstall(p: &Program) -> Result<(), String> {
    let (file, params) = uninstall_command(p);
    run_and_wait(&file, &params, Duration::from_secs(30 * 60))?;
    let deadline = Instant::now() + Duration::from_secs(10 * 60);
    while registry::exists(p.root, &p.key) {
        if Instant::now() > deadline {
            return Err(format!("{} is still installed: its uninstaller was cancelled or didn't finish.", p.name));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}

/// Moves a data folder to the Recycle Bin, retrying for a few seconds: right after an
/// uninstaller finishes, its files are often still held open briefly.
fn move_data(remover: &Remover, path: &Path) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(2));
        }
        match remover.remove(path, RemoveMode::Trash) {
            Ok(()) => return Ok(()),
            Err(e) => last = e,
        }
    }
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    Err(format!("its data folder \u{201c}{name}\u{201d} couldn't go to the Recycle Bin: {last}"))
}

impl AppsReport {
    /// Uninstalls programs (their own uninstallers, then their AppData to the Recycle
    /// Bin) and moves leftovers and installers to the Recycle Bin.
    pub fn remove(&mut self, targets: &[Target], mut on_event: impl FnMut(CleanEvent)) -> CleanSummary {
        let remover = Remover::default();
        let mut summary = CleanSummary::default();
        let total = targets.len();
        let programs = programs();
        for (index, t) in targets.iter().enumerate() {
            let (id, size, result) = match t {
                Target::App(id) => {
                    let Some(a) = self.apps.iter().find(|a| &a.id == id) else { continue };
                    on_event(CleanEvent::Started { id: a.id.clone(), name: a.name.clone(), size: a.size + a.related_size, index, total });
                    // Running is fine here: unlike quitting a Mac app, a background service can't
                    // just be closed, and an uninstaller handles its own program.
                    let result = match root_of(id).and_then(|(root, key)| programs.iter().find(|p| p.root == root && p.key == key)) {
                        None => Err(format!("{} is no longer in the installed programs list. Scan again.", a.name)),
                        Some(p) => uninstall(p).and_then(|()| {
                            // Its data, now that the program itself is gone.
                            let errors: Vec<String> = a.related.iter().filter_map(|f| move_data(&remover, Path::new(&f.path)).err()).collect();
                            match errors.first() {
                                None => Ok(()),
                                Some(e) => Err(format!("Uninstalled, but {e}")),
                            }
                        }),
                    };
                    (a.id.clone(), a.size + a.related_size, result)
                }
                Target::Leftover(id) => {
                    let Some(l) = self.leftovers.iter().find(|l| &l.id == id) else { continue };
                    on_event(CleanEvent::Started { id: l.id.clone(), name: l.id.clone(), size: l.size, index, total });
                    let errors: Vec<String> = l.files.iter().filter_map(|f| move_data(&remover, Path::new(&f.path)).err()).collect();
                    (l.id.clone(), l.size, errors.first().map_or(Ok(()), |e| Err(e.clone())))
                }
                Target::Installer(path) => {
                    let Some(i) = self.installers.iter().find(|i| &i.path == path) else { continue };
                    on_event(CleanEvent::Started { id: i.path.clone(), name: i.name.clone(), size: i.size, index, total });
                    (i.path.clone(), i.size, remover.remove(Path::new(&i.path), RemoveMode::Trash))
                }
            };
            match result {
                Ok(()) => {
                    summary.bytes_freed += size;
                    summary.cleaned.push(id.clone());
                    on_event(CleanEvent::Cleaned { id, bytes: size });
                }
                Err(error) => {
                    summary.failed.push((id.clone(), error.clone()));
                    on_event(CleanEvent::Failed { id, error, bytes: 0 });
                }
            }
        }
        let done: HashSet<&str> = summary.cleaned.iter().map(String::as_str).collect();
        self.apps.retain(|a| !done.contains(a.id.as_str()));
        self.leftovers.retain(|l| !done.contains(l.id.as_str()));
        self.installers.retain(|i| !done.contains(i.path.as_str()));
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_installed_programs() {
        // Every Windows machine (and CI runner) has some.
        let r = scan(&ScanControl::new());
        assert!(!r.apps.is_empty());
        assert!(r.apps.iter().all(|a| !a.name.is_empty() && a.id.starts_with("HK")));
    }
}
