// Xcode, simulators and VM rules are macOS-only; Windows doesn't call them.
#![cfg_attr(windows, allow(dead_code))]

use super::*;
use parking_lot::Mutex;
use std::collections::BTreeMap;

/// A regenerable folder inside a project, recognised by its name plus a marker file
/// next to it (so a random folder called "build" is never touched).
struct Artifact {
    dir: &'static str,
    /// Any of these in the same folder confirms it. "*.ext" matches by extension.
    markers: &'static [&'static str],
}

const ARTIFACTS: &[Artifact] = &[
    Artifact { dir: "node_modules", markers: &["package.json"] },
    Artifact { dir: "target", markers: &["Cargo.toml"] },
    Artifact { dir: ".next", markers: &["package.json"] },
    Artifact { dir: ".nuxt", markers: &["package.json"] },
    Artifact { dir: ".svelte-kit", markers: &["package.json"] },
    Artifact { dir: ".turbo", markers: &["package.json", "turbo.json"] },
    Artifact { dir: ".parcel-cache", markers: &["package.json"] },
    Artifact { dir: ".expo", markers: &["package.json"] },
    Artifact { dir: ".angular", markers: &["angular.json"] },
    Artifact { dir: "build", markers: &["build.gradle", "build.gradle.kts", "pubspec.yaml"] },
    Artifact { dir: ".gradle", markers: &["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"] },
    Artifact { dir: ".cxx", markers: &["build.gradle", "build.gradle.kts"] },
    Artifact { dir: ".dart_tool", markers: &["pubspec.yaml"] },
    Artifact { dir: "Pods", markers: &["Podfile"] },
    Artifact { dir: ".build", markers: &["Package.swift"] },
    Artifact { dir: "DerivedData", markers: &["*.xcodeproj", "*.xcworkspace"] },
    Artifact { dir: ".tox", markers: &["tox.ini", "pyproject.toml", "setup.py"] },
    Artifact { dir: ".mypy_cache", markers: &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt", "mypy.ini"] },
    Artifact { dir: ".pytest_cache", markers: &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt", "pytest.ini"] },
    Artifact { dir: ".ruff_cache", markers: &["pyproject.toml", "ruff.toml", "requirements.txt"] },
    Artifact { dir: "vendor", markers: &["composer.json"] },
    Artifact { dir: "_build", markers: &["mix.exs"] },
    Artifact { dir: "deps", markers: &["mix.exs"] },
    Artifact { dir: ".stack-work", markers: &["stack.yaml"] },
    Artifact { dir: "zig-cache", markers: &["build.zig"] },
    Artifact { dir: ".zig-cache", markers: &["build.zig"] },
    Artifact { dir: "zig-out", markers: &["build.zig"] },
    Artifact { dir: "obj", markers: &["*.csproj", "*.fsproj"] },
    Artifact { dir: "bin", markers: &["*.csproj", "*.fsproj"] },
    Artifact { dir: "elm-stuff", markers: &["elm.json"] },
];

/// Python virtualenvs are recognised by their own pyvenv.cfg instead of a sibling marker.
const VENV_NAMES: &[&str] = &[".venv", "venv", "env", ".env"];

/// Folders that are usually one part of a larger project. Without a git root to go on,
/// their parent names the project ("penvision", not "src-tauri").
const SUBPROJECT_DIRS: &[&str] = &[
    "src-tauri", "android", "ios", "macos", "app", "api", "web", "client", "server", "frontend", "backend",
    "desktop", "mobile", "functions",
];

/// Never descended into at the top of the home folder.
// AppData (Windows) is huge and never holds projects.
const HOME_SKIP: &[&str] = &["Library", "Pictures", "Music", "Movies", "Applications", "Public", "AppData"];

/// Package-style folders that are never project trees.
pub(crate) const BUNDLE_EXTS: &[&str] = &["app", "photoslibrary", "musiclibrary", "tvlibrary", "fcpbundle", "logicx", "imovielibrary", "xcarchive", "framework", "bundle"];

/// True for a regenerable build folder: `name` inside a folder whose entries are `markers`.
pub(crate) fn is_build_artifact(name: &str, path: &Path, markers: &[String]) -> bool {
    ARTIFACTS.iter().any(|a| a.dir == name && a.markers.iter().any(|m| matches_marker(markers, m)))
        || (VENV_NAMES.contains(&name) && path.join("pyvenv.cfg").is_file())
}

struct Found {
    project: PathBuf,
    artifact: PathBuf,
}

fn matches_marker(files: &[String], marker: &str) -> bool {
    match marker.strip_prefix("*.") {
        Some(ext) => files.iter().any(|f| f.rsplit_once('.').is_some_and(|(_, e)| e == ext)),
        None => files.iter().any(|f| f == marker),
    }
}

fn walk(dir: &Path, git_root: Option<&Path>, depth: usize, cx: &Cx, out: &Mutex<Vec<Found>>) {
    if depth > 14 || cx.ctl.is_cancelled() {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    for e in rd.flatten() {
        // d_type from readdir: no stat per entry.
        let Ok(ft) = e.file_type() else { continue };
        let name = e.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            dirs.push((name, e.path()));
        } else if ft.is_file() {
            files.push(name);
        }
    }
    cx.ctl.add((files.len() + dirs.len()) as u64, 0);
    if depth < 6 {
        cx.ctl.set_current(dir);
    }

    let here_git = dirs.iter().any(|(n, _)| n == ".git");
    let git_root = if here_git { Some(dir) } else { git_root };
    let project = git_root.unwrap_or_else(|| match dir.parent() {
        Some(parent) if depth > 1 && SUBPROJECT_DIRS.contains(&file_name(dir).as_str()) => parent,
        _ => dir,
    });
    // Xcode projects are folders; count them as marker "files".
    let mut markers = files.clone();
    markers.extend(dirs.iter().filter(|(n, _)| n.ends_with(".xcodeproj") || n.ends_with(".xcworkspace")).map(|(n, _)| n.clone()));

    let mut descend = Vec::new();
    for (name, path) in dirs {
        if is_build_artifact(&name, &path, &markers) {
            out.lock().push(Found { project: project.to_path_buf(), artifact: path });
            continue;
        }
        let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        let skip = name.starts_with('.')
            || name == "node_modules"
            || BUNDLE_EXTS.contains(&ext)
            || (depth == 0 && HOME_SKIP.contains(&name.as_str()));
        if !skip {
            descend.push(path);
        }
    }
    descend.par_iter().for_each(|p| walk(p, git_root, depth + 1, cx, out));
}

/// Newest modification among a project's own files (not its build output): the root,
/// git's index, and two levels of entries. Bounded so huge repos stay cheap.
fn project_activity(root: &Path, artifacts: &HashSet<&Path>) -> Option<i64> {
    let mut newest = mtime_of(root);
    let mut bump = |t: Option<i64>| newest = newest.max(t);
    for f in [".git/index", ".git/HEAD", ".git/logs/HEAD"] {
        bump(mtime_of(&root.join(f)));
    }
    let mut budget = 4000usize;
    let mut level = vec![root.to_path_buf()];
    for _ in 0..2 {
        let mut next = Vec::new();
        for dir in level {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if budget == 0 {
                    return newest;
                }
                budget -= 1;
                let p = e.path();
                if e.file_name().to_string_lossy().starts_with('.') || artifacts.contains(p.as_path()) {
                    continue;
                }
                let Ok(md) = e.metadata() else { continue };
                bump(mtime(&md));
                if md.is_dir() {
                    next.push(p);
                }
            }
        }
        level = next;
    }
    newest
}

pub fn projects(cx: &Cx) -> Vec<Group> {
    let found = Mutex::new(Vec::new());
    walk(&cx.home, None, 0, cx, &found);

    let mut by_project: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for f in found.into_inner() {
        by_project.entry(f.project).or_default().push(f.artifact);
    }
    // A nested artifact (node_modules inside an ignored Pods tree) would be counted twice.
    for artifacts in by_project.values_mut() {
        artifacts.sort();
        let copy = artifacts.clone();
        artifacts.retain(|a| !copy.iter().any(|b| b != a && a.starts_with(b)));
    }

    let items: Vec<Item> = by_project
        .into_par_iter()
        .filter_map(|(root, artifacts)| {
            let size: u64 = artifacts.iter().filter_map(|a| cx.measure(a)).sum();
            if size < MIN_ITEM {
                return None;
            }
            let set: HashSet<&Path> = artifacts.iter().map(PathBuf::as_path).collect();
            let last = project_activity(&root, &set);
            let stale = last.is_none_or(|t| cx.now - t > STALE_PROJECT_DAYS * DAY);
            let names: Vec<String> = artifacts
                .iter()
                .map(|a| a.strip_prefix(&root).unwrap_or(a).display().to_string())
                .collect();
            Some(Item {
                id: format!("project:{}", root.display()),
                name: file_name(&root),
                detail: Some(format!("{} · {}", cx.tilde(&root), names.join(", "))),
                path: Some(root.to_string_lossy().into_owned()),
                size,
                kind: ItemKind::Project,
                mode: RemoveMode::Delete,
                last_activity: last,
                recommended: stale,
                warning: None,
                action: Action::Paths(artifacts),
            })
        })
        .collect();

    vec![group(
        "projects",
        Category::Developer,
        "Project Build Artifacts",
        "Dependencies and build output your tools recreate with one install or build. Projects untouched for 30 days are pre-selected.",
        items,
    )]
}

fn spec(cx: &Cx, rel: &str, name: &str, recommended: bool) -> Option<Item> {
    let path = cx.p(rel);
    ItemSpec { path: &path, name: name.into(), kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended }.build(cx)
}

pub fn xcode(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();
    // One item per project so the one you're working on can be kept.
    for d in children(&cx.p("Library/Developer/Xcode/DerivedData")) {
        let raw = file_name(&d);
        if raw == "ModuleCache.noindex" {
            items.extend(ItemSpec { path: &d, name: "Module cache".into(), kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: true }.build(cx));
            continue;
        }
        let project = raw.rsplit_once('-').map_or(raw.as_str(), |(p, _)| p).to_owned();
        if let Some(mut item) = (ItemSpec { path: &d, name: format!("{project} (DerivedData)"), kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: false }).build(cx) {
            item.recommended = item.last_activity.is_none_or(|t| cx.now - t > 7 * DAY);
            items.push(item);
        }
    }
    for platform in ["iOS", "watchOS", "tvOS", "visionOS", "macOS"] {
        for d in children(&cx.p(&format!("Library/Developer/Xcode/{platform} DeviceSupport"))) {
            let name = format!("{platform} {} device support", file_name(&d));
            items.extend(ItemSpec { path: &d, name, kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: true }.build(cx));
        }
    }
    items.extend(spec(cx, "Library/Developer/Xcode/UserData/Previews", "SwiftUI previews", true));
    items.extend(spec(cx, "Library/Developer/Xcode/UserData/IB Support", "Interface Builder support", true));
    items.extend(spec(cx, "Library/Developer/CoreSimulator/Caches", "Simulator caches", true));
    items.extend(spec(cx, "Library/Caches/com.apple.dt.Xcode", "Xcode cache", !cx.is_running("com.apple.dt.Xcode")));
    if let Some(mut a) = (ItemSpec {
        path: &cx.p("Library/Developer/Xcode/Archives"),
        name: "Archives".into(),
        kind: ItemKind::Folder,
        mode: RemoveMode::Trash,
        recommended: false,
    })
    .build(cx)
    {
        a.warning = Some("Needed to symbolicate crash reports from builds you've shipped.".into());
        items.push(a);
    }
    items.extend(spec(cx, "Library/Developer/Xamarin", "Xamarin", false));
    vec![group("xcode", Category::Developer, "Xcode", "Build products, device support files and caches Xcode recreates on demand.", items)]
}

#[derive(Deserialize)]
struct SimList {
    devices: HashMap<String, Vec<SimDevice>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimDevice {
    udid: String,
    name: String,
    state: String,
    is_available: bool,
    data_path: Option<String>,
}

pub fn simulators(cx: &Cx) -> Vec<Group> {
    let devices_dir = cx.p("Library/Developer/CoreSimulator/Devices");
    if !devices_dir.is_dir() || !Path::new("/usr/bin/xcrun").exists() {
        return vec![];
    }
    let Ok(out) = Command::new("/usr/bin/xcrun").args(["simctl", "list", "devices", "-j"]).output() else { return vec![] };
    let Ok(list) = serde_json::from_slice::<SimList>(&out.stdout) else { return vec![] };

    let mut items = Vec::new();
    for (runtime, devices) in list.devices {
        // com.apple.CoreSimulator.SimRuntime.iOS-17-5 → iOS 17.5
        let os = runtime.rsplit('.').next().unwrap_or(&runtime).replacen('-', " ", 1).replace('-', ".");
        for d in devices {
            let dir = devices_dir.join(&d.udid);
            let Some(size) = cx.measure(&dir) else { continue };
            let booted = d.state == "Booted";
            items.push(Item {
                id: format!("sim:{}", d.udid),
                name: format!("{} ({os})", d.name),
                detail: Some(if d.is_available { format!("{} · {}", d.state, d.udid) } else { "Runtime no longer installed".into() }),
                path: d.data_path.or_else(|| Some(dir.to_string_lossy().into_owned())),
                size,
                kind: ItemKind::Device,
                mode: RemoveMode::Delete,
                last_activity: mtime_of(&dir),
                recommended: !d.is_available,
                warning: booted.then(|| "This simulator is running.".into()),
                action: Action::Command(vec!["/usr/bin/xcrun".into(), "simctl".into(), "delete".into(), d.udid]),
            });
        }
    }
    vec![group("simulators", Category::Developer, "iOS Simulators", "Simulated devices and their installed apps. Ones whose runtime is gone are pre-selected.", items)]
}

pub fn package_caches(cx: &Cx) -> Vec<Group> {
    // (path under home, name, pre-selected). Not pre-selected: slow or large to re-download.
    const CACHES: &[(&str, &str, bool)] = &[
        (".npm/_cacache", "npm cache", true),
        ("Library/Caches/Yarn", "Yarn cache", true),
        ("Library/Caches/pnpm", "pnpm metadata cache", true),
        (".bun/install/cache", "Bun cache", true),
        ("Library/Caches/Homebrew", "Homebrew downloads", true),
        ("Library/Caches/pip", "pip cache", true),
        (".cache/pip", "pip cache", true),
        (".cache/uv", "uv cache", true),
        ("Library/Caches/pypoetry", "Poetry cache", true),
        ("Library/Caches/go-build", "Go build cache", true),
        ("Library/Caches/CocoaPods", "CocoaPods cache", true),
        ("Library/Caches/org.carthage.CarthageKit", "Carthage cache", true),
        ("Library/Caches/deno", "Deno cache", true),
        ("Library/Caches/node-gyp", "node-gyp headers", true),
        ("Library/Caches/typescript", "TypeScript cache", true),
        ("Library/Caches/electron", "Electron downloads", true),
        ("Library/Caches/electron-builder", "electron-builder cache", true),
        ("Library/Caches/composer", "Composer cache", true),
        (".cache/composer", "Composer cache", true),
        (".gradle/daemon", "Gradle daemon logs", true),
        (".android/cache", "Android SDK cache", true),
        (".cache/prisma", "Prisma engines", true),
        (".cache/node", "Node downloads", true),
        (".cache/firebase", "Firebase emulators", false),
        (".cache/puppeteer", "Puppeteer browsers", false),
        (".cache/selenium", "Selenium drivers", false),
        ("Library/Caches/ms-playwright", "Playwright browsers", false),
        ("Library/Caches/Cypress", "Cypress binaries", false),
        ("Library/Caches/cargo-xwin", "cargo-xwin Windows SDK", false),
        (".cargo/registry/cache", "Cargo crate archives", true),
        (".cargo/registry/src", "Cargo crate sources", false),
        (".cargo/git/checkouts", "Cargo git checkouts", false),
        (".gradle/caches", "Gradle caches", false),
        (".gradle/wrapper/dists", "Gradle distributions", false),
        (".m2/repository", "Maven repository", false),
        ("go/pkg/mod", "Go module cache", false),
        (".pub-cache", "Dart & Flutter packages", false),
        (".nuget/packages", "NuGet packages", false),
    ];
    let mut items: Vec<Item> = CACHES.par_iter().filter_map(|(rel, name, rec)| spec(cx, rel, name, *rec)).collect();

    if let Some(mut store) = spec(cx, "Library/pnpm/store", "pnpm store", false) {
        store.warning = Some("Projects hard-link files from here, so less space may be freed than shown.".into());
        items.push(store);
    }
    vec![group(
        "package-caches",
        Category::Developer,
        "Package Manager Caches",
        "Downloaded packages and toolchains. Everything here downloads again when a project needs it.",
        items,
    )]
}

/// "WebStorm2025.2" → ("WebStorm", [2025, 2])
fn product_version(name: &str) -> (String, Vec<u32>) {
    let split = name.find(|c: char| c.is_ascii_digit()).unwrap_or(name.len());
    let (product, version) = name.split_at(split);
    (product.to_owned(), version.split('.').filter_map(|p| p.parse().ok()).collect())
}

pub fn ide_caches(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();

    // JetBrains and Android Studio keep one cache folder per version; only the newest is live.
    let mut versioned: Vec<(PathBuf, String, Vec<u32>)> = Vec::new();
    for root in ["Library/Caches/JetBrains", "Library/Caches/Google"] {
        for d in children(&cx.p(root)) {
            let name = file_name(&d);
            if root.ends_with("Google") && !name.starts_with("AndroidStudio") {
                continue;
            }
            let (product, version) = product_version(&name);
            versioned.push((d, product, version));
        }
    }
    let mut newest: HashMap<String, Vec<u32>> = HashMap::new();
    for (_, product, version) in &versioned {
        let e = newest.entry(product.clone()).or_default();
        if version > e {
            *e = version.clone();
        }
    }
    for (path, product, version) in &versioned {
        let old = newest.get(product).is_some_and(|n| version < n);
        if let Some(mut item) = (ItemSpec { path, name: file_name(path), kind: ItemKind::Folder, mode: RemoveMode::Delete, recommended: old }).build(cx) {
            item.detail = Some(if old { "Older version's cache".into() } else { cx.tilde(path) });
            items.push(item);
        }
    }

    // Electron-based editors: only their cache folders, never settings or extensions.
    const EDITORS: &[(&str, &str, &str)] = &[
        ("Code", "Visual Studio Code", "com.microsoft.VSCode"),
        ("Cursor", "Cursor", "com.todesktop.230313mzl4w4u92"),
        ("Windsurf", "Windsurf", "com.exafunction.windsurf"),
    ];
    for (dir, name, bundle) in EDITORS {
        let base = cx.p(&format!("Library/Application Support/{dir}"));
        let parts: Vec<PathBuf> = ["Cache", "CachedData", "CachedExtensionVSIXs", "Code Cache", "GPUCache", "logs", "Crashpad/completed"]
            .iter()
            .map(|p| base.join(p))
            .filter(|p| p.exists())
            .collect();
        let size: u64 = parts.iter().filter_map(|p| cx.measure(p)).sum();
        if size < MIN_ITEM {
            continue;
        }
        let running = cx.is_running(bundle);
        items.push(Item {
            id: format!("editor:{dir}"),
            name: format!("{name} caches"),
            detail: Some(cx.tilde(&base)),
            path: Some(base.to_string_lossy().into_owned()),
            size,
            kind: ItemKind::Folder,
            mode: RemoveMode::Delete,
            last_activity: None,
            recommended: !running,
            warning: running.then(|| format!("{name} is running. Quit it first.")),
            action: Action::Paths(parts),
        });
    }
    vec![group("ide-caches", Category::Developer, "IDE Caches", "Indexes and caches editors rebuild when you reopen a project.", items)]
}

pub fn virtual_machines(cx: &Cx) -> Vec<Group> {
    let mut items = Vec::new();
    if let Some(mut colima) = spec(cx, ".colima", "Colima", false) {
        colima.warning = Some("Removes every Colima VM with all its images, containers and volumes. `docker system prune` frees space without that.".into());
        items.push(colima);
    }
    if let Some(mut docker) = spec(cx, "Library/Containers/com.docker.docker/Data/vms", "Docker Desktop VM", false) {
        docker.warning = Some("Removes all Docker Desktop images, containers and volumes.".into());
        items.push(docker);
    }
    for avd in children(&cx.p(".android/avd")) {
        if avd.extension().is_some_and(|e| e == "avd") {
            let name = file_name(&avd).trim_end_matches(".avd").replace('_', " ");
            let ini = avd.with_extension("ini");
            if let Some(mut item) = (ItemSpec { path: &avd, name: format!("{name} (Android emulator)"), kind: ItemKind::Device, mode: RemoveMode::Delete, recommended: false }).build(cx) {
                if ini.exists() {
                    if let Action::Paths(p) = &mut item.action {
                        p.push(ini);
                    }
                }
                items.push(item);
            }
        }
    }
    vec![group("virtual-machines", Category::Developer, "VMs & Emulators", "Container VMs and Android emulators. Large, but each holds state you may want.", items)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ide_versions() {
        assert_eq!(product_version("WebStorm2025.2"), ("WebStorm".into(), vec![2025, 2]));
        assert!(product_version("WebStorm2024.3").1 < product_version("WebStorm2025.1").1);
    }

    #[test]
    fn finds_artifacts_only_next_to_markers() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        let mk = |rel: &str| fs::create_dir_all(h.join(rel)).unwrap();
        let touch = |rel: &str| fs::write(h.join(rel), b"x").unwrap();
        mk("code/app/.git");
        touch("code/app/package.json");
        mk("code/app/node_modules/left-pad");
        touch("code/app/node_modules/left-pad/package.json");
        mk("code/app/src-tauri/target/debug");
        touch("code/app/src-tauri/Cargo.toml");
        mk("photos/build"); // no marker: must be ignored
        mk("Library/proj/node_modules"); // skipped root
        touch("Library/proj/package.json");

        let ctl = ScanControl::new();
        let cx = Cx { home: h.to_path_buf(), ctl: &ctl, apps: AppIndex::default(), running: HashSet::new(), now: 0 };
        let found = Mutex::new(Vec::new());
        walk(h, None, 0, &cx, &found);
        let mut got: Vec<(String, String)> = found
            .into_inner()
            .into_iter()
            .map(|f| (file_name(&f.project), f.artifact.strip_prefix(h).unwrap().display().to_string().replace('\\', "/")))
            .collect();
        got.sort();
        assert_eq!(got, [("app".into(), "code/app/node_modules".into()), ("app".into(), "code/app/src-tauri/target".into())]);
    }
}
