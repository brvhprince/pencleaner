// Fake backend for running the UI in a plain browser (`pnpm dev`, then open
// http://localhost:1420). Lets the UI be designed and screenshotted without Tauri.
import type {
  IgnoreKind,
  Settings,
  Grant,
  PrivacyItem,
  ProtectionReport,
  WatchAlert,
  ActivityEvent,
  AppUpdate,
  UpdateEvent,
  UpdatesReport,
  LaunchItem,
  PerfReport,
  TaskResult,
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
    id: `/Applications/${name}.app`,
    bundleId: id,
    name,
    version: "1.0",
    path: `/Applications/${name}.app`,
    sharesSupportFiles: false,
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

const item = (name: string, kind: LaunchItem["kind"], extra: Partial<LaunchItem> = {}): LaunchItem => ({
  id: name.toLowerCase().replace(/\s+/g, "."),
  name,
  developer: null,
  kind,
  controllable: true,
  enabled: true,
  running: false,
  plist: `${HOME}/Library/LaunchAgents/${name}.plist`,
  executable: null,
  appPath: null,
  appName: null,
  broken: false,
  isSelf: false,
  ...extra,
});

let perfItems: LaunchItem[] = [
  item("CleanMyMac Menu", "loginItem", { controllable: false, running: true, appName: "CleanMyMac", developer: "MacPaw Way Ltd", plist: null }),
  item("PostgresLoginHelper", "loginItem", { controllable: false, appName: "Postgres", developer: "Jakob Egger", plist: null }),
  item("TMOG Hotkey", "loginItem", { controllable: false, running: true, appName: "Task Manager TMOG", plist: null }),
  item("kafka-server-start", "agent", { id: "homebrew.mxcl.kafka", running: true }),
  item("rabbitmq-server", "agent", { id: "homebrew.mxcl.rabbitmq", running: true }),
  item("Creative Cloud", "agent", { id: "com.adobe.AdobeCreativeCloud", running: true, developer: "Adobe Creative Cloud", plist: "/Library/LaunchAgents/com.adobe.AdobeCreativeCloud.plist" }),
  item("Java Updater", "agent", { id: "com.oracle.java.Java-Updater", developer: "Oracle Java", plist: "/Library/LaunchAgents/com.oracle.java.Java-Updater.plist", broken: true }),
  item("DropboxMacUpdate", "agent", { id: "com.dropbox.DropboxMacUpdate.agent", enabled: false, developer: "Dropbox" }),
  item("cloudflared", "daemon", { id: "com.cloudflare.cloudflared", running: true, plist: "/Library/LaunchDaemons/com.cloudflare.cloudflared.plist" }),
  item("VirtualBoxStartup.sh", "daemon", { id: "org.virtualbox.startup", plist: "/Library/LaunchDaemons/org.virtualbox.startup.plist" }),
  item("us.zoom.ZoomDaemon", "daemon", { id: "us.zoom.ZoomDaemon", developer: "Zoom Video Communications, Inc.", plist: "/Library/LaunchDaemons/us.zoom.ZoomDaemon.plist" }),
];
let snapshots = Array.from({ length: 18 }, (_, i) => ({ id: `2026-10-${String(10 + (i >> 1)).padStart(2, "0")}-0${i % 2}0000`, created: days(18 - i) }));

function buildPerf(): PerfReport {
  return {
    memory: { total: 17.2 * GB, used: 13.2 * GB, app: 7 * GB, wired: 4.3 * GB, compressed: 1.9 * GB, cached: 3.7 * GB, free: 0.3 * GB, swapUsed: 7 * GB, swapTotal: 8.6 * GB, pressure: "normal" },
    processes: [
      { name: "WebStorm", appPath: "/Applications/WebStorm.app", bytes: 3 * GB, processes: 7 },
      { name: "Google Chrome", appPath: "/Applications/Google Chrome.app", bytes: 1.9 * GB, processes: 43 },
      { name: "claude", appPath: null, bytes: 620 * MB, processes: 2 },
      { name: "GitHub Desktop", appPath: "/Applications/GitHub Desktop.app", bytes: 276 * MB, processes: 8 },
      { name: "mdworker_shared", appPath: null, bytes: 250 * MB, processes: 16 },
    ],
    items: perfItems,
    snapshots,
    purgeable: 59 * GB,
    tasks: [
      { id: "free-ram", title: "Free Up RAM", description: "Asks macOS to drop inactive memory and file caches so apps that need memory get it without swapping.", admin: true, slow: false, recommended: true, reason: "7.0 GB of memory has spilled into swap on disk." },
      { id: "flush-dns", title: "Flush DNS Cache", description: "Clears remembered website addresses. Fixes sites that won't load after a network or DNS change.", admin: true, slow: false, recommended: true, reason: "Quick and harmless." },
      { id: "thin-snapshots", title: "Remove Time Machine Snapshots", description: "Deletes local snapshots. Backups on your Time Machine drive are not touched.", admin: true, slow: false, recommended: true, reason: "18 local snapshots are holding about 59.0 GB." },
      { id: "quicklook", title: "Reset Quick Look", description: "Clears cached file previews and thumbnails. Fixes previews that are stale or blank.", admin: false, slow: false, recommended: true, reason: "Quick and harmless." },
      { id: "speed-up-mail", title: "Speed Up Mail", description: "Compacts Mail's message database. Quit Mail first.", admin: false, slow: false, recommended: false, reason: null },
      { id: "rebuild-launch-services", title: "Rebuild Launch Services", description: "Re-registers apps. Fixes duplicate entries in Open With and documents opening in the wrong app.", admin: false, slow: true, recommended: false, reason: null },
      { id: "reindex-spotlight", title: "Reindex Spotlight", description: "Rebuilds the search index from scratch. Fixes missing search results.", admin: true, slow: true, recommended: false, reason: null },
      { id: "font-cache", title: "Clear Font Caches", description: "Removes your font caches. Takes effect after you log out.", admin: false, slow: false, recommended: false, reason: null },
    ],
  };
}

let activity: ActivityEvent[] = location.hash.includes("noactivity")
  ? []
  : [
      { at: days(27), module: "cleanup", action: "Cleaned junk", bytes: 84.5 * GB, items: 11, detail: "system" },
      { at: days(27), module: "performance", action: "Ran maintenance", bytes: 0, items: 3, detail: "free-ram, flush-dns, quicklook" },
      { at: days(21), module: "spacelens", action: "Moved to the Trash", bytes: 24.9 * GB, items: 2, detail: null },
      { at: days(14), module: "cleanup", action: "Cleaned junk", bytes: 85.7 * GB, items: 19, detail: "developer, ai" },
      { at: days(9), module: "clutter", action: "Moved files to the Trash", bytes: 581 * MB, items: 5, detail: null },
      { at: days(6), module: "applications", action: "Removed app leftovers", bytes: 1.7 * GB, items: 5, detail: null },
      { at: days(3), module: "performance", action: "Removed Time Machine snapshots", bytes: 59 * GB, items: 18, detail: null },
      { at: days(1), module: "performance", action: "Switched off a background item", bytes: 0, items: 1, detail: "kafka-server-start" },
      { at: days(0), module: "cleanup", action: "Cleaned junk", bytes: 2.2 * GB, items: 6, detail: "system" },
    ];

const upd = (name: string, source: AppUpdate["source"], installed: string, latest: string, extra: Partial<AppUpdate> = {}): AppUpdate => ({
  id: `/Applications/${name}.app`,
  name,
  appPath: `/Applications/${name}.app`,
  source,
  installed,
  latest,
  size: null,
  notes: null,
  notesUrl: null,
  oneClick: source === "sparkle" || source === "homebrew",
  actionNote: source === "appStore" ? "Opens in the App Store, which installs it" : source === "catalog" ? "Updates itself: opens the app so it can install the update" : null,
  signed: source === "appStore",
  requires: null,
  running: false,
  ...extra,
});

let updates: AppUpdate[] = [
  upd("Maccy", "sparkle", "2.7.1", "2.8.0", { notes: "• Pin items with ⌥P\n• Faster search in long histories\n• Fixes a crash when pasting images" }),
  upd("Transmission", "sparkle", "4.0.6", "4.1.4", { size: 11.5 * MB, signed: false, notes: "Fixes and performance improvements." }),
  upd("iTerm", "sparkle", "3.7.5", "3.7.6", { size: 45 * MB, signed: true, running: true, notes: "Bug fixes." }),
  upd("Wave", "homebrew", "0.10.4", "0.14.5", { notes: "Open-source AI-native terminal" }),
  upd("RAR Archiver", "homebrew", "6.23", "7.23", { id: "brew:rar", appPath: null }),
  upd("WhatsApp", "appStore", "26.38.74", "26.42.74", { size: 418 * MB, notes: "We update the app regularly to fix bugs, optimize performance and improve the experience." }),
  upd("Xcode", "appStore", "26.5", "27.1", { size: 3.1 * GB, notes: "Xcode 27.1 includes Swift 6.4 and SDKs for iOS 27.1." }),
  upd("Developer", "appStore", "10.8.3", "11.1.1", { size: 27.6 * MB, requires: "Needs macOS 27.0" }),
  upd("Google Chrome", "catalog", "156.0.8078.49", "156.0.8078.74"),
  upd("Obsidian", "catalog", "1.14.4", "1.14.5"),
  upd("pgAdmin 4", "catalog", "9.2", "9.18", { actionNote: "Opens the developer's website to download it" }),
];

const threat = location.hash.includes("threat");
const priv = (owner: string, trace: PrivacyItem["trace"], title: string, detail: string, mb: number, extra: Partial<PrivacyItem> = {}): PrivacyItem => ({
  id: `${owner}:${title}`.toLowerCase().replace(/\s+/g, "-"),
  owner,
  appPath: owner === "macOS" ? null : `/Applications/${owner}.app`,
  trace,
  title,
  detail,
  size: mb * MB,
  recommended: trace === "history" || trace === "recentItems",
  warning: null,
  running: false,
  ...extra,
});
const grant = (label: string, appName: string, extra: Partial<Grant> = {}): Grant => ({
  id: `${label}|${appName}`,
  service: label.replace(/\s+/g, ""),
  label,
  settingsAnchor: "Privacy_Camera",
  client: `com.example.${appName.toLowerCase().replace(/\s+/g, "")}`,
  appName,
  appPath: `/Applications/${appName}.app`,
  target: null,
  granted: days(200),
  orphaned: false,
  revocable: !extra.orphaned,
  lockedToSigner: true,
  isSelf: false,
  system: false,
  ...extra,
});
let watcherOn = location.hash.includes("watching");
let watchAlerts: WatchAlert[] = watcherOn
  ? [{ at: days(1), id: "com.updater.helper", name: "updater", plist: `${HOME}/Library/LaunchAgents/com.updater.helper.plist`, program: `${HOME}/Library/.cache/updater`, severity: "high", reasons: ["Runs from a hidden folder", "Its program isn't signed by any developer"], malware: null }]
  : [];
let protectionReport: ProtectionReport | null = null;

function buildProtection(): ProtectionReport {
  return {
    rules: { version: "5368", rules: 589 },
    filesScanned: 1208,
    detections: threat ? [{ file: "/Applications/Search Lite.app/Contents/MacOS/Search Lite", target: "/Applications/Search Lite.app", rule: "MACOS.ADLOAD.D", description: null }] : [],
    suspicious: threat
      ? [
          {
            item: { id: "com.apple.update.helper", name: "update", developer: null, kind: "agent", controllable: true, enabled: true, running: true, plist: `${HOME}/Library/LaunchAgents/com.apple.update.helper.plist`, executable: "/bin/sh", appPath: null, appName: null, broken: false, isSelf: false },
            program: `${HOME}/Library/.hidden/update.sh`,
            signer: { kind: "script" },
            severity: "high",
            reasons: ["Uses an Apple name but isn't signed by Apple", "Runs from a hidden folder"],
          },
        ]
      : [],
    privacy: [
      priv("Safari", "history", "Browsing history", "Visited sites, recently closed tabs and the downloads list", 18, { warning: "With iCloud Safari sync on, history from your other devices may come back." }),
      priv("Safari", "cookies", "Cookies & website data", "Signs you out of websites", 142, { warning: "You'll be signed out of every website in Safari." }),
      priv("Google Chrome", "history", "Browsing history", "History, top sites and the downloads list · 14 profiles", 386, { running: true }),
      priv("Google Chrome", "cookies", "Cookies & website data", "Signs you out of websites · 14 profiles", 1240, { running: true, warning: "You'll be signed out of every website in this browser." }),
      priv("Google Chrome", "autofill", "Autofill data", "Form entries · 14 profiles", 21, { running: true, warning: "Also removes saved addresses, payment cards and custom search engines. Saved passwords are never touched." }),
      priv("Brave", "cookies", "Cookies & website data", "Signs you out of websites · 1 profile", 64),
      priv("macOS", "recentItems", "Recent items", "The Recent Items menu and every app's Open Recent list", 0.3),
    ],
    permissions: [
      grant("Camera", "zoom.us"),
      grant("Camera", "Google Chrome"),
      grant("Microphone", "Adobe Audition"),
      grant("Microphone", "Skype", { orphaned: true, appPath: null }),
      grant("Screen Recording", "RustDesk", { system: true }),
      grant("Screen Recording", "Loom", { orphaned: true, appPath: null, system: true }),
      grant("Accessibility", "Task Manager TMOG", { system: true }),
      grant("Full Disk Access", "Terminal", { system: true, appPath: "/System/Applications/Utilities/Terminal.app" }),
      grant("Full Disk Access", "node", { system: true, revocable: false, client: "/usr/local/bin/node", appPath: null }),
      grant("Automation", "PenCleaner", { target: "System Events" }),
    ],
    permissionsError: null,
    watcher: watcherOn,
    alerts: watchAlerts,
    errors: [],
    elapsedMs: 14000,
    cancelled: false,
  };
}

const protection = {
  async scan(onProgress: (p: Progress) => void): Promise<ProtectionReport> {
    for (let i = 1; i <= 12; i++) {
      onProgress({ items: i * 100, bytes: i * 200 * MB, current: `/Applications/App ${i}.app/Contents/MacOS/App ${i}` });
      await wait(location.hash.includes("slowscan") ? 600 : 110);
    }
    protectionReport = buildProtection();
    return protectionReport;
  },
  cancel: async () => {},
  remove: async (kind: "detection" | "suspicious", id: string) => {
    await wait(400);
    if (!protectionReport) return;
    if (kind === "detection") protectionReport.detections = protectionReport.detections.filter((d) => d.target !== id);
    else protectionReport.suspicious = protectionReport.suspicious.filter((f) => f.item.id !== id);
  },
  cleanPrivacy: async (ids: string[]) => {
    await wait(500);
    const items = protectionReport?.privacy ?? [];
    const cleaned = items.filter((i) => ids.includes(i.id) && !i.running).map((i) => i.id);
    const failed = items.filter((i) => ids.includes(i.id) && i.running).map((i) => [i.id, `Quit ${i.owner} first.`] as [string, string]);
    const bytes = items.filter((i) => cleaned.includes(i.id)).reduce((s, i) => s + i.size, 0);
    if (protectionReport) protectionReport.privacy = items.filter((i) => !cleaned.includes(i.id));
    return { cleaned, failed, bytes };
  },
  revoke: async (id: string) => {
    await wait(400);
    if (protectionReport) protectionReport.permissions = protectionReport.permissions.filter((g) => g.id !== id);
    return protectionReport?.permissions ?? [];
  },
  openSettings: async () => {},
  setWatcher: async (enabled: boolean) => {
    await wait(300);
    watcherOn = enabled;
    return { enabled, alerts: watchAlerts };
  },
  clearAlerts: async () => {
    watchAlerts = [];
  },
  watcherStatus: async () => watcherOn,
};

let mockSettings: Settings = {
  staleProjectDays: 30,
  largeFileMb: 50,
  unusedAppDays: 180,
  checkUpdatesOnScan: true,
  onlineNotarization: true,
  offerTrashedAppCleanup: true,
  careMode: "remind" as const,
  careEveryDays: 7,
  ignore: location.hash.includes("ignored")
    ? [
        { kind: "path", value: `${HOME}/Sites/js/simpleworship`, label: "simpleworship", module: "settings", added: days(2) },
        { kind: "item", value: "updates:/Applications/Xcode.app@27.1", label: "Xcode 27.1", module: "applications", added: days(1) },
        { kind: "item", value: "cleanup:sim:F95A08D0", label: "iPhone 16 Pro Max (iOS 18.6)", module: "cleanup", added: days(5) },
        { kind: "item", value: "protection:suspicious:homebrew.mxcl.kafka", label: "kafka-server-start", module: "protection", added: days(9) },
      ]
    : [],
};

const settings = {
  get: async () => structuredClone(mockSettings),
  save: async (s: Settings) => {
    mockSettings = { ...s, ignore: mockSettings.ignore };
    return structuredClone(mockSettings);
  },
  ignore: async (kind: IgnoreKind, value: string, label: string, module: string) => {
    if (!mockSettings.ignore.some((r) => r.kind === kind && r.value === value)) {
      mockSettings.ignore.push({ kind, value, label, module, added: Date.now() / 1000 });
    }
    return structuredClone(mockSettings);
  },
  unignore: async (kind: IgnoreKind, value: string) => {
    mockSettings.ignore = mockSettings.ignore.filter((r) => !(r.kind === kind && r.value === value));
    return structuredClone(mockSettings);
  },
};

const perf = {
  scan: async () => {
    await wait(900);
    return buildPerf();
  },
  memory: async () => {
    const r = buildPerf();
    r.memory.used += (Math.random() - 0.5) * 0.4 * GB;
    return { memory: r.memory, processes: r.processes };
  },
  runTasks: async (ids: string[], onStart: (id: string) => void): Promise<TaskResult[]> => {
    const out: TaskResult[] = [];
    for (const id of ids) {
      onStart(id);
      await wait(500);
      out.push({ id, ok: id !== "speed-up-mail", message: id === "free-ram" ? "Freed 1.4 GB." : id === "speed-up-mail" ? "Quit Mail first." : "Done." });
    }
    return out;
  },
  fullItems: async () => {
    await wait(1200);
    if (!perfItems.some((i) => i.id === "com.movavi.helper")) {
      perfItems = [
        ...perfItems.map((i) => (i.kind === "agent" && !i.developer ? { ...i, developer: i.name.includes("kafka") || i.name.includes("rabbit") ? null : "Homebrew" } : i)),
        item("Movavi Screen Recorder Helper", "loginItem", { id: "com.movavi.helper", enabled: false, controllable: false, plist: null, developer: "Movavi Software", appName: "Movavi Screen Recorder" }),
      ];
    }
    return perfItems;
  },
  setItem: async (id: string, enabled: boolean) => {
    await wait(300);
    perfItems = perfItems.map((i) => (i.id === id ? { ...i, enabled, running: enabled && i.running } : i));
    return perfItems;
  },
  removeItem: async (id: string) => {
    await wait(300);
    perfItems = perfItems.filter((i) => i.id !== id);
    return perfItems;
  },
  deleteSnapshots: async (ids: string[]) => {
    await wait(600);
    snapshots = ids.length ? snapshots.filter((s) => !ids.includes(s.id)) : [];
    return { snapshots, purgeable: snapshots.length ? 59 * GB * (snapshots.length / 18) : 0.4 * GB };
  },
  openLoginItemsSettings: async () => {},
};

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
      // Two copies, one bundle id: the case that once broke keyed lists.
      app("IDLE", "org.python.IDLE", 2.1, 400, [], { id: "/Applications/Python 3.11/IDLE.app", path: "/Applications/Python 3.11/IDLE.app", sharesSupportFiles: true, version: "3.11" }),
      app("IDLE", "org.python.IDLE", 2.2, 30, [], { id: "/Applications/Python 3.13/IDLE.app", path: "/Applications/Python 3.13/IDLE.app", sharesSupportFiles: true, version: "3.13" }),
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
  perf,
  async vitals() {
    const j = () => Math.random();
    return {
      model: "MacBook Pro",
      diskName: "WANPEN",
      diskFree: 312.6 * GB,
      diskTotal: 1000.2 * GB,
      memoryPressure: 36 + Math.round(j() * 4),
      memoryLevel: "normal" as const,
      swapUsed: 7 * GB,
      cpuLoad: 8 + Math.round(j() * 30),
      battery: { percent: 100, state: "charged", remaining: null, cycles: 878, health: 72 },
      netUp: Math.round(j() * 40_000),
      netDown: Math.round(j() * 2_000_000),
      trash: location.hash.includes("fulltrash") ? 2.4 * GB : 0,
      uptimeSecs: 589300,
      health: "good" as const,
    };
  },
  settings,
  protection,
  activity: {
    list: async () => activity,
    clear: async () => {
      activity = [];
    },
  },
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
  async checkUpdates(onStep: (s: string) => void): Promise<UpdatesReport> {
    for (const s of ["Asking the App Store…", "Checking Maccy…", "Asking Homebrew…", "Checking Homebrew's catalog…"]) {
      onStep(s);
      await wait(location.hash.includes("slowupdates") ? 4000 : 250);
    }
    return { updates, checked: 49, unchecked: ["Adobe Photoshop 2026", "Microsoft Word", "zoom.us"], errors: [["Twingate", "curl: (56) 400"]], elapsedMs: 2900 };
  },
  async installUpdates(ids: string[], onEvent: (e: UpdateEvent) => void): Promise<string[]> {
    const done: string[] = [];
    for (const id of ids) {
      const u = updates.find((x) => x.id === id);
      if (!u) continue;
      for (const step of u.oneClick ? ["Downloading…", "Checking the developer's signature…", "Verifying…", "Installing…"] : ["Opening…"]) {
        onEvent({ status: "started", id, step });
        await wait(location.hash.includes("slowinstall") ? 1500 : 200);
      }
      if (u.running) {
        onEvent({ status: "failed", id, error: `${u.name} is running. Quit it, then update.` });
      } else if (u.source === "appStore" || u.source === "catalog") {
        onEvent({ status: "done", id, message: u.source === "appStore" ? "Opened in the App Store. Click Update there." : `Opened ${u.name}. Use its Check for Updates.` });
      } else {
        done.push(id);
        onEvent({ status: "done", id, message: `Updated to ${u.latest}. The old version is in the Trash.` });
      }
    }
    updates = updates.filter((u) => !done.includes(u.id));
    return done;
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
      const size = a
        ? a.size + a.relatedSize
        : (appsReport?.leftovers.find((l) => l.id === t.id)?.size ?? appsReport?.installers.find((i) => i.path === t.id)?.size ?? 0);
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
