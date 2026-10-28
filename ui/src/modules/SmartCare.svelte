<script lang="ts">
  import { AppWindow, Check, CircleAlert, HardDrive, Minus, RotateCcw, SprayCan, Zap } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Illustration from "../lib/illustrations/Illustration.svelte";
  import { bytes, plural } from "../lib/format";
  import { MODULE } from "../lib/modules";
  import { app } from "../lib/state.svelte";
  import { apps } from "./applications/apps.svelte";
  import { cleanup } from "./cleanup/clean.svelte";
  import { perf } from "./performance/perf.svelte";
  import { STEPS, smart, type Step } from "./smartcare/smart.svelte";

  const module = MODULE.smartcare;
  const ICON: Record<Step, Component> = { cleanup: SprayCan, performance: Zap, applications: AppWindow };
  const volume = $derived(app.volumes.find((v) => v.isRoot));

  function open(step: Step) {
    if (step === "cleanup") {
      cleanup.view = "summary";
      app.go("cleanup");
    } else if (step === "performance") {
      perf.review("tasks");
      app.go("performance");
    } else {
      apps.review("leftovers");
      app.go("applications");
    }
  }

  // Browser preview only: #autoscan&run drives the flow for screenshots.
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await smart.scan();
      if (q.has("run")) await smart.run();
    })();
  });
</script>

{#snippet stepState(id: Step)}
  {@const s = smart.state.get(id)}
  <span class="st {s}">
    {#if s === "working"}<span class="spin"></span>{:else if s === "done"}<Check size={14} />{:else if s === "failed"}<CircleAlert size={14} />{:else if s === "skipped"}<Minus size={14} />{/if}
  </span>
{/snippet}

{#if smart.phase === "home"}
  <ModuleHome {module} />
  <ScanButton label="Scan" onclick={() => smart.scan()} />
{:else if smart.phase === "scanning" || smart.phase === "running"}
  <section class="center">
    <div class="drag" data-tauri-drag-region></div>
    <Illustration name={module.illustration} size={200} hero active />
    <h2>{smart.phase === "scanning" ? "Checking your Mac…" : "Taking care of it…"}</h2>
    <ul class="steps">
      {#each STEPS as s (s.id)}
        {@const Icon = ICON[s.id]}
        <li><Icon size={15} /><span>{smart.state.get(s.id) === "working" ? s.working : s.title}</span>{@render stepState(s.id)}</li>
      {/each}
    </ul>
    {#if smart.phase === "running" && smart.tasks.some((t) => t.admin)}
      <p class="muted hint">macOS will ask for your password once, for the maintenance tasks.</p>
    {/if}
  </section>
  {#if smart.phase === "scanning"}<ScanButton label="Scanning" busy onclick={() => {}} disabled />{/if}
{:else if smart.phase === "results"}
  <section class="results">
    <div class="topbar" data-tauri-drag-region>
      <button class="link" onclick={() => smart.startOver()}><RotateCcw size={13} /> Start Over</button>
      <span class="title" data-tauri-drag-region>Smart Care</span>
    </div>
    <header>
      {#if smart.nothingToDo}
        <h1>Your Mac is in good shape.</h1>
        <p class="muted">Nothing safe to clean or run right now.</p>
      {:else}
        <p class="eyebrow">Ready to run</p>
        <h1 class="tabular">{bytes(smart.total)}</h1>
        <p class="muted">to free, plus {plural(smart.tasks.length, "maintenance task")}. Only safe, recommended items are included; review any of them first.</p>
      {/if}
    </header>
    <div class="tiles">
      <div class="tile glass">
        <span class="badge"><SprayCan size={16} /></span>
        <h3>Junk</h3>
        <p class="big tabular">{bytes(smart.junkSize)}</p>
        <p class="muted">{plural(smart.junk.length, "item")}: caches, logs, stale build output</p>
        <button class="btn" onclick={() => open("cleanup")}>Review</button>
      </div>
      <div class="tile glass">
        <span class="badge"><Zap size={16} /></span>
        <h3>Speed</h3>
        <p class="big tabular">{plural(smart.tasks.length, "task")}</p>
        <p class="muted">{smart.tasks.map((t) => t.title).join(", ") || "Nothing needs doing"}</p>
        <button class="btn" onclick={() => open("performance")}>Review</button>
      </div>
      <div class="tile glass">
        <span class="badge"><AppWindow size={16} /></span>
        <h3>App Leftovers</h3>
        <p class="big tabular">{bytes(smart.leftoverSize)}</p>
        <p class="muted">{plural(smart.leftovers.length, "removed app")} left files behind</p>
        <button class="btn" onclick={() => open("applications")}>Review</button>
      </div>
      {#if volume}
        <div class="tile glass">
          <span class="badge"><HardDrive size={16} /></span>
          <h3>{volume.name}</h3>
          <p class="big tabular">{bytes(volume.free)} <span>free</span></p>
          <div class="bar"><div style:width="{(volume.used / volume.total) * 100}%"></div></div>
          <p class="muted">Your own files aren't touched here. Use My Clutter or Space Lens for those.</p>
          <button class="btn" onclick={() => app.go("spacelens")}>Open Space Lens</button>
        </div>
      {/if}
    </div>
  </section>
  <ScanButton label="Run" onclick={() => smart.run()} disabled={smart.nothingToDo} />
{:else}
  <section class="center">
    <div class="drag" data-tauri-drag-region></div>
    <Illustration name="achievements" size={150} hero />
    <p class="eyebrow">All done</p>
    <h1 class="tabular">{bytes(smart.freed)}</h1>
    <p class="muted">freed{smart.freed ? " (moved items sit in the Trash until you empty it)" : ""}.</p>
    <ul class="steps">
      {#each STEPS as s (s.id)}
        {@const Icon = ICON[s.id]}
        <li><Icon size={15} /><span>{smart.outcome.get(s.id) ?? `${s.title}: nothing to do`}</span>{@render stepState(s.id)}</li>
      {/each}
    </ul>
    <button class="btn lg primary" onclick={() => smart.startOver()}>Done</button>
  </section>
{/if}

<style>
  .drag {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--titlebar-h);
  }
  .center {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 20px 40px 110px;
    text-align: center;
  }
  h2 {
    margin-top: 22px;
    font-size: 28px;
    font-weight: 300;
  }
  .steps {
    list-style: none;
    padding: 0;
    margin: 20px 0 0;
    width: 340px;
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.07);
    margin-bottom: 6px;
    text-align: left;
  }
  .steps span:first-of-type {
    flex: 1;
  }
  .st {
    display: grid;
    place-items: center;
    width: 18px;
  }
  .st.failed {
    color: #ffb4b4;
  }
  .st.skipped {
    color: var(--text-3);
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #fff;
    animation: spin 0.8s linear infinite;
  }
  .hint {
    margin-top: 14px;
    font-size: 12px;
  }
  .results {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 28px 120px;
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
  header {
    text-align: center;
    margin: auto 0 26px;
  }
  .eyebrow {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
    margin-top: 16px;
  }
  h1 {
    font-size: 56px;
    font-weight: 200;
    margin: 4px 0;
  }
  header h1:only-child,
  header h1:first-child {
    font-size: 30px;
    font-weight: 300;
  }
  header p.muted {
    max-width: 480px;
    margin: 0 auto;
    line-height: 1.5;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
    margin-bottom: auto;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 16px;
    min-height: 200px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    background: linear-gradient(160deg, var(--accent-strong), var(--accent));
    margin-bottom: 6px;
  }
  .tile h3 {
    font-size: 14px;
    font-weight: 600;
  }
  .tile .big {
    font-size: 24px;
    font-weight: 300;
  }
  .big span {
    font-size: 13px;
    color: var(--text-2);
  }
  .tile .muted {
    font-size: 12px;
    line-height: 1.4;
  }
  .tile .btn {
    margin-top: auto;
  }
  .bar {
    width: 100%;
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.15);
    overflow: hidden;
    margin: 4px 0 6px;
  }
  .bar div {
    height: 100%;
    background: #fff;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
