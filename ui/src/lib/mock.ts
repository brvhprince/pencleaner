// Fake backend for running the UI in a plain browser (`pnpm dev`, then open
// http://localhost:1420). Lets the UI be designed and screenshotted without Tauri.
import type {
  AppEntry,
  AppTarget,
  AppsReport,
  Category,
  ClutterReport,
  FileInfo,
  FileKind,
  FileSet,
  CleanEvent,
  CleanSummary,
  CleanupGroup,
  CleanupItem,
  CleanupReport,
  Entry,
  EntryKind,
  ItemKind,
  Listing,
  Progress,
  RemoveEvent,
  RemoveSummary,
  ScanSummary,
  SystemInfo,
  Volume,
} from "./api";

const GB = 1e9;
const MB = 1e6;

type Node = { size: number; items?: number; kind?: EntryKind; children?: Record<string, Node> };

const leaf = (size: number, kind: EntryKind = "file"): Node => ({ size, kind });
const dir = (children: Record<string, Node>, extra = 0): Node => ({
  children,
  size: Object.values(children).reduce((s, c) => s + c.size, 0) + extra,
  items: Object.values(children).reduce((s, c) => s + 1 + (c.items ?? 0), 0) + Math.round(extra / 2e5),
});

const HOME = "/Users/wanpeninsula";

const TREE: Node = dir({
  Users: dir({
    wanpeninsula: dir({
      Sites: dir({ js: dir({ "pen-pos": dir({ "src-tauri": dir({ target: dir({}, 38 * GB) }, 1 * GB), node_modules: dir({}, 1.4 * GB) }), veefyed: dir({ task: dir({ android: dir({}, 21 * GB) }) }), simpleworship: dir({ target: dir({}, 29 * GB) }, 2 * GB) }, 150 * GB), "install.zip": leaf(0.1 * GB) }, 80 * GB),
      Library: dir({ Containers: dir({}, 61 * GB), "Application Support": dir({}, 88 * GB), Caches: dir({}, 34 * GB), Developer: dir({ Xcode: dir({}, 41 * GB) }), Mail: dir({}, 9 * GB) }, 11 * GB),
      Pictures: dir({ "Photos Library.photoslibrary": { ...dir({}, 66 * GB), kind: "bundle" } }, 1.9 * GB),
      Documents: dir({ Works: dir({}, 40 * GB), "old-backup.zip": leaf(6.3 * GB) }, 17 * GB),
      ".colima": dir({}, 24.9 * GB),
      Applications: dir({}, 12.3 * GB),
      ".android": dir({}, 5.7 * GB),
      Downloads: dir({ "Xcode_16.xip": leaf(3.4 * GB), "Figma.dmg": leaf(0.4 * GB) }, 2.4 * GB),
      ".npm": dir({}, 3.1 * GB),
      Movies: dir({}, 1.2 * GB),
    }),
    Shared: dir({}, 0.4 * GB),
  }),
  Applications: dir({
    "Xcode.app": { ...dir({}, 5.4 * GB), kind: "app" },
    "Adobe Photoshop 2026": dir({}, 5.2 * GB),
    "Adobe Lightroom CC": dir({}, 3.6 * GB),
    MAMP: dir({}, 3.4 * GB),
    "Microsoft Word.app": { ...dir({}, 2.9 * GB), kind: "app" },
    "Microsoft Excel.app": { ...dir({}, 2.7 * GB), kind: "app" },
    "Visual Studio Code.app": { ...dir({}, 1.5 * GB), kind: "app" },
    "Google Chrome.app": { ...dir({}, 1.5 * GB), kind: "app" },
  }, 28.8 * GB),
  usr: dir({ local: dir({ share: dir({ dotnet: dir({}, 14.3 * GB) }) }, 9 * GB) }, 2.7 * GB),
  System: dir({}, 25.8 * GB),
  Library: dir({}, 22 * GB),
  private: dir({}, 6.1 * GB),
  bin: dir({}, 4.9 * MB),
  sbin: dir({}, 2.5 * MB),
});

function nodeAt(path: string): Node | null {
  let n: Node | undefined = TREE;
  for (const part of path.split("/").filter(Boolean)) n = n?.children?.[part];
  return n ?? null;
}

function protectedReason(path: string): string | null {
  const parts = path.split("/").filter(Boolean);
  if (parts.length <= 1) return "Top-level system folder";
  if (path.startsWith("/System") || (path.startsWith("/usr") && !path.startsWith("/usr/local/"))) return "Part of macOS";
  if (parts[0] === "Users" && parts.length === 2) return "User home folder";
  if (path.startsWith(HOME + "/")) {
    const rel = path.slice(HOME.length + 1).split("/");
    if (rel.length === 1 && ["Library", "Documents", "Pictures", "Sites", "Downloads", "Movies", "Applications"].includes(rel[0]))
      return "Standard macOS folder";
    if (rel.length === 2 && rel[0] === "Library") return "Library folder used by macOS and apps";
  }
  return null;
}

const removed = new Set<string>();

const now = Math.floor(Date.now() / 1000);
const days = (n: number) => now - n * 86400;

type MockItem = [name: string, gb: number, recommended: boolean, extra?: Partial<CleanupItem>];

function mkGroup(id: string, category: Category, title: string, description: string, kind: ItemKind, rows: MockItem[], note: string | null = null): CleanupGroup {
  const items = rows.map(([name, gb, recommended, extra]) => ({
    id: `${id}:${name}`,
    name,
    detail: null,
    path: `${HOME}/${name}`,
    size: Math.round(gb * GB),
    kind,
    mode: "delete" as const,
    lastActivity: null,
    recommended,
    warning: null,
    ...extra,
  }));
  return { id, category, title, description, items, size: items.reduce((s, i) => s + i.size, 0), note };
}

const project = (name: string, gb: number, ageDays: number, artifacts: string): MockItem => [
  name,
  gb,
  ageDays > 30,
  { detail: `~/Sites/js/${name} · ${artifacts}`, lastActivity: days(ageDays) },
];

let cleanupReport: CleanupReport | null = null;

function file(path: string, mb: number, kind: FileKind, extra: Partial<FileInfo> = {}): FileInfo {
  return {
    path: `${HOME}/${path}`,
    name: path.split("/").pop()!,
    size: Math.round(mb * MB),
    kind,
    modified: days(400),
    created: days(700),
    lastUsed: null,
    added: null,
    source: null,
    width: null,
    height: null,
    ...extra,
  };
}

function set(files: FileInfo[]): FileSet {
  return { id: `set:${files[0].path}`, files, keep: files[0].path, reclaimable: files.slice(1).reduce((s, f) => s + f.size, 0) };
}

const photo = (path: string, mb: number, ageDays: number) => file(path, mb, "image", { width: 4032, height: 3024, modified: days(ageDays) });

const rel = (label: string, path: string, mb: number, admin = false) => ({ label, path, size: Math.round(mb * MB), admin });

function app(name: string, id: string, appMb: number, usedDays: number | null, related: ReturnType<typeof rel>[], extra: Partial<AppEntry> = {}): AppEntry {
  return {
    id,
    name,
    version: "1.0",
    path: `/Applications/${name}.app`,
    size: Math.round(appMb * MB),
    related,
    relatedSize: related.reduce((s, f) => s + f.size, 0),
    lastUsed: usedDays === null ? null : days(usedDays),
    appStore: false,
    running: false,
    ...extra,
  };
}

let appsReport: AppsReport | null = null;

function buildApps(): AppsReport {
  return {
    elapsedMs: 6900,
    cancelled: false,
    apps: [
      app("Claude", "com.anthropic.claudefordesktop", 659, 1, [rel("Application Support", `${HOME}/Library/Application Support/Claude`, 12200), rel("Caches", `${HOME}/Library/Caches/com.anthropic.claudefordesktop`, 18)], { running: true, version: "1.4.2" }),
      app("Adobe Photoshop 2026", "com.adobe.Photoshop", 5180, 5, [rel("Preferences", `${HOME}/Library/Preferences/Adobe Photoshop 2026 Settings`, 3)], { version: "27.1" }),
      app("Xcode", "com.apple.dt.Xcode", 5140, null, [rel("Caches", `${HOME}/Library/Caches/com.apple.dt.Xcode`, 3.2), rel("Launch Daemon", "/Library/LaunchDaemons/com.apple.dt.Xcode.helper.plist", 0.01, true)], { appStore: true, version: "26.0" }),
      app("WhatsApp", "net.whatsapp.WhatsApp", 291, 0, [rel("Group Container", `${HOME}/Library/Group Containers/group.net.whatsapp.WhatsApp.shared`, 4300)], { appStore: true }),
      app("Wondershare Filmora Mac", "com.wondershare.filmoramacos", 1190, null, [rel("Application Support", `${HOME}/Library/Application Support/Wondershare Filmora Mac`, 1990)]),
      app("zoom.us", "us.zoom.xos", 410, 220, [
        rel("Application Support", `${HOME}/Library/Application Support/zoom.us`, 0.7),
        rel("Helper Tool", "/Library/PrivilegedHelperTools/us.zoom.ZoomDaemon", 0.3, true),
        rel("Launch Daemon", "/Library/LaunchDaemons/us.zoom.ZoomDaemon.plist", 0.01, true),
        rel("Preferences", `${HOME}/Library/Preferences/us.zoom.xos.plist`, 0.01),
      ]),
      app("DataGrip", "com.jetbrains.datagrip", 2730, null, []),
      app("Microsoft Excel", "com.microsoft.Excel", 2740, null, [rel("Container", `${HOME}/Library/Containers/com.microsoft.Excel`, 8)]),
    ],
    leftovers: [
      { id: "net.metaquotes.wine", vendorInstalled: false, files: [rel("Application Support", `${HOME}/Library/Application Support/net.metaquotes.wine`, 1200)], size: 1200 * MB, lastActivity: days(561) },
      { id: "com.wondershare.installer", vendorInstalled: true, files: [rel("Application Support", "", 500), rel("WebKit Data", "", 17.6)], size: 517.6 * MB, lastActivity: days(647) },
      { id: "com.tinyspeck.slackmacgap", vendorInstalled: false, files: [rel("HTTP Storage", "", 0.06), rel("Preferences", "", 0.04)], size: 0.1 * MB, lastActivity: days(345) },
      { id: "com.surfshark.vpn", vendorInstalled: false, files: [rel("Group Container", "", 2), rel("App Scripts", "", 1)], size: 3 * MB, lastActivity: days(1131) },
      { id: "com.adobe.aftereffects", vendorInstalled: true, files: [rel("WebKit Data", "", 0.5), rel("Preferences", "", 0.4)], size: 0.9 * MB, lastActivity: days(104) },
    ],
    installers: [
      { path: `${HOME}/Downloads/Figma-125.4.dmg`, name: "Figma-125.4.dmg", size: 412 * MB, added: days(95), installedApp: null, recommended: true },
      { path: `${HOME}/Downloads/Zoom.pkg`, name: "Zoom.pkg", size: 64 * MB, added: days(12), installedApp: "zoom.us", recommended: true },
    ],
  };
}

function buildClutter(): ClutterReport {
  return {
    root: HOME,
    filesScanned: 391577,
    elapsedMs: 43500,
    cancelled: false,
    large: [
      file("Documents/Ideas/flowing-particles-collision-logo.zip", 1450, "archive"),
      file("Documents/Ideas/flow-logo-reveal-2023-11-27.zip", 1220, "archive"),
      file("Documents/Works/Poppa/Thanksgiving/IMG_1823.MOV", 893, "video", { lastUsed: days(569) }),
      file("Documents/Ideas/fleet_travel_shopping_ui_kit.zip", 725, "archive"),
      file("Documents/Works/Home/10X25.skp", 572, "document", { lastUsed: days(805) }),
      file("Sites/js/veefyed/task/android/app/release/app-release.aab", 145, "installer", { lastUsed: days(402), modified: days(402) }),
      file("Movies/screen-recording-demo.mov", 312, "video", { lastUsed: days(40), modified: days(40) }),
    ],
    duplicates: [
      set([file("Documents/Works/NAEC/ID Cards/id-card-holder-mockup.zip", 223.5, "archive"), file("Downloads/id-card-holder-mockup.zip", 223.5, "archive", { modified: days(30) })]),
      set([file("Documents/Works/Pencasa/clean-corporate-presets.zip", 141.4, "archive"), file("Desktop/clean-corporate-presets.zip", 141.4, "archive")]),
      set([file("Documents/Works/Bulk SMS/Metronic_v9.2.2.fig", 130.3, "document"), file("Documents/Works/Bulk SMS/copy/Metronic_v9.2.2.fig", 130.3, "document")]),
      set([file("Documents/Audio/Shiloh/Supernatural Supplies.mp3", 42.7, "audio"), file("Downloads/Supernatural Supplies.mp3", 42.7, "audio"), file("Desktop/Supernatural Supplies (1).mp3", 42.7, "audio")]),
    ],
    similar: [
      set([photo("Pictures/Thanksgiving/IMG_1779.jpg", 5.4, 300), photo("Pictures/Thanksgiving/IMG_1777.jpg", 5.1, 300), photo("Pictures/Thanksgiving/IMG_1779-2.jpg", 4.9, 300), photo("Pictures/Thanksgiving/IMG_1777.heic", 2.6, 300)]),
      set([photo("Documents/Works/Cards/back-card.tif", 88.1, 500), photo("Documents/Works/Cards/front-card.tif", 88.1, 500)]),
      set([photo("Pictures/Thanksgiving/IMG_1869.jpg", 7.4, 280), photo("Pictures/Thanksgiving/IMG_1868.jpg", 7.4, 280), photo("Pictures/Thanksgiving/IMG_1869-2.jpg", 7.3, 280)]),
    ],
    downloads: [
      file("Downloads/metronic-950", 2550, "folder", { added: days(120) }),
      file("Downloads/Nicholas", 453, "folder", { added: days(60) }),
      file("Downloads/themeforest-huro-admin.zip", 167, "archive", { added: days(210), source: "mega.nz" }),
      file("Downloads/Figma.dmg", 412, "installer", { added: days(95), source: "figma.com" }),
      file("Downloads/penvote-full.zip", 32.8, "archive", { added: days(15), source: "drive.usercontent.google.com" }),
      file("Downloads/invoice-0042.pdf", 0.4, "document", { added: days(3), source: "mail.google.com" }),
    ],
  };
}

function buildReport(): CleanupReport {
  return {
    elapsedMs: 45700,
    fullDiskAccess: true,
    cancelled: false,
    groups: [
      mkGroup("user-caches", "system", "User Cache Files", "Data apps keep around to start and load faster. They rebuild it as needed.", "folder", [
        ["Adobe After Effects", 82.48, true, { detail: "~/Library/Caches/Adobe/After Effects" }],
        ["com.apple.mediaanalysisd", 0.54, true, { detail: "~/Library/Containers/com.apple.mediaanalysisd/Data/Library/Caches" }],
        ["penflow", 0.52, true, { detail: "~/Library/Caches/penflow" }],
        ["Google Chrome", 0.34, false, { detail: "~/Library/Caches/Google/Chrome", warning: "Google Chrome is running. Quit it first." }],
        ["Adobe Camera Raw 2", 0.33, true, { detail: "~/Library/Caches/Adobe Camera Raw 2" }],
        ["GeoServices", 0.08, true, { detail: "~/Library/Caches/GeoServices" }],
      ]),
      mkGroup("user-logs", "system", "User Log Files", "Activity and crash logs written by apps and macOS.", "folder", [
        ["CreativeCloud", 0.51, true],
        ["JetBrains", 0.02, true],
        ["Claude", 0.01, true],
      ]),
      mkGroup("projects", "developer", "Project Build Artifacts", "Dependencies and build output your tools recreate with one install or build. Projects untouched for 30 days are pre-selected.", "project", [
        project("simpleworship", 70.24, 2, "apps/desktop/node_modules, apps/remote/node_modules, target"),
        project("churchcms-desktop", 27.31, 140, "node_modules, src-tauri/target"),
        project("tenancy", 25.77, 210, "api/node_modules, desktop/admin/node_modules"),
        project("penmedia", 13.19, 64, ".turbo, apps/desktop/node_modules"),
        project("penflow", 8.15, 9, "node_modules, src-tauri/target"),
        project("penhomes", 7.02, 18, "android/.gradle, android/app/build"),
        project("penvision", 5.39, 95, "node_modules, src-tauri/target"),
        project("pen-pos", 5.29, 120, "node_modules, src-tauri/target"),
      ]),
      mkGroup("xcode", "developer", "Xcode", "Build products, device support files and caches Xcode recreates on demand.", "folder", [
        ["Archives", 9.35, false, { warning: "Needed to symbolicate crash reports from builds you've shipped.", mode: "trash" }],
        ["Interface Builder support", 3.75, true],
        ["Xamarin", 1.34, false],
      ]),
      mkGroup("simulators", "developer", "iOS Simulators", "Simulated devices and their installed apps. Ones whose runtime is gone are pre-selected.", "device", [
        ["iPhone 16 Pro Max (iOS 18.6)", 4.79, false, { detail: "Shutdown" }],
        ["iPhone 17 Pro Max (iOS 26.5)", 2.8, true, { detail: "Runtime no longer installed" }],
        ["iPhone 16 Pro (iOS 18.6)", 2.57, false, { detail: "Shutdown" }],
      ]),
      mkGroup("package-caches", "developer", "Package Manager Caches", "Downloaded packages and toolchains. Everything here downloads again when a project needs it.", "folder", [
        ["pnpm store", 8.59, false, { warning: "Projects hard-link files from here, so less space may be freed than shown." }],
        ["pnpm metadata cache", 1.52, true],
        ["Puppeteer browsers", 1.36, false],
        ["Bun cache", 0.7, true],
      ]),
      mkGroup("virtual-machines", "developer", "VMs & Emulators", "Container VMs and Android emulators. Large, but each holds state you may want.", "device", [
        ["Colima", 24.94, false, { warning: "Removes every Colima VM with all its images, containers and volumes." }],
        ["Pendroid (Android emulator)", 5.71, false],
      ]),
      mkGroup("ai-models", "ai", "Local AI Models", "Models downloaded for local AI tools. Large, and they download again if you need them.", "model", [
        ["llama3:latest", 4.66, false, { detail: "Ollama · ~/.ollama/models", lastActivity: days(200) }],
      ]),
      mkGroup("ai-caches", "ai", "AI Tool Caches", "Indexes, logs and downloads from AI assistants and runtimes.", "folder", [["GitHub Copilot cache", 1.86, true]]),
      mkGroup("mail", "mail", "Mail Attachments", "Local copies of attachments that Mail keeps on disk.", "folder", [
        ["Downloaded attachments", 0.19, false, { warning: "POP attachments are gone for good." }],
      ]),
      mkGroup("trash", "trash", "Trash Bins", "Everything in the Trash on this Mac and connected drives.", "trash", [
        ["Trash on WANPEN", 0.27, true, { detail: "1 item", warning: "Emptying the Trash can't be undone." }],
      ]),
    ],
  };
}
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const mock = {
  async systemInfo(): Promise<SystemInfo> {
    return { fullDiskAccess: !location.hash.includes("nofda"), home: HOME, user: "wanpeninsula", devBuild: true };
  },
  async volumes(): Promise<Volume[]> {
    return [
      { name: "WANPEN", mount: "/", total: 1000 * GB, used: 827 * GB, free: 173 * GB, isRoot: true, fsType: "apfs" },
      { name: "Backup", mount: "/Volumes/Backup", total: 2000 * GB, used: 612 * GB, free: 1388 * GB, isRoot: false, fsType: "apfs" },
    ];
  },
  async scan(root: string, onProgress: (p: Progress) => void): Promise<ScanSummary> {
    const total = nodeAt(root)?.size ?? 0;
    const steps = location.hash.includes("slowscan") ? 400 : 12;
    for (let i = 1; i <= steps; i++) {
      onProgress({ items: Math.round((i / steps) * 8.1e6), bytes: (i / steps) * total, current: `${HOME}/Sites/js/simpleworship/target/debug/deps/lib${i}.rlib` });
      await wait(120);
    }
    return { root, size: total, items: TREE.items ?? 0, denied: 3, cancelled: false, elapsedMs: 1500 };
  },
  async list(path: string): Promise<Listing> {
    const node = nodeAt(path);
    if (!node) throw new Error(`${path} is not inside the scanned folder`);
    const entries: Entry[] = Object.entries(node.children ?? {})
      .map(([name, c]) => {
        const p = path === "/" ? `/${name}` : `${path}/${name}`;
        const kind: EntryKind = c.kind ?? (c.children ? "folder" : "file");
        return { name, path: p, size: c.size, items: c.items ?? 0, kind, protected: protectedReason(p) };
      })
      .filter((e) => !removed.has(e.path))
      .sort((a, b) => b.size - a.size);
    const name = path === "/" ? "WANPEN" : path.split("/").pop()!;
    return { path, name, size: node.size, items: node.items ?? 0, denied: path === "/private", entries, restCount: 0, restSize: 0 };
  },
  async appsScan(onProgress: (p: Progress) => void): Promise<AppsReport> {
    for (let i = 1; i <= 10; i++) {
      onProgress({ items: i * 400, bytes: i * 9 * GB, current: `/Applications/App ${i}.app` });
      await wait(110);
    }
    appsReport = buildApps();
    return appsReport;
  },
  async appsRemove(targets: AppTarget[], onEvent: (e: CleanEvent) => void): Promise<CleanSummary> {
    const out: CleanSummary = { cleaned: [], failed: [], bytesFreed: 0, byCategory: {} };
    for (const [index, t] of targets.entries()) {
      const a = appsReport?.apps.find((x) => x.id === t.id);
      const size = a ? a.size + a.relatedSize : 0;
      onEvent({ status: "started", id: t.id, name: a?.name ?? t.id, size, index, total: targets.length });
      await wait(400);
      if (a?.running) {
        out.failed.push([t.id, `${a.name} is running. Quit it, then try again.`]);
        onEvent({ status: "failed", id: t.id, error: "running", bytes: 0 });
        continue;
      }
      out.cleaned.push(t.id);
      out.bytesFreed += size;
      onEvent({ status: "cleaned", id: t.id, bytes: size });
    }
    return out;
  },
  async clutterScan(_root: string, onProgress: (p: Progress) => void): Promise<ClutterReport> {
    const steps = location.hash.includes("slowscan") ? 400 : 12;
    for (let i = 1; i <= steps; i++) {
      onProgress({ items: i * 32000, bytes: i * 4 * GB, current: `${HOME}/Documents/Works/NAEC/Designs/teachers-need-${i}.jpg` });
      await wait(110);
    }
    return buildClutter();
  },
  /** Stand-in "photo" so the browser preview shows something in the image grid. */
  thumb(path: string): string {
    let h = 0;
    for (const c of path.replace(/(-\d)?\.\w+$/, "")) h = (h * 31 + c.charCodeAt(0)) % 360;
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 3"><defs><linearGradient id="g" x2="1" y2="1"><stop stop-color="hsl(${h},60%,62%)"/><stop offset="1" stop-color="hsl(${(h + 50) % 360},55%,35%)"/></linearGradient></defs><rect width="4" height="3" fill="url(#g)"/><circle cx="2.8" cy="1" r=".45" fill="hsl(${(h + 180) % 360},70%,85%)"/></svg>`;
    return `data:image/svg+xml,${encodeURIComponent(svg)}`;
  },
  async cleanupScan(_categories: Category[] | null, onProgress: (p: Progress) => void): Promise<CleanupReport> {
    const steps = location.hash.includes("slowscan") ? 400 : 12;
    for (let i = 1; i <= steps; i++) {
      onProgress({ items: i * 900, bytes: (i / steps) * 411 * GB, current: `${HOME}/Sites/js/tenancy/api/node_modules/.pnpm/lib${i}` });
      await wait(110);
    }
    cleanupReport = buildReport();
    return cleanupReport;
  },
  async cleanupClean(ids: string[], onEvent: (e: CleanEvent) => void): Promise<CleanSummary> {
    const all = cleanupReport?.groups.flatMap((g) => g.items.map((i) => ({ i, c: g.category }))) ?? [];
    const chosen = all.filter(({ i }) => ids.includes(i.id));
    const out: CleanSummary = { cleaned: [], failed: [], bytesFreed: 0, byCategory: {} };
    for (const [index, { i, c }] of chosen.entries()) {
      onEvent({ status: "started", id: i.id, name: i.name, size: i.size, index, total: chosen.length });
      await wait(location.hash.includes("slowclean") ? 1200 : 160);
      out.cleaned.push(i.id);
      out.bytesFreed += i.size;
      out.byCategory[c] = (out.byCategory[c] ?? 0) + i.size;
      onEvent({ status: "cleaned", id: i.id, bytes: i.size });
    }
    return out;
  },
  async remove(items: { path: string; size?: number }[], onEvent: (e: RemoveEvent) => void): Promise<RemoveSummary> {
    const out: RemoveSummary = { removed: [], failed: [], bytesFreed: 0 };
    for (const [index, it] of items.entries()) {
      onEvent({ status: "started", path: it.path, index, total: items.length });
      await wait(300);
      removed.add(it.path);
      out.removed.push([it.path, it.size ?? 0]);
      out.bytesFreed += it.size ?? 0;
      onEvent({ status: "removed", path: it.path, bytes: it.size ?? 0 });
    }
    return out;
  },
};
