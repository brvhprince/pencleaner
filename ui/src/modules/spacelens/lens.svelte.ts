// Space Lens state lives at module scope so a scan keeps going, and results stay put,
// while you visit other modules.
import { SvelteMap } from "svelte/reactivity";
import { api, type Entry, type Listing, type Progress, type RemoveEvent, type ScanSummary } from "../../lib/api";
import { bytes, plural } from "../../lib/format";
import { app } from "../../lib/state.svelte";

export interface Target {
  name: string;
  path: string;
  /** Used bytes when scanning a whole volume; drives the progress ring. */
  expected: number | null;
}

type Phase = "home" | "scanning" | "results";

function isInside(path: string, parent: string) {
  return path !== parent && path.startsWith(parent.endsWith("/") ? parent : parent + "/");
}

class Lens {
  phase = $state<Phase>("home");
  target = $state<Target | null>(null);
  progress = $state<Progress | null>(null);
  summary = $state<ScanSummary | null>(null);
  listing = $state<Listing | null>(null);
  back = $state<string[]>([]);
  forward = $state<string[]>([]);
  hovered = $state<string | null>(null);
  selected = new SvelteMap<string, Entry>();

  selectedSize = $derived([...this.selected.values()].reduce((sum, e) => sum + e.size, 0));

  private setPhase(p: Phase) {
    this.phase = p;
    app.phases.spacelens = p;
  }

  async scan() {
    if (!this.target) return;
    const target = this.target;
    this.progress = null;
    this.setPhase("scanning");
    try {
      const summary = await api.spaceLens.scan(target.path, (p) => (this.progress = p));
      if (summary.cancelled) {
        this.setPhase("home");
        return;
      }
      this.summary = summary;
      this.back = [];
      this.forward = [];
      this.selected.clear();
      this.listing = await api.spaceLens.list(target.path);
      this.setPhase("results");
      if (app.module !== "spacelens") app.toast(`Space Lens finished: ${bytes(summary.size)} mapped.`, "success");
    } catch (e) {
      app.toast(`Scan failed: ${e}`, "error");
      this.setPhase("home");
    }
  }

  stop() {
    api.spaceLens.cancel();
  }

  async open(path: string) {
    if (this.listing?.path === path) return;
    const next = await api.spaceLens.list(path);
    if (this.listing) this.back.push(this.listing.path);
    this.forward = [];
    this.listing = next;
    this.hovered = null;
  }

  async goBack() {
    const path = this.back.pop();
    if (!path) return;
    if (this.listing) this.forward.push(this.listing.path);
    this.listing = await api.spaceLens.list(path);
  }

  async goForward() {
    const path = this.forward.pop();
    if (!path) return;
    if (this.listing) this.back.push(this.listing.path);
    this.listing = await api.spaceLens.list(path);
  }

  async reload() {
    if (this.listing) this.listing = await api.spaceLens.list(this.listing.path);
  }

  startOver() {
    api.spaceLens.discard();
    this.listing = null;
    this.summary = null;
    this.selected.clear();
    this.setPhase("home");
  }

  /** True when an ancestor folder is already selected, so this entry goes with it. */
  covered(path: string) {
    for (const p of this.selected.keys()) if (isInside(path, p)) return true;
    return false;
  }

  toggle(entry: Entry) {
    if (entry.protected || this.covered(entry.path)) return;
    if (this.selected.has(entry.path)) {
      this.selected.delete(entry.path);
      return;
    }
    // Selecting a folder subsumes anything already picked inside it.
    for (const p of [...this.selected.keys()]) if (isInside(p, entry.path)) this.selected.delete(p);
    this.selected.set(entry.path, entry);
  }

  selectAll() {
    for (const e of this.listing?.entries ?? []) if (!e.protected && !this.selected.has(e.path)) this.toggle(e);
  }

  selectNone() {
    for (const e of this.listing?.entries ?? []) this.selected.delete(e.path);
  }

  async removeSelected(onEvent: (e: RemoveEvent) => void) {
    const items = [...this.selected.values()].map((e) => ({ path: e.path, size: e.size }));
    const result = await api.remove(items, "trash", onEvent);
    for (const [path] of result.removed) this.selected.delete(path);
    await Promise.all([this.reload(), app.refreshVolumes()]);
    if (result.removed.length) {
      app.toast(
        `Moved ${plural(result.removed.length, "item")} (${bytes(result.bytesFreed)}) to the Trash. Empty it to free the space.`,
        "success",
      );
    }
    if (result.failed.length) {
      app.toast(`${plural(result.failed.length, "item")} couldn't be removed: ${result.failed[0][1]}`, "error");
    }
    return result;
  }
}

export const lens = new Lens();
