// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.
import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
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
  isSelf: boolean;
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

export interface ActivityEvent {
  at: number;
  module: "cleanup" | "clutter" | "applications" | "spacelens" | "performance" | "protection";
  action: string;
  bytes: number;
  items: number;
  detail: string | null;
}

export type UpdateSource = "appStore" | "sparkle" | "homebrew" | "catalog";

export interface AppUpdate {
  id: string;
  name: string;
  appPath: string | null;
  source: UpdateSource;
  installed: string;
  latest: string;
  size: number | null;
  notes: string | null;
  notesUrl: string | null;
  oneClick: boolean;
  actionNote: string | null;
  signed: boolean;
  requires: string | null;
  running: boolean;
}

export interface UpdatesReport {
  updates: AppUpdate[];
  checked: number;
  unchecked: string[];
  errors: [string, string][];
  elapsedMs: number;
}

export type UpdateEvent =
  | { status: "started"; id: string; step: string }
  | { status: "done"; id: string; message: string }
  | { status: "failed"; id: string; error: string };

export interface Detection {
  file: string;
  target: string;
  rule: string;
  description: string | null;
}

export type Signer =
  | { kind: "apple" }
  | { kind: "developer"; name: string; team: string; notarized: boolean }
  | { kind: "homebrew" }
  | { kind: "adHoc" }
  | { kind: "unsigned" }
  | { kind: "script" };

export interface Finding {
  item: LaunchItem;
  program: string;
  signer: Signer;
  severity: "high" | "medium";
  reasons: string[];
}

export interface PrivacyItem {
  id: string;
  owner: string;
  appPath: string | null;
  trace: "history" | "cookies" | "autofill" | "recentItems";
  title: string;
  detail: string;
  size: number;
  recommended: boolean;
  warning: string | null;
  running: boolean;
}

export interface Grant {
  id: string;
  service: string;
  label: string;
  settingsAnchor: string;
  client: string;
  appName: string;
  appPath: string | null;
  target: string | null;
  granted: number | null;
  orphaned: boolean;
  revocable: boolean;
  lockedToSigner: boolean;
  isSelf: boolean;
  system: boolean;
}

export interface WatchAlert {
  at: number;
  id: string;
  name: string;
  plist: string;
  program: string | null;
  severity: "high" | "medium" | null;
  reasons: string[];
  malware: string | null;
}

export interface ProtectionReport {
  rules: { version: string | null; rules: number } | null;
  filesScanned: number;
  detections: Detection[];
  suspicious: Finding[];
  privacy: PrivacyItem[];
  permissions: Grant[];
  permissionsError: string | null;
  watcher: boolean;
  alerts: WatchAlert[];
  errors: string[];
  elapsedMs: number;
  cancelled: boolean;
}

/** URL for showing a local image in the webview. */
export function fileSrc(path: string) {
  return inTauri ? convertFileSrc(path) : mock.thumb(path);
}

export type IgnoreKind = "path" | "item";

export interface IgnoreRule {
  kind: IgnoreKind;
  value: string;
  label: string;
  module: string;
  added: number;
}

export interface Settings {
  staleProjectDays: number;
  largeFileMb: number;
  unusedAppDays: number;
  checkUpdatesOnScan: boolean;
  onlineNotarization: boolean;
  offerTrashedAppCleanup: boolean;
  careMode: CareMode;
  careEveryDays: number;
  ignore: IgnoreRule[];
}

export interface Vitals {
  model: string;
  diskName: string;
  diskFree: number;
  diskTotal: number;
  memoryPressure: number;
  memoryLevel: "normal" | "warning" | "critical";
  swapUsed: number;
  cpuLoad: number;
  battery: { percent: number; state: string; remaining: string | null; cycles: number | null; health: number | null } | null;
  netUp: number;
  netDown: number;
  trash: number;
  uptimeSecs: number;
  sensors: { cpuCelsius: number | null; fanRpm: number | null; fanMaxRpm: number | null } | null;
  health: "good" | "fair" | "attention";
}

export interface TrashedApp {
  name: string;
  bundleId: string;
  path: string;
  files: RelatedFile[];
  size: number;
}

export type CareMode = "off" | "remind" | "clean";

export interface CareStatus {
  mode: CareMode;
  everyDays: number;
  lastRun: number | null;
  nextDue: number | null;
  due: boolean;
}

export interface LoginStatus {
  enabled: boolean;
  needsApproval: boolean;
  available: boolean;
}

export interface AboutInfo {
  version: string;
  commit: string;
  builtAt: number;
  macos: string;
  arch: string;
  fullDiskAccess: boolean;
  xprotect: string | null;
}

const tauriApi = {
  systemInfo: () => invoke<SystemInfo>("system_info"),
  about: () => invoke<AboutInfo>("about_info"),
  vitals: () => invoke<Vitals>("vitals"),
  showMain: (module?: string) => invoke<void>("show_main", { module }),
  emptyTrash: () => invoke<number>("empty_trash"),
  care: {
    status: () => invoke<CareStatus>("care_status"),
    done: () => invoke<void>("care_done"),
  },
  loginItem: {
    status: () => invoke<LoginStatus>("login_item_status"),
    set: (on: boolean) => invoke<LoginStatus>("login_item_set", { on }),
    openSettings: () => invoke<void>("open_login_items_for_app"),
  },
  trashedApp: {
    remove: () => invoke<CleanSummary>("trashed_app_remove"),
    dismiss: () => invoke<void>("trashed_app_dismiss"),
  },
  settings: {
    get: () => invoke<Settings>("settings_get"),
    save: (settings: Settings) => invoke<Settings>("settings_save", { settings }),
    ignore: (kind: IgnoreKind, value: string, label: string, module: string) => invoke<Settings>("ignore_add", { kind, value, label, module }),
    unignore: (kind: IgnoreKind, value: string) => invoke<Settings>("ignore_remove", { kind, value }),
  },
  volumes: () => invoke<Volume[]>("volumes_list"),
  openFullDiskAccessSettings: () => invoke<void>("open_full_disk_access_settings"),
  revealInFinder: (path: string) => invoke<void>("reveal_in_finder", { path }),
  /** http(s) links only, in the default browser. */
  openUrl: (url: string) => (/^https?:\/\//.test(url) ? openUrl(url) : Promise.resolve()),

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
    checkUpdates(onStep: (step: string) => void) {
      const ch = new Channel<string>();
      ch.onmessage = onStep;
      return invoke<UpdatesReport>("updates_scan", { onStep: ch });
    },
    installUpdates(ids: string[], onEvent: (e: UpdateEvent) => void) {
      const ch = new Channel<UpdateEvent>();
      ch.onmessage = onEvent;
      return invoke<string[]>("updates_install", { ids, onEvent: ch });
    },
    openAppStoreUpdates: () => invoke<void>("open_app_store_updates"),
  },

  perf: {
    scan: () => invoke<PerfReport>("perf_scan"),
    memory: () => invoke<{ memory: MemoryStats; processes: ProcessMemory[] }>("perf_memory"),
    runTasks(ids: string[], onStart: (id: string) => void) {
      const ch = new Channel<string>();
      ch.onmessage = onStart;
      return invoke<TaskResult[]>("perf_run_tasks", { ids, onStart: ch });
    },
    fullItems: () => invoke<LaunchItem[]>("perf_full_items"),
    setItem: (id: string, enabled: boolean) => invoke<LaunchItem[]>("perf_set_item", { id, enabled }),
    removeItem: (id: string) => invoke<LaunchItem[]>("perf_remove_item", { id }),
    deleteSnapshots: (ids: string[]) => invoke<{ snapshots: Snapshot[]; purgeable: number | null }>("perf_delete_snapshots", { ids }),
    openLoginItemsSettings: () => invoke<void>("open_login_items_settings"),
  },

  activity: {
    list: () => invoke<ActivityEvent[]>("activity_list"),
    clear: () => invoke<void>("activity_clear"),
  },

  protection: {
    scan(onProgress: (p: Progress) => void) {
      const ch = new Channel<Progress>();
      ch.onmessage = onProgress;
      return invoke<ProtectionReport>("protection_scan", { onProgress: ch });
    },
    cancel: () => invoke<void>("protection_cancel"),
    remove: (kind: "detection" | "suspicious", id: string) => invoke<void>("protection_remove", { kind, id }),
    cleanPrivacy: (ids: string[]) => invoke<{ cleaned: string[]; failed: [string, string][]; bytes: number }>("privacy_clean", { ids }),
    revoke: (id: string) => invoke<Grant[]>("permission_revoke", { id }),
    openSettings: (anchor: string) => invoke<void>("open_privacy_settings", { anchor }),
    setWatcher: (enabled: boolean) => invoke<{ enabled: boolean; alerts: WatchAlert[] }>("watcher_set", { enabled }),
    clearAlerts: () => invoke<void>("watcher_clear_alerts"),
    watcherStatus: () => invoke<boolean>("watcher_status"),
  },

  remove(items: { path: string; size?: number }[], mode: RemoveMode, onEvent: (e: RemoveEvent) => void) {
    const ch = new Channel<RemoveEvent>();
    ch.onmessage = onEvent;
    return invoke<RemoveSummary>("remove_items", { items, mode, onEvent: ch });
  },
};

const mockApi: typeof tauriApi = {
  systemInfo: mock.systemInfo,
  vitals: mock.vitals,
  showMain: async () => {},
  emptyTrash: async () => 2.4e9,
  care: {
    status: async (): Promise<CareStatus> => ({ mode: "remind", everyDays: 7, lastRun: Date.now() / 1000 - 9 * 86400, nextDue: Date.now() / 1000 - 2 * 86400, due: location.hash.includes("caredue") }),
    done: async () => {},
  },
  loginItem: {
    status: async () => ({ enabled: true, needsApproval: false, available: true }),
    set: async (on: boolean) => ({ enabled: on, needsApproval: false, available: true }),
    openSettings: async () => {},
  },
  trashedApp: {
    remove: async () => ({ cleaned: ["x"], failed: [], bytesFreed: 214e6, byCategory: {} }) as CleanSummary,
    dismiss: async () => {},
  },
  settings: mock.settings,
  about: async () => ({ version: "0.1.0", commit: "b39b86b+", builtAt: Date.now() / 1000 - 3600, macos: "26.6.2", arch: "Intel", fullDiskAccess: true, xprotect: "5368" }),
  volumes: mock.volumes,
  openFullDiskAccessSettings: async () => {},
  revealInFinder: async () => {},
  openUrl: async (url: string) => {
    window.open(url, "_blank");
  },
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
    checkUpdates: mock.checkUpdates,
    installUpdates: mock.installUpdates,
    openAppStoreUpdates: async () => {},
  },
  perf: mock.perf,
  protection: mock.protection,
  activity: mock.activity,
  remove: (items, _mode, onEvent) => mock.remove(items, onEvent),
};

export const api = inTauri ? tauriApi : mockApi;
