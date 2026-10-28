<script lang="ts">
  import { RotateCcw } from "@lucide/svelte";
  import { bytes, plural } from "../../lib/format";
  import MemoryBar from "./MemoryBar.svelte";
  import { perf } from "./perf.svelte";

  const r = $derived(perf.report!);
  const ram = $derived(r.tasks.find((t) => t.id === "free-ram"));
  const running = $derived(r.items.filter((i) => i.enabled && i.running).length);
  const broken = $derived(r.items.filter((i) => i.broken).length);

  // Live memory while this screen is visible.
  $effect(() => {
    const t = setInterval(() => perf.refreshMemory(), 4000);
    return () => clearInterval(t);
  });
</script>

<section class="summary">
  <div class="topbar" data-tauri-drag-region>
    <button class="link" onclick={() => perf.startOver()}><RotateCcw size={13} /> Start Over</button>
    <span class="title" data-tauri-drag-region>Performance</span>
  </div>

  <header>
    <h1>Apply curated recommendations<br />or run performance tasks manually.</h1>
    <button class="btn lg" onclick={() => perf.review("tasks")}>View All Tasks</button>
  </header>

  <div class="grid">
    <div class="card glass ram">
      <h3>Free Up Your RAM</h3>
      <p>{ram?.reason ?? "Memory is in good shape right now."}</p>
      <div class="big tabular">{bytes(r.memory.used)} <span>of {bytes(r.memory.total)} in use</span></div>
      <MemoryBar memory={r.memory} />
      <div class="actions">
        <button class="btn" onclick={() => perf.review("memory")}>What's Using It</button>
        <button class="btn primary" onclick={() => perf.runTasks(["free-ram"])} disabled={perf.running}>
          {perf.taskState.get("free-ram") === "running" ? "Freeing…" : "Free Up"}
        </button>
      </div>
      {#if perf.taskResult.get("free-ram")}<p class="result">{perf.taskResult.get("free-ram")?.message}</p>{/if}
    </div>

    <div class="card glass">
      <h3>{perf.recommended.length ? `${plural(perf.recommended.length, "Maintenance Task")} Recommended` : "Maintenance"}</h3>
      <p>{perf.recommended.map((t) => t.title).join(", ") || "Nothing needs doing right now."}</p>
      <div class="actions">
        <button class="btn" onclick={() => perf.review("tasks")}>Review</button>
        {#if perf.recommended.length}
          <button class="btn primary" onclick={() => perf.runTasks(perf.recommended.map((t) => t.id))} disabled={perf.running}>
            {perf.running ? "Running…" : "Run Tasks"}
          </button>
        {/if}
      </div>
    </div>

    <div class="card glass">
      <h3>{r.snapshots.length ? "Thin Out Time Machine Snapshots" : "No Local Snapshots"}</h3>
      <p>
        {r.snapshots.length
          ? `${plural(r.snapshots.length, "snapshot")}${r.purgeable ? `, holding about ${bytes(r.purgeable)}` : ""}. Your backup drive isn't touched.`
          : "Nothing is holding hidden space."}
      </p>
      <div class="actions">
        <button class="btn" onclick={() => perf.review("snapshots")}>Learn More</button>
        {#if r.snapshots.length}
          <button class="btn primary" onclick={() => perf.deleteSnapshots()} disabled={perf.deletingSnapshots}>
            {perf.deletingSnapshots ? "Removing…" : "Remove"}
          </button>
        {/if}
      </div>
    </div>

    <button class="card glass" onclick={() => perf.review("login")}>
      <h3>You Have {plural(perf.loginItems.length, "Login Item")}</h3>
      <p>Apps and helpers that start every time you log in.</p>
      <span class="btn push">Review</span>
    </button>

    <button class="card glass" onclick={() => perf.review("background")}>
      <h3>{plural(perf.backgroundItems.length, "Background Item")} Found</h3>
      <p>{running} running now{broken ? `, ${broken} broken (their program is gone)` : ""}.</p>
      <span class="btn push">Review</span>
    </button>
  </div>
</section>

<style>
  .summary {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 28px 24px;
  }
  .topbar {
    display: flex;
    align-items: center;
    height: var(--titlebar-h);
    flex: none;
  }
  .title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-weight: 600;
    color: var(--text-2);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
    color: var(--text-2);
  }
  .link:hover {
    color: var(--text);
  }
  header {
    text-align: center;
    padding: 6px 0 24px;
  }
  h1 {
    font-size: 21px;
    font-weight: 500;
    line-height: 1.35;
    margin-bottom: 14px;
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1.15fr 1fr 1fr;
    grid-template-rows: 1fr 1fr;
    gap: 12px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 18px;
    text-align: left;
    overflow: hidden;
  }
  button.card:hover {
    background: var(--glass-2);
  }
  .ram {
    grid-row: 1 / span 2;
  }
  h3 {
    font-size: 17px;
    font-weight: 500;
  }
  .card p {
    margin-top: 6px;
    font-size: 12.5px;
    line-height: 1.4;
    color: var(--text-2);
  }
  .big {
    margin: auto 0 12px;
    font-size: 34px;
    font-weight: 200;
  }
  .big span {
    font-size: 13px;
    color: var(--text-2);
    font-weight: 400;
  }
  .ram :global(.mem) {
    width: 100%;
  }
  .actions,
  .push {
    margin-top: auto;
    align-self: flex-end;
    display: flex;
    gap: 8px;
  }
  .ram .actions {
    margin-top: 18px;
  }
  .result {
    align-self: flex-end;
    font-size: 12px !important;
  }
</style>
