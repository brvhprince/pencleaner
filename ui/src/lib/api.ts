// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.
import { Channel, invoke } from "@tauri-apps/api/core";
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
  remove: (items, _mode, onEvent) => mock.remove(items, onEvent),
};

export const api = inTauri ? tauriApi : mockApi;
