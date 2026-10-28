// Cleanup state, kept at module scope so a scan or a review survives switching modules.
import type { Component } from "svelte";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { Bot, Hammer, Mail, Sparkles, Trash } from "@lucide/svelte";
import { api, type Category, type CleanSummary, type CleanupGroup, type CleanupItem, type CleanupReport, type Progress } from "../../lib/api";
import { bytes } from "../../lib/format";
import { pick } from "../../lib/modules";
import { app } from "../../lib/state.svelte";

export interface CategoryMeta {
  id: Category;
  title: string;
  blurb: string;
  icon: Component;
}

export const CATEGORIES: CategoryMeta[] = [
  { id: "system", title: "System Junk", blurb: "Caches, logs and temporary files", icon: Sparkles },
  { id: "developer", title: "Developer Junk", blurb: pick("Build output, package caches, simulators", "Build output and package caches"), icon: Hammer },
  { id: "ai", title: "AI Clutter", blurb: "Local models and AI tool caches", icon: Bot },
  ...pick<CategoryMeta[]>([{ id: "mail", title: "Mail Attachments", blurb: "Attachments Mail stored locally", icon: Mail }], []),
  { id: "trash", title: pick("Trash Bins", "Recycle Bin"), blurb: pick("Everything waiting in the Trash", "Everything waiting in the Recycle Bin"), icon: Trash },
];

type Phase = "home" | "scanning" | "results" | "cleaning" | "done";
export type CleanStatus = "pending" | "working" | "done" | "failed";

class Cleanup {
  phase = $state<Phase>("home");
  /** Summary cards, or the item-by-item manager. */
  view = $state<"summary" | "review">("summary");
  focusGroup = $state<string | null>(null);
  progress = $state<Progress | null>(null);
  report = $state<CleanupReport | null>(null);
  selected = new SvelteSet<string>();
  /** Dev projects untouched for this many days are pre-selected. */
  staleDays = $state(30);

  // Cleaning run
  status = new SvelteMap<string, CleanStatus>();
  log = $state<{ id: string; name: string; size: number }[]>([]);
  done = $state(0);
  total = $state(0);
  result = $state<CleanSummary | null>(null);
  /** Bytes that went to the Trash rather than being deleted. */
  trashed = $state(0);

  groups = $derived(this.report?.groups ?? []);
  items = $derived(this.groups.flatMap((g) => g.items));
  /** Selected items that carry a warning; cleaning asks about these first. */
  risky = $derived(this.items.filter((i) => i.warning && this.selected.has(i.id)));
  confirming = $state(false);
  selectedSize = $derived(this.items.filter((i) => this.selected.has(i.id)).reduce((s, i) => s + i.size, 0));
  foundSize = $derived(this.items.reduce((s, i) => s + i.size, 0));

  private setPhase(p: Phase) {
    this.phase = p;
    app.phases.cleanup = p === "home" ? "home" : p === "scanning" || p === "cleaning" ? "scanning" : "results";
  }

  groupsOf(category: Category) {
    return this.groups.filter((g) => g.category === category);
  }

  totals(category: Category) {
    let found = 0;
    let picked = 0;
    for (const g of this.groupsOf(category))
      for (const i of g.items) {
        found += i.size;
        if (this.selected.has(i.id)) picked += i.size;
      }
    return { found, picked };
  }

  groupSelected(g: CleanupGroup) {
    return g.items.reduce((s, i) => s + (this.selected.has(i.id) ? i.size : 0), 0);
  }

  groupState(g: CleanupGroup): "all" | "some" | "none" {
    const n = g.items.filter((i) => this.selected.has(i.id)).length;
    return n === 0 ? "none" : n === g.items.length ? "all" : "some";
  }

  isRecommended(item: CleanupItem, group: CleanupGroup) {
    if (group.id === "projects") {
      return item.lastActivity === null || Date.now() / 1000 - item.lastActivity > this.staleDays * 86400;
    }
    return item.recommended;
  }

  applyRecommendations(only?: CleanupGroup) {
    for (const g of only ? [only] : this.groups)
      for (const i of g.items) {
        if (this.isRecommended(i, g)) this.selected.add(i.id);
        else this.selected.delete(i.id);
      }
  }

  setStaleDays(n: number) {
    this.staleDays = n;
    app.saveSettings({ staleProjectDays: n });
    const projects = this.groups.find((g) => g.id === "projects");
    if (projects) this.applyRecommendations(projects);
  }

  toggle(id: string) {
    if (this.selected.has(id)) this.selected.delete(id);
    else this.selected.add(id);
  }

  toggleGroup(g: CleanupGroup) {
    const on = this.groupState(g) !== "all";
    for (const i of g.items) {
      if (on) this.selected.add(i.id);
      else this.selected.delete(i.id);
    }
  }

  /** `quiet`: Smart Care runs this and reports for itself. */
  async scan(quiet = false) {
    this.staleDays = app.settings.staleProjectDays;
    this.progress = null;
    this.setPhase("scanning");
    try {
      const report = await api.cleanup.scan(null, (p) => (this.progress = p));
      if (report.cancelled) {
        this.setPhase("home");
        return;
      }
      this.report = report;
      this.selected.clear();
      this.applyRecommendations();
      this.view = "summary";
      this.setPhase("results");
      if (!quiet && app.module !== "cleanup") app.toast(`Cleanup found ${bytes(this.foundSize)} you could free.`, "success");
    } catch (e) {
      app.toast(`Scan failed: ${e}`, "error");
      this.setPhase("home");
    }
  }

  stop() {
    api.cleanup.cancel();
  }

  review(groupId: string | null = null) {
    this.focusGroup = groupId ?? this.groups.find((g) => g.items.length)?.id ?? null;
    this.view = "review";
  }

  /** Drops an ignored item from the results on screen. */
  forget(id: string) {
    this.selected.delete(id);
    if (!this.report) return;
    this.report = {
      ...this.report,
      groups: this.report.groups
        .map((g) => {
          const items = g.items.filter((i) => i.id !== id);
          return { ...g, items, size: items.reduce((s, i) => s + i.size, 0) };
        })
        .filter((g) => g.items.length || g.note),
    };
  }

  async ignore(item: CleanupItem) {
    if (await app.ignore("item", `cleanup:${item.id}`, item.name, "cleanup")) this.forget(item.id);
  }

  requestClean() {
    if (this.risky.length) this.confirming = true;
    else this.clean();
  }

  async clean() {
    this.confirming = false;
    const ids = this.items.filter((i) => this.selected.has(i.id)).map((i) => i.id);
    if (!ids.length) return;
    this.status.clear();
    for (const id of ids) this.status.set(id, "pending");
    this.log = [];
    this.done = 0;
    this.total = ids.length;
    this.setPhase("cleaning");
    try {
      this.result = await api.cleanup.clean(ids, (ev) => {
        if (ev.status === "started") {
          this.status.set(ev.id, "working");
          this.log = [...this.log, { id: ev.id, name: ev.name, size: ev.size }].slice(-6);
        } else {
          this.status.set(ev.id, ev.status === "cleaned" ? "done" : "failed");
          this.done++;
        }
      });
      // Mirror the backend: cleaned items leave the report.
      const gone = new Set(this.result.cleaned);
      this.trashed = this.items.filter((i) => gone.has(i.id) && i.mode === "trash").reduce((s, i) => s + i.size, 0);
      if (this.report) {
        this.report = {
          ...this.report,
          groups: this.report.groups
            .map((g) => {
              const items = g.items.filter((i) => !gone.has(i.id));
              return { ...g, items, size: items.reduce((s, i) => s + i.size, 0) };
            })
            .filter((g) => g.items.length),
        };
      }
      for (const id of gone) this.selected.delete(id);
      this.setPhase("done");
      app.refreshVolumes();
    } catch (e) {
      app.toast(`Cleaning stopped: ${e}`, "error");
      this.setPhase("results");
    }
  }

  finish() {
    if (this.items.length) {
      this.view = "summary";
      this.setPhase("results");
    } else {
      this.startOver();
    }
  }

  startOver() {
    this.report = null;
    this.selected.clear();
    this.result = null;
    this.setPhase("home");
  }
}

export const cleanup = new Cleanup();
