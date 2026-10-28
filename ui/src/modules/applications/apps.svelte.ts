// Applications state, at module scope so results survive switching modules.
import type { Component } from "svelte";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { AppWindow, Download, PackageX, RefreshCw } from "@lucide/svelte";
import { api, type AppEntry, type AppTarget, type AppUpdate, type AppsReport, type CleanSummary, type Progress, type UpdatesReport } from "../../lib/api";
import { bytes, plural } from "../../lib/format";
import { app } from "../../lib/state.svelte";

export type Section = "apps" | "updates" | "leftovers" | "installers";

export const SECTIONS: { id: Section; title: string; blurb: string; icon: Component }[] = [
  { id: "apps", title: "Uninstaller", blurb: "Remove apps together with the support files, caches and helpers they scattered around.", icon: AppWindow },
  { id: "updates", title: "Updater", blurb: "Newer versions from the App Store, the apps' own update feeds and Homebrew.", icon: RefreshCw },
  { id: "leftovers", title: "Leftovers", blurb: "Files left behind by apps that are already gone.", icon: PackageX },
  { id: "installers", title: "Installation Files", blurb: "Disk images and installer packages that have done their job.", icon: Download },
];

const DAY = 86400;
/** Apps not opened for this long count as unused. */
export const UNUSED_DAYS = 180;

export function isUnused(a: AppEntry) {
  return a.lastUsed !== null && Date.now() / 1000 - a.lastUsed > UNUSED_DAYS * DAY;
}

/** Selection keys are "kind:id" so the three lists can share one set. */
export const key = (t: AppTarget) => `${t.kind}:${t.id}`;
function parse(k: string): AppTarget {
  const i = k.indexOf(":");
  return { kind: k.slice(0, i) as AppTarget["kind"], id: k.slice(i + 1) };
}

export type RemoveStatus = "pending" | "working" | "done" | "failed";

export interface UpdateState {
  status: "working" | "done" | "failed";
  /** Current step while working, the outcome after. */
  message: string;
}

class Apps {
  phase = $state<"home" | "scanning" | "results">("home");
  view = $state<"summary" | "review">("summary");
  section = $state<Section>("apps");
  progress = $state<Progress | null>(null);
  report = $state<AppsReport | null>(null);
  selected = new SvelteSet<string>();
  expanded = new SvelteSet<string>();

  // Removal run, shown in the sheet
  sheet = $state<"closed" | "confirm" | "working" | "done">("closed");
  status = new SvelteMap<string, RemoveStatus>();
  errors = new SvelteMap<string, string>();
  result = $state<CleanSummary | null>(null);

  unused = $derived(this.report?.apps.filter(isUnused) ?? []);

  // Updater: checked in the background so a slow network never delays the scan.
  updates = $state<UpdatesReport | null>(null);
  checkingUpdates = $state(false);
  updateStep = $state("");
  updatePicked = new SvelteSet<string>();
  updateState = new SvelteMap<string, UpdateState>();
  installing = $state(false);

  installable = $derived(this.updates?.updates.filter((u) => u.oneClick && !u.requires) ?? []);

  sizeOf(k: string): number {
    const { kind, id } = parse(k);
    const r = this.report;
    if (!r) return 0;
    if (kind === "app") {
      const a = r.apps.find((x) => x.id === id);
      return a ? a.size + a.relatedSize : 0;
    }
    if (kind === "leftover") return r.leftovers.find((x) => x.id === id)?.size ?? 0;
    return r.installers.find((x) => x.path === id)?.size ?? 0;
  }

  nameOf(k: string): string {
    const { kind, id } = parse(k);
    const r = this.report;
    if (kind === "app") return r?.apps.find((x) => x.id === id)?.name ?? id;
    if (kind === "installer") return r?.installers.find((x) => x.path === id)?.name ?? id;
    return id;
  }

  selectedSize = $derived([...this.selected].reduce((s, k) => s + this.sizeOf(k), 0));
  selectedApps = $derived(
    [...this.selected].filter((k) => k.startsWith("app:")).map((k) => this.report?.apps.find((a) => `app:${a.id}` === k)).filter((a): a is AppEntry => !!a),
  );

  private setPhase(p: "home" | "scanning" | "results") {
    this.phase = p;
    app.phases.applications = p;
  }

  async checkUpdates() {
    if (this.checkingUpdates) return;
    this.checkingUpdates = true;
    this.updateStep = "";
    try {
      this.updates = await api.apps.checkUpdates((s) => (this.updateStep = s));
      this.updatePicked.clear();
      this.updateState.clear();
      for (const u of this.installable) if (!u.running) this.updatePicked.add(u.id);
    } catch (e) {
      app.toast(`Couldn't check for updates: ${e}`, "error");
    } finally {
      this.checkingUpdates = false;
    }
  }

  /** One-click updates install in place; the rest open the App Store, app or website. */
  async installUpdates(ids: string[]) {
    if (!ids.length || this.installing) return;
    this.installing = true;
    for (const id of ids) this.updateState.set(id, { status: "working", message: "Waiting…" });
    try {
      const done = await api.apps.installUpdates(ids, (ev) => {
        if (ev.status === "started") this.updateState.set(ev.id, { status: "working", message: ev.step });
        else if (ev.status === "done") this.updateState.set(ev.id, { status: "done", message: ev.message });
        else this.updateState.set(ev.id, { status: "failed", message: ev.error });
      });
      for (const id of done) this.updatePicked.delete(id);
      const n = done.length;
      if (n) app.toast(`${plural(n, "app")} updated.`, "success");
      const failed = ids.filter((id) => this.updateState.get(id)?.status === "failed").length;
      if (failed) app.toast(`${plural(failed, "update")} didn't install. See the list for why.`, "error");
    } catch (e) {
      app.toast(`Updating stopped: ${e}`, "error");
    } finally {
      this.installing = false;
    }
  }

  async scan() {
    this.progress = null;
    this.setPhase("scanning");
    void this.checkUpdates();
    try {
      this.report = await api.apps.scan((p) => (this.progress = p));
      this.selected.clear();
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

  toggle(t: AppTarget) {
    const k = key(t);
    if (this.selected.has(k)) this.selected.delete(k);
    else this.selected.add(k);
  }

  has(t: AppTarget) {
    return this.selected.has(key(t));
  }

  select(targets: AppTarget[], on: boolean) {
    for (const t of targets) {
      if (on) this.selected.add(key(t));
      else this.selected.delete(key(t));
    }
  }

  /** Leftovers from vendors with nothing else installed, untouched for a month. */
  recommendedLeftovers(): AppTarget[] {
    const cutoff = Date.now() / 1000 - 30 * DAY;
    return (this.report?.leftovers ?? [])
      .filter((l) => !l.vendorInstalled && (l.lastActivity ?? 0) < cutoff)
      .map((l) => ({ kind: "leftover", id: l.id }));
  }

  async remove() {
    const keys = [...this.selected];
    if (!keys.length) return;
    this.status.clear();
    this.errors.clear();
    for (const k of keys) this.status.set(k, "pending");
    this.sheet = "working";
    try {
      const targets = keys.map(parse);
      const kindOf = new Map(targets.map((t) => [t.id, t.kind]));
      const k = (id: string) => `${kindOf.get(id)}:${id}`;
      this.result = await api.apps.remove(targets, (ev) => {
        if (ev.status === "started") this.status.set(k(ev.id), "working");
        else if (ev.status === "cleaned") this.status.set(k(ev.id), "done");
        else {
          this.status.set(k(ev.id), "failed");
          this.errors.set(k(ev.id), ev.error);
        }
      });
      const gone = new Set(this.result.cleaned.map(k));
      const r = this.report;
      if (r) {
        this.report = {
          ...r,
          apps: r.apps.filter((a) => !gone.has(`app:${a.id}`)),
          leftovers: r.leftovers.filter((l) => !gone.has(`leftover:${l.id}`)),
          installers: r.installers.filter((i) => !gone.has(`installer:${i.path}`)),
        };
      }
      for (const g of gone) this.selected.delete(g);
      this.sheet = "done";
      if (this.result.cleaned.length) {
        app.toast(`Moved ${plural(this.result.cleaned.length, "item")} (${bytes(this.result.bytesFreed)}) to the Trash. Empty it to free the space.`, "success");
      }
      app.refreshVolumes();
    } catch (e) {
      app.toast(`Removal stopped: ${e}`, "error");
      this.sheet = "closed";
    }
  }

  startOver() {
    this.report = null;
    this.selected.clear();
    this.setPhase("home");
  }
}

export const apps = new Apps();
