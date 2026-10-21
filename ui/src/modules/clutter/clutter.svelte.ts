// My Clutter state, at module scope so results survive switching modules.
import type { Component } from "svelte";
import { SvelteSet } from "svelte/reactivity";
import { Copy, Download, FileClock, Images } from "@lucide/svelte";
import { api, type ClutterReport, type FileInfo, type FileSet, type Progress } from "../../lib/api";
import { bytes, plural } from "../../lib/format";
import { app } from "../../lib/state.svelte";

export type Section = "duplicates" | "similar" | "large" | "downloads";

export interface SectionMeta {
  id: Section;
  title: string;
  blurb: string;
  icon: Component;
}

export const SECTIONS: SectionMeta[] = [
  { id: "duplicates", title: "Duplicates", blurb: "Identical files stored in more than one place.", icon: Copy },
  { id: "similar", title: "Similar Images", blurb: "Burst shots, edits and re-exports of the same picture. Keep the best one.", icon: Images },
  { id: "large", title: "Large & Old Files", blurb: "Files over 50 MB, with when you last opened them.", icon: FileClock },
  { id: "downloads", title: "Downloads", blurb: "Everything sitting in your Downloads folder.", icon: Download },
];

const YEAR = 365 * 86400;
const now = () => Date.now() / 1000;

/** Last sign of use: opened if Spotlight knows, else modified. */
export function lastTouched(f: FileInfo) {
  return f.lastUsed ?? f.modified;
}

export interface Target {
  name: string;
  path: string;
}

class Clutter {
  phase = $state<"home" | "scanning" | "results">("home");
  view = $state<"summary" | "review">("summary");
  section = $state<Section>("duplicates");
  target = $state<Target | null>(null);
  progress = $state<Progress | null>(null);
  report = $state<ClutterReport | null>(null);
  selected = new SvelteSet<string>();
  removing = $state<{ done: number; total: number } | null>(null);

  /** Every file the report mentions, by path (a file can appear in more than one view). */
  files = $derived.by(() => {
    const m = new Map<string, FileInfo>();
    const r = this.report;
    if (!r) return m;
    for (const f of [...r.large, ...r.downloads, ...r.duplicates.flatMap((s) => s.files), ...r.similar.flatMap((s) => s.files)])
      m.set(f.path, f);
    return m;
  });
  selectedSize = $derived([...this.selected].reduce((s, p) => s + (this.files.get(p)?.size ?? 0), 0));
  oldLarge = $derived(this.report?.large.filter((f) => (lastTouched(f) ?? 0) < now() - YEAR) ?? []);

  private setPhase(p: "home" | "scanning" | "results") {
    this.phase = p;
    app.phases.clutter = p;
  }

  stats(section: Section) {
    const r = this.report;
    if (!r) return { count: 0, size: 0 };
    switch (section) {
      case "duplicates":
        return { count: r.duplicates.length, size: r.duplicates.reduce((s, d) => s + d.reclaimable, 0) };
      case "similar":
        return { count: r.similar.length, size: r.similar.reduce((s, d) => s + d.reclaimable, 0) };
      case "large":
        return { count: r.large.length, size: r.large.reduce((s, f) => s + f.size, 0) };
      case "downloads":
        return { count: r.downloads.length, size: r.downloads.reduce((s, f) => s + f.size, 0) };
    }
  }

  async scan() {
    if (!this.target) return;
    this.progress = null;
    this.setPhase("scanning");
    try {
      const report = await api.clutter.scan(this.target.path, (p) => (this.progress = p));
      if (report.cancelled) {
        this.setPhase("home");
        return;
      }
      this.report = report;
      this.selected.clear();
      this.view = "summary";
      this.setPhase("results");
      if (app.module !== "clutter") app.toast(`My Clutter found ${plural(this.files.size, "file")} to sort through.`, "success");
    } catch (e) {
      app.toast(`Scan failed: ${e}`, "error");
      this.setPhase("home");
    }
  }

  stop() {
    api.clutter.cancel();
  }

  review(section: Section) {
    this.section = section;
    this.view = "review";
  }

  private setsWith(path: string): FileSet[] {
    const r = this.report;
    if (!r) return [];
    return [...r.duplicates, ...r.similar].filter((s) => s.files.some((f) => f.path === path));
  }

  /** Selecting the last unselected copy in a set is refused: one copy always stays. */
  toggle(path: string) {
    if (this.selected.has(path)) {
      this.selected.delete(path);
      return;
    }
    for (const s of this.setsWith(path)) {
      if (s.files.every((f) => f.path === path || this.selected.has(f.path))) {
        app.toast("That's the last copy in its group. At least one stays.");
        return;
      }
    }
    this.selected.add(path);
  }

  /** Ticks every copy except the suggested one to keep. */
  autoSelect(sets: FileSet[]) {
    for (const s of sets) for (const f of s.files) if (f.path !== s.keep) this.selected.add(f.path);
  }

  unselect(paths: string[]) {
    for (const p of paths) this.selected.delete(p);
  }

  async remove() {
    const ids = [...this.selected];
    if (!ids.length) return;
    this.removing = { done: 0, total: ids.length };
    try {
      const result = await api.clutter.remove(ids, (ev) => {
        if (ev.status !== "started" && this.removing) this.removing.done++;
      });
      const gone = new Set(result.removed.map(([p]) => p));
      const r = this.report;
      if (r) {
        const keepFile = (f: FileInfo) => !gone.has(f.path);
        const pruneSets = (sets: FileSet[]) =>
          sets.map((s) => ({ ...s, files: s.files.filter(keepFile) })).filter((s) => s.files.length > 1);
        this.report = {
          ...r,
          large: r.large.filter(keepFile),
          downloads: r.downloads.filter(keepFile),
          duplicates: pruneSets(r.duplicates),
          similar: pruneSets(r.similar),
        };
      }
      for (const p of gone) this.selected.delete(p);
      if (result.removed.length) {
        app.toast(
          `Moved ${plural(result.removed.length, "file")} (${bytes(result.bytesFreed)}) to the Trash. Empty it to free the space.`,
          "success",
        );
      }
      if (result.failed.length) app.toast(`${plural(result.failed.length, "file")} couldn't be moved: ${result.failed[0][1]}`, "error");
      app.refreshVolumes();
    } catch (e) {
      app.toast(`Couldn't move files: ${e}`, "error");
    } finally {
      this.removing = null;
    }
  }

  startOver() {
    this.report = null;
    this.selected.clear();
    this.setPhase("home");
  }
}

export const clutter = new Clutter();
