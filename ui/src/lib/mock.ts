// Fake backend for running the UI in a plain browser (`pnpm dev`, then open
// http://localhost:1420). Lets the UI be designed and screenshotted without Tauri.
import type { Entry, EntryKind, Listing, Progress, RemoveEvent, RemoveSummary, ScanSummary, SystemInfo, Volume } from "./api";

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
