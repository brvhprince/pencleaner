// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.
import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { mock } from "./mock";

/** False when the UI is opened in a plain browser; `mock` then stands in for Rust. */
export const inTauri = "__TAURI_INTERNALS__" in window;

export interface SystemInfo {
  fullDiskAccess: boolean;
  home: string;
  user: string;
  devBuild: boolean;
}

export interface Volume {
  name: string;
  mount: string;
  total: number;
  free: number;
  used: number;
  isRoot: boolean;
  fsType: string;
}

export interface Progress {
  items: number;
  bytes: number;
  current: string;
}

export interface ScanSummary {
  root: string;
  size: number;
  items: number;
  denied: number;
  cancelled: boolean;
  elapsedMs: number;
}

export type EntryKind = "folder" | "app" | "bundle" | "file" | "symlink";

export interface Entry {
  name: string;
  path: string;
  size: number;
  items: number;
  kind: EntryKind;
  protected: string | null;
}

export interface Listing {
  path: string;
  name: string;
  size: number;
  items: number;
  denied: boolean;
  entries: Entry[];
  restCount: number;
  restSize: number;
}

export type RemoveMode = "trash" | "delete";

export type RemoveEvent =
  | { status: "started"; path: string; index: number; total: number }
  | { status: "removed"; path: string; bytes: number }
  | { status: "failed"; path: string; error: string };

export interface RemoveSummary {
  removed: [string, number][];
  failed: [string, string][];
  bytesFreed: number;
}

export type Category = "system" | "developer" | "ai" | "mail" | "trash";
export type ItemKind = "folder" | "file" | "app" | "project" | "model" | "device" | "trash";

export interface CleanupItem {
  id: string;
  name: string;
  detail: string | null;
  path: string | null;
  size: number;
  kind: ItemKind;
  mode: RemoveMode;
  lastActivity: number | null;
  recommended: boolean;
  warning: string | null;
}

export interface CleanupGroup {
  id: string;
  category: Category;
  title: string;
  description: string;
  items: CleanupItem[];
  size: number;
  note: string | null;
}

export interface CleanupReport {
  groups: CleanupGroup[];
  elapsedMs: number;
  fullDiskAccess: boolean;
  cancelled: boolean;
}

export type CleanEvent =
  | { status: "started"; id: string; name: string; size: number; index: number; total: number }
  | { status: "cleaned"; id: string; bytes: number }
  | { status: "failed"; id: string; error: string; bytes: number };

export interface CleanSummary {
  cleaned: string[];
  failed: [string, string][];
  bytesFreed: number;
  byCategory: Partial<Record<Category, number>>;
}

export type FileKind = "image" | "video" | "audio" | "archive" | "installer" | "document" | "folder" | "other";

export interface FileInfo {
  path: string;
  name: string;
  size: number;
  kind: FileKind;
  modified: number | null;
  created: number | null;
  lastUsed: number | null;
  added: number | null;
  source: string | null;
  width: number | null;
  height: number | null;
}

export interface FileSet {
  id: string;
  files: FileInfo[];
  keep: string;
  reclaimable: number;
}

export interface ClutterReport {
  root: string;
  filesScanned: number;
  large: FileInfo[];
  duplicates: FileSet[];
  similar: FileSet[];
  downloads: FileInfo[];
  elapsedMs: number;
  cancelled: boolean;
}

export interface RelatedFile {
  path: string;
  label: string;
  size: number;
  admin: boolean;
}

export interface AppEntry {
  /** The bundle path: unique even when two copies share a bundle id. */
  id: string;
  bundleId: string;
  name: string;
  version: string | null;
  path: string;
  size: number;
  related: RelatedFile[];
  relatedSize: number;
  lastUsed: number | null;
  appStore: boolean;
  running: boolean;
  sharesSupportFiles: boolean;
}

export interface Leftover {
  id: string;
  vendorInstalled: boolean;
  files: RelatedFile[];
  size: number;
  lastActivity: number | null;
}

export interface Installer {
  path: string;
  name: string;
  size: number;
  added: number | null;
  installedApp: string | null;
  recommended: boolean;
}

export interface AppsReport {
  apps: AppEntry[];
  leftovers: Leftover[];
  installers: Installer[];
  elapsedMs: number;
  cancelled: boolean;
}

export type AppTarget = { kind: "app" | "leftover" | "installer"; id: string };

export interface MemoryStats {
  total: number;
  used: number;
  app: number;
  wired: number;
  compressed: number;
  cached: number;
  free: number;
  swapUsed: number;
  swapTotal: number;
  pressure: "normal" | "warning" | "critical";
}

export interface ProcessMemory {
  name: string;
  appPath: string | null;
  bytes: number;
  processes: number;
}

export type LaunchKind = "loginItem" | "agent" | "daemon";

export interface LaunchItem {
  id: string;
  name: string;
  developer: string | null;
  kind: LaunchKind;
  controllable: boolean;
  enabled: boolean;
  running: boolean;
  plist: string | null;
  executable: string | null;
  appPath: string | null;
  appName: string | null;
  broken: boolean;
}

export interface Snapshot {
  id: string;
  created: number | null;
}

export interface PerfTask {
  id: string;
  title: string;
  description: string;
  admin: boolean;
  slow: boolean;
  recommended: boolean;
  reason: string | null;
}

export interface PerfReport {
  memory: MemoryStats;
  processes: ProcessMemory[];
  items: LaunchItem[];
  snapshots: Snapshot[];
  purgeable: number | null;
  tasks: PerfTask[];
}

export interface TaskResult {
  id: string;
  ok: boolean;
  message: string;
}

/** URL for showing a local image in the webview. */
export function fileSrc(path: string) {
  return inTauri ? convertFileSrc(path) : mock.thumb(path);
}

const tauriApi = {
  systemInfo: () => invoke<SystemInfo>("system_info"),
  volumes: () => invoke<Volume[]>("volumes_list"),
  openFullDiskAccessSettings: () => invoke<void>("open_full_disk_access_settings"),
  revealInFinder: (path: string) => invoke<void>("reveal_in_finder", { path }),

  spaceLens: {
    scan(root: string, onProgress: (p: Progress) => void) {
      const ch = new Channel<Progress>();
      ch.onmessage = onProgress;
      return invoke<ScanSummary>("space_lens_scan", { root, onProgress: ch });
    },
    cancel: () => invoke<void>("space_lens_cancel"),
    list: (path: string) => invoke<Listing>("space_lens_list", { path }),
    discard: () => invoke<void>("space_lens_discard"),
  },

  cleanup: {
    scan(categories: Category[] | null, onProgress: (p: Progress) => void) {
      const ch = new Channel<Progress>();
      ch.onmessage = onProgress;
      return invoke<CleanupReport>("cleanup_scan", { categories, onProgress: ch });
    },
    cancel: () => invoke<void>("cleanup_cancel"),
    clean(ids: string[], onEvent: (e: CleanEvent) => void) {
      const ch = new Channel<CleanEvent>();
      ch.onmessage = onEvent;
      return invoke<CleanSummary>("cleanup_clean", { ids, onEvent: ch });
    },
  },

  clutter: {
    scan(root: string, onProgress: (p: Progress) => void) {
      const ch = new Channel<Progress>();
      ch.onmessage = onProgress;
      return invoke<ClutterReport>("clutter_scan", { root, onProgress: ch });
    },
    cancel: () => invoke<void>("clutter_cancel"),
    remove(ids: string[], onEvent: (e: RemoveEvent) => void) {
      const ch = new Channel<RemoveEvent>();
      ch.onmessage = onEvent;
      return invoke<RemoveSummary>("clutter_remove", { ids, onEvent: ch });
    },
  },

  apps: {
    scan(onProgress: (p: Progress) => void) {
      const ch = new Channel<Progress>();
      ch.onmessage = onProgress;
      return invoke<AppsReport>("apps_scan", { onProgress: ch });
    },
    remove(targets: AppTarget[], onEvent: (e: CleanEvent) => void) {
      const ch = new Channel<CleanEvent>();
      ch.onmessage = onEvent;
      return invoke<CleanSummary>("apps_remove", { targets, onEvent: ch });
    },
    icon: (path: string) => invoke<string | null>("app_icon", { path }),
  },

  perf: {
    scan: () => invoke<PerfReport>("perf_scan"),
    memory: () => invoke<{ memory: MemoryStats; processes: ProcessMemory[] }>("perf_memory"),
    runTasks(ids: string[], onStart: (id: string) => void) {
      const ch = new Channel<string>();
      ch.onmessage = onStart;
      return invoke<TaskResult[]>("perf_run_tasks", { ids, onStart: ch });
    },
    setItem: (id: string, enabled: boolean) => invoke<LaunchItem[]>("perf_set_item", { id, enabled }),
    removeItem: (id: string) => invoke<LaunchItem[]>("perf_remove_item", { id }),
    deleteSnapshots: (ids: string[]) => invoke<{ snapshots: Snapshot[]; purgeable: number | null }>("perf_delete_snapshots", { ids }),
    openLoginItemsSettings: () => invoke<void>("open_login_items_settings"),
  },

  remove(items: { path: string; size?: number }[], mode: RemoveMode, onEvent: (e: RemoveEvent) => void) {
    const ch = new Channel<RemoveEvent>();
    ch.onmessage = onEvent;
    return invoke<RemoveSummary>("remove_items", { items, mode, onEvent: ch });
  },
};

const mockApi: typeof tauriApi = {
  systemInfo: mock.systemInfo,
  volumes: mock.volumes,
  openFullDiskAccessSettings: async () => {},
  revealInFinder: async () => {},
  spaceLens: {
    scan: mock.scan,
    cancel: async () => {},
    list: mock.list,
    discard: async () => {},
  },
  cleanup: {
    scan: mock.cleanupScan,
    cancel: async () => {},
    clean: mock.cleanupClean,
  },
  clutter: {
    scan: mock.clutterScan,
    cancel: async () => {},
    remove: (ids, onEvent) => mock.remove(ids.map((path) => ({ path })), onEvent),
  },
  apps: {
    scan: mock.appsScan,
    remove: mock.appsRemove,
    icon: async () => null,
  },
  perf: mock.perf,
  remove: (items, _mode, onEvent) => mock.remove(items, onEvent),
};

export const api = inTauri ? tauriApi : mockApi;
