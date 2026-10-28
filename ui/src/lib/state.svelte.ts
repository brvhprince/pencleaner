import { api, type IgnoreKind, type Settings, type SystemInfo, type Volume } from "./api";
import type { ModuleId } from "./modules";

export interface Toast {
  id: number;
  message: string;
  tone: "info" | "success" | "error";
}

class AppState {
  module = $state<ModuleId>("smartcare");
  /** Each module reports where it is; anything but "home" collapses the sidebar. */
  phases = $state<Partial<Record<ModuleId, "home" | "scanning" | "results">>>({});
  compact = $derived((this.phases[this.module] ?? "home") !== "home");
  system = $state<SystemInfo | null>(null);
  volumes = $state<Volume[]>([]);
  toasts = $state<Toast[]>([]);
  onboardingDismissed = $state(false);
  settings = $state<Settings>({
    staleProjectDays: 30,
    largeFileMb: 50,
    unusedAppDays: 180,
    checkUpdatesOnScan: true,
    onlineNotarization: true,
    offerTrashedAppCleanup: true,
    ignore: [],
  });

  async refresh() {
    const [system, volumes, settings] = await Promise.all([api.systemInfo(), api.volumes(), api.settings.get()]);
    this.system = system;
    this.volumes = volumes;
    this.settings = settings;
  }

  async saveSettings(patch: Partial<Settings>) {
    try {
      this.settings = await api.settings.save({ ...this.settings, ...patch });
    } catch (e) {
      this.toast(`Couldn't save settings: ${e}`, "error");
    }
  }

  /** Hides something from future scans (and the current results, which callers prune). */
  async ignore(kind: IgnoreKind, value: string, label: string, module: string) {
    try {
      this.settings = await api.settings.ignore(kind, value, label, module);
      this.toast(`${label} won't be shown again. Undo it in Settings → Ignore List.`, "success");
      return true;
    } catch (e) {
      this.toast(`Couldn't ignore it: ${e}`, "error");
      return false;
    }
  }

  async unignore(kind: IgnoreKind, value: string) {
    this.settings = await api.settings.unignore(kind, value);
  }

  async refreshVolumes() {
    this.volumes = await api.volumes();
  }

  go(id: ModuleId) {
    this.module = id;
  }

  toast(message: string, tone: Toast["tone"] = "info") {
    const id = Date.now() + Math.random();
    this.toasts.push({ id, message, tone });
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id);
    }, 4200);
  }
}

export const app = new AppState();
