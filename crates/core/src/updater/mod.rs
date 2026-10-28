//! App Updater: finds newer versions of installed apps from the source that actually
//! knows (the Mac App Store, the app's own Sparkle feed, or Homebrew) and installs
//! the ones that can be installed safely.
//!
//! Network: this module sends the bundle ids of App Store apps to Apple's public lookup
//! API, fetches each Sparkle app's own feed, and downloads Homebrew's public cask catalog
//! (cached for a day). Nothing else leaves the Mac.

pub mod appcast;
mod install;
pub mod version;

use crate::apps::{self, AppInfo};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub use install::{install, prepare, Prepared};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    AppStore,
    Sparkle,
    Homebrew,
    /// Homebrew's catalog knows the latest version, but the app wasn't installed with brew.
    Catalog,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    /// The app's path, or "brew:<token>" for casks without an app.
    pub id: String,
    pub name: String,
    pub app_path: Option<String>,
    pub source: Source,
    pub installed: String,
    pub latest: String,
    pub size: Option<u64>,
    /// Plain-text release notes, trimmed.
    pub notes: Option<String>,
    pub notes_url: Option<String>,
    /// One click from PenCleaner; otherwise `action` says what happens.
    pub one_click: bool,
    /// For App Store updates: opens the App Store. For the rest: why it isn't one click.
    pub action_note: Option<String>,
    /// The update is cryptographically signed by the developer (Sparkle EdDSA).
    pub signed: bool,
    /// This Mac can't run it (shown, not installable).
    pub requires: Option<String>,
    pub running: bool,
    #[serde(skip)]
    pub(crate) plan: Plan,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum Plan {
    #[default]
    None,
    AppStore { track_id: u64 },
    Sparkle { release: Box<appcast::Release>, ed_key: Option<String>, bundle_id: String },
    Homebrew { token: String },
    /// Open the app (it updates itself) or, failing that, the developer's site.
    Open { url: Option<String> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesReport {
    pub updates: Vec<Update>,
    /// Apps we could ask about.
    pub checked: u32,
    /// Apps with no source we can check (they usually update themselves).
    pub unchecked: Vec<String>,
    /// Sources that didn't answer, e.g. a feed that's offline.
    pub errors: Vec<(String, String)>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum UpdateEvent {
    Started { id: String, step: String },
    Done { id: String, message: String },
    Failed { id: String, error: String },
}

/// GET over HTTPS with curl (already on every Mac; avoids bundling a TLS stack).
pub(crate) fn fetch(url: &str, max_secs: u32) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err(format!("Refusing insecure URL {url}"));
    }
    let out = Command::new("/usr/bin/curl")
        .args(["-fsSL", "--proto", "=https", "--max-time", &max_secs.to_string(), "-A", "PenCleaner/0.1", url])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// For the examples; the scan reads it itself.
pub fn os_version_for_examples() -> String {
    os_version()
}

pub(crate) fn os_version() -> String {
    Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// Strips HTML to readable text: tags out, block ends to newlines, entities decoded.
pub(crate) fn plain_text(html: &str, max: usize) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if matches!(t.as_str(), "p" | "br" | "li" | "h1" | "h2" | "h3" | "h4" | "div" | "ul" | "ol") {
                    out.push('\n');
                }
                if t == "li" && !tag.starts_with('/') {
                    out.push_str("• ");
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    let decoded = out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ");
    let lines: Vec<&str> = decoded.lines().map(str::trim).filter(|l| !l.is_empty() && *l != "•").collect();
    let mut text = lines.join("\n");
    if text.chars().count() > max {
        text = text.chars().take(max).collect::<String>() + "…";
    }
    text
}

struct Installed {
    info: AppInfo,
    build: Option<String>,
    feed: Option<String>,
    ed_key: Option<String>,
    app_store: bool,
}

fn installed() -> Vec<Installed> {
    apps::installed()
        .into_iter()
        .filter(|a| !a.path.starts_with("/System"))
        .map(|info| {
            let plist = plist::Value::from_file(info.path.join("Contents/Info.plist")).ok();
            let get = |k: &str| plist.as_ref()?.as_dictionary()?.get(k)?.as_string().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
            Installed {
                build: get("CFBundleVersion"),
                feed: get("SUFeedURL").filter(|f| f.starts_with("https://")),
                ed_key: get("SUPublicEDKey"),
                app_store: info.path.join("Contents/_MASReceipt").exists(),
                info,
            }
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreResult {
    bundle_id: String,
    version: String,
    track_id: u64,
    file_size_bytes: Option<String>,
    release_notes: Option<String>,
    minimum_os_version: Option<String>,
    track_view_url: Option<String>,
}

#[derive(Deserialize)]
struct StoreResponse {
    results: Vec<StoreResult>,
}

/// The Mac's storefront: "en_GH" → "gh". Apple's lookup answers per country.
fn country() -> String {
    let out = Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLocale"]).output();
    let locale = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
    locale.split(['_', '@']).nth(1).filter(|c| c.len() == 2).map(str::to_ascii_lowercase).unwrap_or_else(|| "us".into())
}

fn app_store(list: &[&Installed], os: &str, running: &HashSet<String>) -> Result<Vec<Update>, String> {
    if list.is_empty() {
        return Ok(vec![]);
    }
    let lookup = |ids: &[&str], country: &str| -> Result<Vec<StoreResult>, String> {
        let url = format!("https://itunes.apple.com/lookup?entity=macSoftware&country={country}&bundleId={}", ids.join(","));
        let body = fetch(&url, 30)?;
        serde_json::from_slice::<StoreResponse>(&body).map(|r| r.results).map_err(|e| e.to_string())
    };
    let ids: Vec<&str> = list.iter().map(|a| a.info.bundle_id.as_str()).collect();
    let mut results = lookup(&ids, &country())?;
    // Apps missing from the local storefront: ask the US one.
    let found: HashSet<String> = results.iter().map(|r| r.bundle_id.to_ascii_lowercase()).collect();
    let missing: Vec<&str> = ids.iter().copied().filter(|i| !found.contains(&i.to_ascii_lowercase())).collect();
    if !missing.is_empty() {
        results.extend(lookup(&missing, "us").unwrap_or_default());
    }

    Ok(list
        .iter()
        .filter_map(|a| {
            let r = results.iter().find(|r| r.bundle_id.eq_ignore_ascii_case(&a.info.bundle_id))?;
            let current = a.info.version.as_deref().unwrap_or("0");
            if !version::newer(&r.version, current) {
                return None;
            }
            let requires = r.minimum_os_version.as_deref().filter(|m| version::newer(m, os)).map(|m| format!("Needs macOS {m}"));
            Some(Update {
                id: a.info.path.to_string_lossy().into_owned(),
                name: a.info.name.clone(),
                app_path: Some(a.info.path.to_string_lossy().into_owned()),
                source: Source::AppStore,
                installed: current.to_owned(),
                latest: r.version.clone(),
                size: r.file_size_bytes.as_deref().and_then(|s| s.parse().ok()),
                notes: r.release_notes.as_deref().map(|n| plain_text(n, 900)),
                notes_url: r.track_view_url.clone(),
                one_click: false,
                action_note: Some("Opens in the App Store, which installs it".into()),
                signed: true,
                requires,
                running: running.contains(&a.info.bundle_id.to_ascii_lowercase()),
                plan: Plan::AppStore { track_id: r.track_id },
            })
        })
        .collect())
}

fn sparkle(a: &Installed, os: &str, running: &HashSet<String>) -> Result<Option<Update>, String> {
    let feed = a.feed.as_deref().ok_or("no feed")?;
    let body = fetch(feed, 30)?;
    let releases = appcast::parse(&String::from_utf8_lossy(&body));
    let Some(best) = appcast::best(&releases, os, std::env::consts::ARCH) else { return Ok(None) };
    // Sparkle compares build numbers; apps without one compare marketing versions.
    let installed_build = a.build.as_deref().or(a.info.version.as_deref()).unwrap_or("0");
    if !version::newer(&best.version, installed_build) {
        return Ok(None);
    }
    let latest = best.short_version.clone().unwrap_or_else(|| best.version.clone());
    let lower = best.url.to_ascii_lowercase();
    let archive_ok = [".zip", ".dmg", ".tar.gz", ".tgz", ".tar.bz2", ".tbz", ".tar.xz"].iter().any(|e| lower.split('?').next().unwrap_or("").ends_with(e));
    let action_note = (!archive_ok).then(|| "Installer package: opens the app so it can update itself".to_string());
    Ok(Some(Update {
        id: a.info.path.to_string_lossy().into_owned(),
        name: a.info.name.clone(),
        app_path: Some(a.info.path.to_string_lossy().into_owned()),
        source: Source::Sparkle,
        installed: a.info.version.clone().unwrap_or_else(|| installed_build.to_owned()),
        latest,
        size: best.length,
        notes: best.notes_html.as_deref().map(|n| plain_text(n, 900)).filter(|n| !n.is_empty()),
        notes_url: best.notes_link.clone(),
        one_click: archive_ok,
        action_note,
        signed: a.ed_key.is_some() && best.ed_signature.is_some(),
        requires: None,
        running: running.contains(&a.info.bundle_id.to_ascii_lowercase()),
        plan: Plan::Sparkle { release: Box::new(best.clone()), ed_key: a.ed_key.clone(), bundle_id: a.info.bundle_id.clone() },
    }))
}

const BREW: [&str; 2] = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];

pub(crate) fn brew() -> Option<&'static str> {
    BREW.into_iter().find(|b| Path::new(b).exists())
}

pub(crate) fn brew_cmd(args: &[&str]) -> Result<Vec<u8>, String> {
    let bin = brew().ok_or("Homebrew isn't installed")?;
    let out = Command::new(bin)
        .args(args)
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .env("NONINTERACTIVE", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or("brew failed").to_owned())
    }
}

fn homebrew(by_path: &HashMap<PathBuf, &Installed>, running: &HashSet<String>) -> Result<Vec<Update>, String> {
    if brew().is_none() {
        return Ok(vec![]);
    }
    let outdated: serde_json::Value = serde_json::from_slice(&brew_cmd(&["outdated", "--cask", "--greedy", "--json=v2"])?).map_err(|e| e.to_string())?;
    let casks = outdated["casks"].as_array().cloned().unwrap_or_default();
    if casks.is_empty() {
        return Ok(vec![]);
    }
    let tokens: Vec<String> = casks.iter().filter_map(|c| c["name"].as_str().map(str::to_owned)).collect();
    let mut args = vec!["info", "--cask", "--json=v2"];
    args.extend(tokens.iter().map(String::as_str));
    let info: serde_json::Value = serde_json::from_slice(&brew_cmd(&args)?).map_err(|e| e.to_string())?;
    let infos = info["casks"].as_array().cloned().unwrap_or_default();

    Ok(casks
        .iter()
        .filter_map(|c| {
            let token = c["name"].as_str()?;
            let i = infos.iter().find(|i| i["token"].as_str() == Some(token));
            let artifacts = i.and_then(|i| i["artifacts"].as_array()).cloned().unwrap_or_default();
            let app = artifacts.iter().find_map(|a| a["app"].as_array()?.first()?.as_str().map(|n| PathBuf::from("/Applications").join(n)));
            let has_pkg = artifacts.iter().any(|a| a.get("pkg").is_some());
            let installed = c["installed_versions"].as_array()?.first()?.as_str()?.to_owned();
            // Cask versions carry build suffixes ("3.39.13,9bmfVHa69NJ,a"); show the first part.
            let latest = c["current_version"].as_str()?.split(',').next()?.to_owned();
            let known = app.as_ref().and_then(|p| by_path.get(p));
            let name = known
                .map(|k| k.info.name.clone())
                .or_else(|| i.and_then(|i| i["name"].as_array()?.first()?.as_str().map(str::to_owned)))
                .unwrap_or_else(|| token.to_owned());
            Some(Update {
                id: app.as_ref().filter(|p| p.exists()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| format!("brew:{token}")),
                name,
                app_path: app.filter(|p| p.exists()).map(|p| p.to_string_lossy().into_owned()),
                source: Source::Homebrew,
                installed,
                latest,
                size: None,
                notes: i.and_then(|i| i["desc"].as_str()).map(str::to_owned),
                notes_url: i.and_then(|i| i["homepage"].as_str()).map(str::to_owned),
                one_click: !has_pkg,
                action_note: has_pkg.then(|| format!("Needs an installer: run `brew upgrade --cask {token}` in Terminal")),
                signed: false,
                requires: None,
                running: known.is_some_and(|k| running.contains(&k.info.bundle_id.to_ascii_lowercase())),
                plan: Plan::Homebrew { token: token.to_owned() },
            })
        })
        .collect())
}

#[derive(Deserialize)]
struct Cask {
    token: String,
    version: String,
    #[serde(default)]
    auto_updates: Option<bool>,
    homepage: Option<String>,
    desc: Option<String>,
    #[serde(default)]
    artifacts: Vec<serde_json::Value>,
    #[serde(default)]
    deprecated: bool,
    #[serde(default)]
    disabled: bool,
}

const CATALOG_URL: &str = "https://formulae.brew.sh/api/cask.json";

/// Homebrew's public catalog, re-downloaded at most daily and only when it changed.
fn catalog_data(cache_dir: &Path) -> Result<Vec<Cask>, String> {
    let file = cache_dir.join("cask.json");
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age.as_secs() < 86_400));
    if !fresh {
        std::fs::create_dir_all(cache_dir).map_err(|e| e.to_string())?;
        let tmp = cache_dir.join("cask.json.part");
        let mut args = vec!["-fsSL".to_string(), "--proto".into(), "=https".into(), "--max-time".into(), "180".into()];
        if file.exists() {
            // If-Modified-Since: an unchanged catalog costs one tiny request.
            args.extend(["-z".into(), file.to_string_lossy().into_owned()]);
        }
        args.extend(["-o".into(), tmp.to_string_lossy().into_owned(), CATALOG_URL.into()]);
        let ok = Command::new("/usr/bin/curl").args(&args).status().map(|s| s.success()).unwrap_or(false);
        if ok && std::fs::metadata(&tmp).is_ok_and(|m| m.len() > 0) {
            std::fs::rename(&tmp, &file).map_err(|e| e.to_string())?;
        } else {
            let _ = std::fs::remove_file(&tmp);
            if ok && file.exists() {
                // 304 Not Modified: mark the cached copy fresh again.
                let _ = std::fs::File::options().append(true).open(&file).and_then(|f| f.set_modified(std::time::SystemTime::now()));
            } else if !file.exists() {
                return Err("Couldn't download Homebrew's app catalog".into());
            }
        }
    }
    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Both versions in a shape that compares meaningfully, or None. Guards against
/// catalogs that version differently from the app ("5071136" vs "1.0.12",
/// Brave's "1.98.60" vs its own "120.1.61").
pub(crate) fn comparable(installed: &str, latest: &str) -> Option<(String, String)> {
    let latest = latest.split(',').next()?.trim();
    // Drop a trailing commit hash: "3.6.10-ffeefe5d" → "3.6.10".
    let latest = match latest.rsplit_once('-') {
        Some((head, tail)) if tail.len() >= 7 && tail.chars().all(|c| c.is_ascii_hexdigit()) => head,
        _ => latest,
    };
    let installed = installed.trim_start_matches(|c: char| !c.is_ascii_digit()).trim();
    if latest == "latest" || !latest.starts_with(|c: char| c.is_ascii_digit()) || installed.is_empty() {
        return None;
    }
    let numeric = |v: &str| -> Vec<u64> {
        v.split(['.', '-', ' ']).map_while(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()).collect()
    };
    let (a, b) = (numeric(installed), numeric(latest));
    if a.is_empty() || b.is_empty() || (a.len() == 1) != (b.len() == 1) {
        return None;
    }
    let (lo, hi) = (a[0].min(b[0]), a[0].max(b[0]));
    if (lo == 0 && hi > 5) || (lo > 0 && hi / lo > 4) {
        return None;
    }
    Some((installed.to_owned(), latest.to_owned()))
}

fn catalog(apps: &[&Installed], casks: &[Cask], running: &HashSet<String>) -> Vec<Update> {
    // App file name → cask; the main cask beats @beta / @nightly variants.
    let mut by_app: HashMap<String, &Cask> = HashMap::new();
    for c in casks.iter().filter(|c| !c.deprecated && !c.disabled) {
        for a in &c.artifacts {
            for name in a["app"].as_array().into_iter().flatten().filter_map(|n| n.as_str()) {
                let slot = by_app.entry(name.to_owned()).or_insert(c);
                if slot.token.contains('@') && !c.token.contains('@') {
                    *slot = c;
                }
            }
        }
    }
    apps.iter()
        .filter_map(|a| {
            let file = a.info.path.file_name()?.to_string_lossy().into_owned();
            let c = by_app.get(&file)?;
            let (installed, latest) = comparable(a.info.version.as_deref()?, &c.version)?;
            if !version::newer(&latest, &installed) {
                return None;
            }
            let updates_itself = c.auto_updates == Some(true);
            Some(Update {
                id: a.info.path.to_string_lossy().into_owned(),
                name: a.info.name.clone(),
                app_path: Some(a.info.path.to_string_lossy().into_owned()),
                source: Source::Catalog,
                installed,
                latest,
                size: None,
                notes: c.desc.clone(),
                notes_url: c.homepage.clone(),
                one_click: false,
                action_note: Some(if updates_itself {
                    "Updates itself: opens the app so it can install the update".into()
                } else {
                    "Opens the developer's website to download it".into()
                }),
                signed: false,
                requires: None,
                running: running.contains(&a.info.bundle_id.to_ascii_lowercase()),
                plan: Plan::Open { url: (!updates_itself).then(|| c.homepage.clone()).flatten() },
            })
        })
        .collect()
}

/// `cache_dir` holds the Homebrew catalog between scans; None skips that source.
pub fn scan(cache_dir: Option<&Path>, on_step: impl Fn(&str) + Sync) -> UpdatesReport {
    let started = Instant::now();
    let os = os_version();
    let running = apps::running_bundle_ids();
    let all = installed();
    let by_path: HashMap<PathBuf, &Installed> = all.iter().map(|a| (a.info.path.clone(), a)).collect();

    let store: Vec<&Installed> = all.iter().filter(|a| a.app_store).collect();
    let feeds: Vec<&Installed> = all.iter().filter(|a| !a.app_store && a.feed.is_some()).collect();

    let mut errors = Vec::new();
    let (store_res, (feed_res, brew_res)) = rayon::join(
        || {
            on_step("Asking the App Store…");
            app_store(&store, &os, &running)
        },
        || {
            rayon::join(
                || {
                    feeds
                        .par_iter()
                        .map(|a| {
                            on_step(&format!("Checking {}…", a.info.name));
                            (a.info.name.clone(), sparkle(a, &os, &running))
                        })
                        .collect::<Vec<_>>()
                },
                || {
                    on_step("Asking Homebrew…");
                    homebrew(&by_path, &running)
                },
            )
        },
    );

    let mut updates = Vec::new();
    match store_res {
        Ok(u) => updates.extend(u),
        Err(e) => errors.push(("App Store".into(), e)),
    }
    for (name, r) in feed_res {
        match r {
            Ok(Some(u)) => updates.push(u),
            Ok(None) => {}
            Err(e) => errors.push((name, e)),
        }
    }
    // One row per app: the App Store or the app's own feed beats Homebrew's copy of it.
    let have: HashSet<String> = updates.iter().map(|u| u.id.clone()).collect();
    match brew_res {
        Ok(u) => updates.extend(u.into_iter().filter(|u| !have.contains(&u.id))),
        Err(e) => errors.push(("Homebrew".into(), e)),
    }

    // Everything no other source answered for: try Homebrew's catalog.
    let brew_apps: HashSet<String> = updates.iter().filter(|u| u.source == Source::Homebrew).filter_map(|u| u.app_path.clone()).collect();
    let covered: HashSet<&Path> = store.iter().chain(&feeds).map(|a| a.info.path.as_path()).collect();
    let rest: Vec<&Installed> = all
        .iter()
        .filter(|a| !covered.contains(a.info.path.as_path()) && !brew_apps.contains(&*a.info.path.to_string_lossy()))
        .filter(|a| !a.info.bundle_id.starts_with("com.apple."))
        .collect();
    let casks = match cache_dir {
        Some(dir) => {
            on_step("Checking Homebrew's catalog…");
            catalog_data(dir).unwrap_or_else(|e| {
                errors.push(("Homebrew catalog".into(), e));
                vec![]
            })
        }
        None => vec![],
    };
    updates.extend(catalog(&rest, &casks, &running));
    updates.sort_by_key(|u| (u.requires.is_some(), u.name.to_lowercase()));

    // Apps no source knows at all. (The catalog knows more apps than it reports on:
    // those whose versions it can't compare stay quiet rather than guess.)
    let known: HashSet<&str> = casks
        .iter()
        .flat_map(|c| c.artifacts.iter())
        .flat_map(|a| a["app"].as_array().into_iter().flatten())
        .filter_map(|n| n.as_str())
        .collect();
    let mut unchecked: Vec<String> = rest
        .iter()
        .filter(|a| !a.info.path.file_name().is_some_and(|f| known.contains(f.to_string_lossy().as_ref())))
        .map(|a| a.info.name.clone())
        .collect();
    unchecked.sort_by_key(|n| n.to_lowercase());
    unchecked.dedup();

    UpdatesReport {
        checked: (all.len() - unchecked.len()) as u32,
        unchecked,
        errors,
        updates,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

impl UpdatesReport {
    /// Drops skipped versions ("updates:<id>@<version>"): a newer one shows up again.
    pub fn apply_ignore(&mut self, ig: &crate::settings::Ignore) {
        self.updates.retain(|u| !ig.item(&format!("updates:{}@{}", u.id, u.latest)) && !u.app_path.as_deref().is_some_and(|p| ig.path_str(p)));
    }

    /// Installs the chosen updates one at a time and drops the successful ones.
    pub fn install(&mut self, ids: &[String], mut on_event: impl FnMut(UpdateEvent)) -> Vec<String> {
        let mut done = Vec::new();
        for id in ids {
            let Some(u) = self.updates.iter().find(|u| &u.id == id).cloned() else { continue };
            let result = install(&u, |step| on_event(UpdateEvent::Started { id: id.clone(), step: step.to_owned() }));
            match result {
                Ok(message) => {
                    // App Store hand-offs aren't installed yet, so they stay listed.
                    if !matches!(u.plan, Plan::AppStore { .. }) {
                        done.push(id.clone());
                    }
                    on_event(UpdateEvent::Done { id: id.clone(), message });
                }
                Err(error) => on_event(UpdateEvent::Failed { id: id.clone(), error }),
            }
        }
        self.updates.retain(|u| !done.contains(&u.id));
        done
    }
}

/// Free space check before downloading: the archive plus room to unpack it.
pub(crate) fn enough_space(bytes: u64) -> bool {
    crate::volumes::list().first().is_none_or(|v| v.free > bytes.saturating_mul(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html_notes() {
        let t = plain_text("<h2>New</h2><ul><li>Faster &amp; smaller</li><li>Fixes</li></ul><p>Thanks!</p>", 200);
        assert_eq!(t, "New\n• Faster & smaller\n• Fixes\nThanks!");
        assert!(plain_text(&"x".repeat(50), 10).ends_with('…'));
    }

    #[test]
    fn compares_only_like_with_like() {
        assert_eq!(comparable("2.3.8", "2.3.10"), Some(("2.3.8".into(), "2.3.10".into())));
        assert_eq!(comparable("Build 4215", "4215"), Some(("4215".into(), "4215".into())));
        assert_eq!(comparable("12.23.8", "12.34.0,abc"), Some(("12.23.8".into(), "12.34.0".into())));
        assert_eq!(comparable("1.0.12", "5071136"), None, "build number vs version");
        assert_eq!(comparable("120.1.61.116", "1.98.60.0"), None, "different schemes");
        assert_eq!(comparable("1.0", "latest"), None);
        assert_eq!(comparable("3.6.7", "3.6.10-ffeefe5d"), Some(("3.6.7".into(), "3.6.10".into())));
        assert_eq!(comparable("0.12.3", "0.40.12"), Some(("0.12.3".into(), "0.40.12".into())));
    }

    #[test]
    fn refuses_plain_http() {
        assert!(fetch("http://example.com", 5).is_err());
    }
}
