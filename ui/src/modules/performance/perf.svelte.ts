// Performance state, at module scope so it survives switching modules.
import type { Component } from "svelte";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { Clock, Cpu, Power, Rocket, Wrench } from "@lucide/svelte";
import { api, type LaunchItem, type PerfReport, type TaskResult } from "../../lib/api";
import { bytes, plural } from "../../lib/format";
import { app } from "../../lib/state.svelte";

export type Section = "tasks" | "memory" | "login" | "background" | "snapshots";

export const SECTIONS: { id: Section; title: string; blurb: string; icon: Component }[] = [
  { id: "tasks", title: "Maintenance Tasks", blurb: "Fixes that used to need Terminal. Pick some and run them together.", icon: Wrench },
  { id: "memory", title: "Memory", blurb: "What's using your RAM right now, refreshed live.", icon: Cpu },
  { id: "login", title: "Login Items", blurb: "Apps and helpers that start when you log in.", icon: Power },
  { id: "background", title: "Background Items", blurb: "Agents and services that run behind the scenes. Switching one off is reversible.", icon: Rocket },
  { id: "snapshots", title: "Time Machine Snapshots", blurb: "Hourly local snapshots macOS keeps on this disk. Your backup drive is never touched.", icon: Clock },
];

export type TaskState = "idle" | "running" | "done" | "failed";

class Perf {
  phase = $state<"home" | "scanning" | "results">("home");
  view = $state<"summary" | "review">("summary");
  section = $state<Section>("tasks");
  report = $state<PerfReport | null>(null);

  chosen = new SvelteSet<string>();
  taskState = new SvelteMap<string, TaskState>();
  taskResult = new SvelteMap<string, TaskResult>();
  running = $state(false);

  /** Item id currently being switched or removed. */
  busyItem = $state<string | null>(null);
  pickedSnapshots = new SvelteSet<string>();
  deletingSnapshots = $state(false);

  loginItems = $derived(this.report?.items.filter((i) => i.kind === "loginItem") ?? []);
  backgroundItems = $derived(this.report?.items.filter((i) => i.kind !== "loginItem") ?? []);
  recommended = $derived(this.report?.tasks.filter((t) => t.recommended) ?? []);

  private setPhase(p: "home" | "scanning" | "results") {
    this.phase = p;
    app.phases.performance = p;
  }

  async scan() {
    this.setPhase("scanning");
    try {
      this.report = await api.perf.scan();
      this.chosen.clear();
      this.fullListAt = null;
      for (const t of this.recommended) this.chosen.add(t.id);
      this.taskState.clear();
      this.taskResult.clear();
      this.view = "summary";
      this.setPhase("results");
    } catch (e) {
      app.toast(`Scan failed: ${e}`, "error");
      this.setPhase("home");
    }
  }

  review(section: Section) {
    this.section = section;
    this.view = "review";
  }

  async refreshMemory() {
    if (!this.report) return;
    try {
      const m = await api.perf.memory();
      if (this.report) this.report = { ...this.report, memory: m.memory, processes: m.processes };
    } catch {
      // Live numbers are a nicety; keep the last ones.
    }
  }

  async runTasks(ids: string[] = [...this.chosen]) {
    if (!ids.length || this.running) return;
    this.running = true;
    for (const id of ids) {
      this.taskState.set(id, "idle");
      this.taskResult.delete(id);
    }
    try {
      const results = await api.perf.runTasks(ids, (id) => this.taskState.set(id, "running"));
      for (const r of results) {
        this.taskState.set(r.id, r.ok ? "done" : "failed");
        this.taskResult.set(r.id, r);
      }
      const ok = results.filter((r) => r.ok).length;
      if (ok) app.toast(`${plural(ok, "task")} finished.`, "success");
      const failed = results.find((r) => !r.ok);
      if (failed) app.toast(`${this.report?.tasks.find((t) => t.id === failed.id)?.title}: ${failed.message}`, "error");
      if (ids.includes("thin-snapshots")) await this.reloadSnapshots();
      await this.refreshMemory();
      app.refreshVolumes();
    } catch (e) {
      app.toast(`Tasks stopped: ${e}`, "error");
    } finally {
      this.running = false;
    }
  }

  /** When the full System Settings list was last loaded (it asks for a password). */
  fullListAt = $state<number | null>(null);
  loadingFull = $state(false);

  async loadFullList() {
    this.loadingFull = true;
    try {
      this.setItems(await api.perf.fullItems());
      this.fullListAt = Date.now();
      app.toast("Showing the full list from System Settings.", "success");
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.loadingFull = false;
    }
  }

  private setItems(items: LaunchItem[]) {
    if (this.report) this.report = { ...this.report, items };
  }

  async setItem(item: LaunchItem, enabled: boolean) {
    this.busyItem = item.id;
    try {
      this.setItems(await api.perf.setItem(item.id, enabled));
      app.toast(`${item.name} ${enabled ? "will run again" : "is switched off"}.`, "success");
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.busyItem = null;
    }
  }

  async removeItem(item: LaunchItem) {
    this.busyItem = item.id;
    try {
      this.setItems(await api.perf.removeItem(item.id));
      app.toast(`${item.name} removed. Its file is in the Trash.`, "success");
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.busyItem = null;
    }
  }

  private async reloadSnapshots(next?: { snapshots: PerfReport["snapshots"]; purgeable: number | null }) {
    if (!this.report) return;
    const fresh = next ?? (await api.perf.scan());
    this.report = { ...this.report, snapshots: fresh.snapshots, purgeable: fresh.purgeable };
  }

  /** Removes the picked snapshots, or all of them when none are picked. */
  async deleteSnapshots() {
    this.deletingSnapshots = true;
    const before = this.report?.purgeable ?? 0;
    try {
      const next = await api.perf.deleteSnapshots([...this.pickedSnapshots]);
      await this.reloadSnapshots(next);
      this.pickedSnapshots.clear();
      const freed = before - (next.purgeable ?? 0);
      app.toast(freed > 0 ? `Snapshots removed. About ${bytes(freed)} is free again.` : "Snapshots removed.", "success");
      app.refreshVolumes();
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.deletingSnapshots = false;
    }
  }

  startOver() {
    this.report = null;
    this.setPhase("home");
  }
}

export const perf = new Perf();
