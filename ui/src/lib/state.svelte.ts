import { api, type SystemInfo, type Volume } from "./api";
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

  async refresh() {
    const [system, volumes] = await Promise.all([api.systemInfo(), api.volumes()]);
    this.system = system;
    this.volumes = volumes;
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
