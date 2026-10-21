// Fake backend for running the UI in a plain browser (`pnpm dev`, then open
// http://localhost:1420). Lets the UI be designed and screenshotted without Tauri.
import type {
  Category,
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
