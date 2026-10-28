// Smart Care: one scan across Cleanup, Performance and Applications, using their own
// stores so "Review" lands on that module's real results. My Clutter is left out on
// purpose: it's personal files, and those always need a human decision.
import { SvelteMap } from "svelte/reactivity";
import type { CleanupItem } from "../../lib/api";
import { bytes, plural } from "../../lib/format";
import { app } from "../../lib/state.svelte";
import { apps, key as appKey } from "../applications/apps.svelte";
import { cleanup } from "../cleanup/clean.svelte";
import { perf } from "../performance/perf.svelte";

export type Step = "cleanup" | "performance" | "applications";
export type StepState = "waiting" | "working" | "done" | "failed" | "skipped";

export const STEPS: { id: Step; title: string; working: string }[] = [
  { id: "cleanup", title: "Junk", working: "Looking for junk…" },
  { id: "performance", title: "Speed", working: "Checking memory and maintenance…" },
  { id: "applications", title: "Apps", working: "Looking for app leftovers…" },
];

class SmartCare {
  phase = $state<"home" | "scanning" | "results" | "running" | "done">("home");
  state = new SvelteMap<Step, StepState>();
  /** What each step achieved, in words, for the done screen. */
  outcome = new SvelteMap<Step, string>();
  freed = $state(0);

  /** Safe junk: what Cleanup recommends, minus anything carrying a warning. */
  junk = $derived<CleanupItem[]>(cleanup.groups.flatMap((g) => g.items.filter((i) => cleanup.isRecommended(i, g) && !i.warning)));
  junkSize = $derived(this.junk.reduce((s, i) => s + i.size, 0));
  tasks = $derived(perf.recommended);
  leftovers = $derived(apps.recommendedLeftovers());
  leftoverSize = $derived(this.leftovers.reduce((s, t) => s + apps.sizeOf(appKey(t)), 0));
  total = $derived(this.junkSize + this.leftoverSize);
  nothingToDo = $derived(!this.junk.length && !this.tasks.length && !this.leftovers.length);

  private setPhase(p: SmartCare["phase"]) {
    this.phase = p;
    app.phases.smartcare = p === "home" ? "home" : p === "scanning" || p === "running" ? "scanning" : "results";
  }

  async scan() {
    this.setPhase("scanning");
    for (const s of STEPS) this.state.set(s.id, "working");
    const run = async (id: Step, f: () => Promise<unknown>) => {
      try {
        await f();
        this.state.set(id, "done");
      } catch {
        this.state.set(id, "failed");
      }
    };
    await Promise.all([
      run("cleanup", () => cleanup.scan(true)),
      run("performance", () => perf.scan()),
      run("applications", () => apps.scan()),
    ]);
    this.setPhase("results");
  }

  /** Runs everything recommended. Maintenance asks for the password once. */
  async run() {
    this.setPhase("running");
    this.outcome.clear();
    this.freed = 0;
    for (const s of STEPS) this.state.set(s.id, "waiting");

    this.state.set("cleanup", this.junk.length ? "working" : "skipped");
    if (this.junk.length) {
      cleanup.selected.clear();
      for (const i of this.junk) cleanup.selected.add(i.id);
      await cleanup.clean();
      const r = cleanup.result;
      this.freed += r?.bytesFreed ?? 0;
      this.outcome.set("cleanup", r ? `${bytes(r.bytesFreed)} of junk removed` : "Didn't finish");
      this.state.set("cleanup", r && !r.failed.length ? "done" : "failed");
      // Leave Cleanup on its results rather than its own "done" screen.
      cleanup.finish();
    }

    const taskIds = this.tasks.map((t) => t.id);
    this.state.set("performance", taskIds.length ? "working" : "skipped");
    if (taskIds.length) {
      await perf.runTasks(taskIds);
      const ok = taskIds.filter((id) => perf.taskState.get(id) === "done").length;
      this.outcome.set("performance", `${ok} of ${plural(taskIds.length, "task")} done`);
      this.state.set("performance", ok === taskIds.length ? "done" : "failed");
    }

    const left = this.leftovers;
    this.state.set("applications", left.length ? "working" : "skipped");
    if (left.length) {
      apps.selected.clear();
      apps.select(left, true);
      await apps.remove();
      apps.sheet = "closed";
      const r = apps.result;
      this.freed += r?.bytesFreed ?? 0;
      this.outcome.set("applications", r ? `${plural(r.cleaned.length, "leftover")} moved to the Trash` : "Didn't finish");
      this.state.set("applications", r && !r.failed.length ? "done" : "failed");
    }

    this.setPhase("done");
    app.refreshVolumes();
  }

  startOver() {
    this.setPhase("home");
  }
}

export const smart = new SmartCare();
