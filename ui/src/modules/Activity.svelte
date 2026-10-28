<script lang="ts">
  import { onMount } from "svelte";
  import { api, type ActivityEvent, type MemoryStats } from "../lib/api";
  import Illustration from "../lib/illustrations/Illustration.svelte";
  import { ago, bytes, plural } from "../lib/format";
  import { MODULE, type ModuleId } from "../lib/modules";
  import { app } from "../lib/state.svelte";
  import FreedChart from "./activity/FreedChart.svelte";

  let events = $state<ActivityEvent[]>([]);
  let loaded = $state(false);
  let memory = $state<MemoryStats | null>(null);
  let confirmClear = $state(false);

  onMount(async () => {
    [events, memory] = await Promise.all([
      api.activity.list().catch(() => []),
      api.perf.memory().then((m) => m.memory).catch(() => null),
    ]);
    loaded = true;
  });

  const freed = $derived(events.reduce((s, e) => s + e.bytes, 0));
  const removed = $derived(events.filter((e) => e.bytes > 0).reduce((s, e) => s + e.items, 0));
  const tasks = $derived(events.filter((e) => e.action === "Ran maintenance").reduce((s, e) => s + e.items, 0));
  const since = $derived(events[0]?.at ?? null);
  const recent = $derived([...events].reverse().slice(0, 14));
  const volume = $derived(app.volumes.find((v) => v.isRoot));

  const byModule = $derived.by(() => {
    const m = new Map<ModuleId, { bytes: number; count: number }>();
    for (const e of events) {
      const cur = m.get(e.module) ?? { bytes: 0, count: 0 };
      cur.bytes += e.bytes;
      cur.count += 1;
      m.set(e.module, cur);
    }
    return [...m].sort((a, b) => b[1].bytes - a[1].bytes);
  });
  const topModule = $derived(Math.max(1, ...byModule.map(([, v]) => v.bytes)));

  const health = $derived.by(() => {
    if (!volume) return null;
    const freePct = volume.free / volume.total;
    const pressure = memory?.pressure ?? "normal";
    if (freePct < 0.1 || pressure === "critical") return { word: "Needs attention", tone: "bad" };
    if (freePct < 0.2 || pressure === "warning" || (memory && memory.swapUsed > 4e9)) return { word: "Fair", tone: "warn" };
    return { word: "Good", tone: "good" };
  });

  async function clear() {
    if (!confirmClear) {
      confirmClear = true;
      setTimeout(() => (confirmClear = false), 4000);
      return;
    }
    await api.activity.clear();
    events = [];
    confirmClear = false;
  }
</script>

<section class="activity">
  <header data-tauri-drag-region>
    <h1>My Activity</h1>
    {#if since}<span class="muted">Since {new Date(since * 1000).toLocaleDateString([], { dateStyle: "long" })}</span>{/if}
  </header>

  {#if loaded && !events.length}
    <div class="empty">
      <Illustration name={MODULE.activity.illustration} size={140} hero />
      <h2>Nothing yet</h2>
      <p class="muted">Everything PenCleaner frees, removes or runs will show up here.</p>
      <button class="btn lg" onclick={() => app.go("smartcare")}>Run Smart Care</button>
    </div>
  {:else if loaded}
    <div class="row">
      <div class="card glass hero">
        <span class="label">Space freed</span>
        <span class="figure">{bytes(freed)}</span>
        <span class="muted">across {plural(events.length, "action")}</span>
      </div>
      <div class="card glass">
        <span class="label">Items removed</span>
        <span class="value tabular">{removed.toLocaleString()}</span>
      </div>
      <div class="card glass">
        <span class="label">Maintenance tasks run</span>
        <span class="value tabular">{tasks.toLocaleString()}</span>
      </div>
      {#if health && volume}
        <div class="card glass">
          <span class="label">Mac health</span>
          <span class="value {health.tone}">{health.word}</span>
          <span class="muted small">{bytes(volume.free)} free on {volume.name}{memory ? ` · ${bytes(memory.swapUsed)} swap` : ""}</span>
        </div>
      {/if}
    </div>

    <div class="row two">
      <div class="card glass">
        <h3>Space freed, last 30 days</h3>
        <FreedChart {events} />
      </div>
      <div class="card glass">
        <h3>By module</h3>
        <ul class="modules">
          {#each byModule as [id, v] (id)}
            {@const m = MODULE[id]}
            <li>
              <span class="badge" style:background="linear-gradient(160deg, {m.theme.accentStrong}, {m.theme.accent})"><m.icon size={13} /></span>
              <span class="mname">{m.title}</span>
              <span class="mval tabular">{v.bytes ? bytes(v.bytes) : plural(v.count, "action")}</span>
              <span class="track"><span style:width="{(v.bytes / topModule) * 100}%"></span></span>
            </li>
          {/each}
        </ul>
      </div>
    </div>

    <div class="card glass recent">
      <div class="rhead">
        <h3>Recent</h3>
        <button class="link" onclick={clear}>{confirmClear ? "Click again to clear history" : "Clear History"}</button>
      </div>
      <ul>
        {#each recent as e, i (e.at + ":" + i)}
          {@const m = MODULE[e.module]}
          <li>
            <span class="badge" style:background="linear-gradient(160deg, {m.theme.accentStrong}, {m.theme.accent})"><m.icon size={13} /></span>
            <span class="what">
              <span>{e.action}</span>
              <span class="muted small">{[e.detail, e.items ? plural(e.items, "item") : null].filter(Boolean).join(" · ")}</span>
            </span>
            <span class="tabular">{e.bytes ? bytes(e.bytes) : ""}</span>
            <span class="muted small when">{ago(e.at)}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>

<style>
  .activity {
    height: 100%;
    overflow-y: auto;
    padding: 0 28px 28px;
  }
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 46px 0 18px;
  }
  h1 {
    font-size: 32px;
    font-weight: 400;
  }
  .row {
    display: grid;
    grid-template-columns: 1.4fr 1fr 1fr 1.2fr;
    gap: 12px;
    margin-bottom: 12px;
  }
  .row.two {
    grid-template-columns: 1.7fr 1fr;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px;
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--text-2);
  }
  .figure {
    font-size: 48px;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }
  .value {
    font-size: 26px;
    font-weight: 600;
    margin-top: auto;
  }
  .value.good {
    color: #9be686;
  }
  .value.warn {
    color: #ffd08a;
  }
  .value.bad {
    color: #ffb4b4;
  }
  .small {
    font-size: 11.5px;
  }
  h3 {
    font-size: 13.5px;
    font-weight: 600;
    margin-bottom: 8px;
  }
  .modules {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .modules li {
    display: grid;
    grid-template-columns: 24px 1fr auto;
    grid-template-rows: auto 4px;
    align-items: center;
    column-gap: 10px;
    row-gap: 4px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 7px;
    grid-row: span 2;
  }
  .mname {
    font-weight: 500;
  }
  .mval {
    font-size: 12px;
    color: var(--text-2);
  }
  .track {
    grid-column: 2 / span 2;
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.08);
  }
  .track span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: #6c88f0;
  }
  .recent ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .recent li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .what .small {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .when {
    width: 92px;
    text-align: right;
  }
  .rhead {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .link {
    font-size: 12px;
    color: var(--text-2);
    text-decoration: underline;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding-top: 60px;
  }
  .empty h2 {
    margin-top: 12px;
    font-size: 20px;
    font-weight: 500;
  }
  .empty .btn {
    margin-top: 12px;
  }
</style>
