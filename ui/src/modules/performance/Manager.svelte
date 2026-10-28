<script lang="ts">
  import { ChevronLeft, CircleAlert, CircleCheck, ExternalLink, Lock, Settings, Trash, TriangleAlert } from "@lucide/svelte";
  import { untrack } from "svelte";
  import { api, type LaunchItem } from "../../lib/api";
  import AppIcon from "../../lib/components/AppIcon.svelte";
  import Checkbox from "../../lib/components/Checkbox.svelte";
  import Switch from "../../lib/components/Switch.svelte";
  import { ago, bytes, plural } from "../../lib/format";
  import MemoryBar from "./MemoryBar.svelte";
  import { SECTIONS, perf } from "./perf.svelte";

  const r = $derived(perf.report!);
  const meta = $derived(SECTIONS.find((s) => s.id === perf.section)!);

  const count = (id: (typeof SECTIONS)[number]["id"]) =>
    id === "tasks"
      ? perf.recommended.length
      : id === "login"
        ? perf.loginItems.length
        : id === "background"
          ? perf.backgroundItems.length
          : id === "snapshots"
            ? r.snapshots.length
            : null;

  const agents = $derived(perf.backgroundItems.filter((i) => i.kind === "agent"));
  const daemons = $derived(perf.backgroundItems.filter((i) => i.kind === "daemon"));

  // Live memory while the Memory section is open. untrack: refreshing writes the report,
  // which must not re-run this effect (that loops forever).
  $effect(() => {
    if (perf.section !== "memory") return;
    untrack(() => perf.refreshMemory());
    const t = setInterval(() => perf.refreshMemory(), 3000);
    return () => clearInterval(t);
  });

  function owner(i: LaunchItem) {
    return [i.appName ?? i.developer, i.id !== i.name ? i.id : null].filter(Boolean).join(" · ");
  }
</script>

{#snippet itemRow(i: LaunchItem)}
  <li class:off={!i.enabled}>
    {#if i.appPath}<AppIcon path={i.appPath} size={30} />{:else}<span class="kind"><Settings size={15} /></span>{/if}
    <span class="text">
      <span class="name">
        {i.name}
        {#if i.running}<em class="tag live">Running</em>{/if}
        {#if i.broken}<em class="tag bad" title="Its plist or program no longer exists">Broken</em>{/if}
        {#if i.kind === "daemon"}<span class="lock" title="Runs for all users; changing it asks for your password"><Lock size={11} /></span>{/if}
      </span>
      <span class="detail" title={i.plist ?? i.executable ?? ""}>{owner(i) || i.plist || i.executable}</span>
    </span>
    {#if i.controllable}
      {#if i.plist}
        <Switch on={i.enabled} label="{i.name} enabled" busy={perf.busyItem === i.id} onchange={(v) => perf.setItem(i, v)} />
      {/if}
      <button class="reveal shown" title="Remove (moves its file to the Trash)" onclick={() => perf.removeItem(i)} disabled={perf.busyItem === i.id}><Trash size={14} /></button>
    {:else}
      <button class="settings" onclick={() => api.perf.openLoginItemsSettings()} title="Apps register these themselves; only System Settings can switch them off">
        {i.enabled ? "On" : "Off"} · Settings <ExternalLink size={11} />
      </button>
    {/if}
  </li>
{/snippet}

<div class="manager">
  <header data-tauri-drag-region>
    <button class="back" onclick={() => (perf.view = "summary")}><ChevronLeft size={16} /> Back</button>
    <h2 data-tauri-drag-region>Performance Manager</h2>
    <div class="tools"></div>
  </header>

  <div class="body">
    <nav aria-label="Sections">
      <h4>Performance</h4>
      {#each SECTIONS as s (s.id)}
        {@const n = count(s.id)}
        <div class="group" class:active={s.id === perf.section} role="button" tabindex="0" onclick={() => (perf.section = s.id)} onkeydown={(e) => e.key === "Enter" && (perf.section = s.id)}>
          <s.icon size={16} />
          <span class="gname">{s.title}</span>
          {#if n !== null}<span class="pill tabular">{n}</span>{/if}
        </div>
      {/each}
    </nav>

    <section class="list">
      <div class="ghead">
        <h3>{meta.title}</h3>
        <p>{meta.blurb}</p>
        {#if perf.section === "tasks"}
          <div class="controls">
            <span>Select:</span>
            <button onclick={() => { perf.chosen.clear(); for (const t of perf.recommended) perf.chosen.add(t.id); }}>Recommended</button>
            <button onclick={() => perf.chosen.clear()}>None</button>
          </div>
        {:else if perf.section === "login" || perf.section === "background"}
          <div class="controls">
            {#if perf.fullListAt}
              <span class="full"><Lock size={11} /> Full list from System Settings, loaded {new Date(perf.fullListAt).toLocaleTimeString([], { timeStyle: "short" })}</span>
            {:else}
              <button
                onclick={() => perf.loadFullList()}
                disabled={perf.loadingFull}
                title="Adds developer names and items you've switched off in System Settings. macOS needs your password for this."
              >
                <Lock size={11} /> {perf.loadingFull ? "Waiting for your password…" : "Full List (asks for password)"}
              </button>
            {/if}
            <button class="push" onclick={() => api.perf.openLoginItemsSettings()}>Open Login Items in System Settings <ExternalLink size={11} /></button>
          </div>
        {:else if perf.section === "snapshots"}
          <div class="controls">
            <span>{r.purgeable ? `About ${bytes(r.purgeable)} is held as purgeable space.` : ""}</span>
          </div>
        {/if}
      </div>

      <ul>
        {#if perf.section === "tasks"}
          {#each r.tasks as t (t.id)}
            {@const st = perf.taskState.get(t.id)}
            {@const res = perf.taskResult.get(t.id)}
            <li class="task">
              <Checkbox state={perf.chosen.has(t.id)} onchange={() => (perf.chosen.has(t.id) ? perf.chosen.delete(t.id) : perf.chosen.add(t.id))} label="Select {t.title}" disabled={perf.running} />
              <div class="text">
                <span class="name">
                  {t.title}
                  {#if t.recommended}<em class="tag">Recommended</em>{/if}
                  {#if t.slow}<em class="tag">Takes a while</em>{/if}
                  {#if t.admin}<span class="lock" title="Asks for your password"><Lock size={11} /></span>{/if}
                </span>
                <span class="detail wrap">{t.description}</span>
                {#if res}
                  <span class="res" class:bad={!res.ok}>{#if res.ok}<CircleCheck size={12} />{:else}<CircleAlert size={12} />{/if} {res.message}</span>
                {:else if t.reason && t.recommended}
                  <span class="res why">{t.reason}</span>
                {/if}
              </div>
              <span class="state">{#if st === "running"}<span class="spin"></span>{/if}</span>
            </li>
          {/each}
        {:else if perf.section === "memory"}
          <li class="memory"><MemoryBar memory={r.memory} light /></li>
          {#each r.processes as p, idx (idx)}
            <li>
              {#if p.appPath}<AppIcon path={p.appPath} size={28} />{:else}<span class="kind"><Settings size={14} /></span>{/if}
              <span class="text">
                <span class="name">{p.name}</span>
                <span class="detail">{plural(p.processes, "process", "processes")}</span>
              </span>
              <span class="size tabular">{bytes(p.bytes)}</span>
            </li>
          {/each}
        {:else if perf.section === "login"}
          {#each perf.loginItems as i (i.id)}{@render itemRow(i)}{:else}<li class="empty">Nothing opens at login.</li>{/each}
        {:else if perf.section === "background"}
          <li class="sub">For you <span>Agents · {agents.length}</span></li>
          {#each agents as i (i.id)}{@render itemRow(i)}{/each}
          <li class="sub">For all users <span>Daemons · {daemons.length}</span></li>
          {#each daemons as i (i.id)}{@render itemRow(i)}{/each}
        {:else}
          {#each [...r.snapshots].reverse() as s (s.id)}
            <li>
              <Checkbox state={perf.pickedSnapshots.has(s.id)} onchange={() => (perf.pickedSnapshots.has(s.id) ? perf.pickedSnapshots.delete(s.id) : perf.pickedSnapshots.add(s.id))} label="Select snapshot {s.id}" />
              <span class="text">
                <span class="name">{s.created ? new Date(s.created * 1000).toLocaleString([], { dateStyle: "medium", timeStyle: "short", timeZone: "UTC" }) : s.id}</span>
                <span class="detail">{s.created ? ago(s.created) : ""}</span>
              </span>
            </li>
          {:else}
            <li class="empty">No local snapshots on this Mac.</li>
          {/each}
        {/if}
      </ul>
    </section>
  </div>

  <footer>
    {#if perf.section === "tasks"}
      <span class="count tabular">{perf.chosen.size ? plural(perf.chosen.size, "task") + " selected" : "No tasks selected"}{[...perf.chosen].some((id) => r.tasks.find((t) => t.id === id)?.admin) ? " · asks for your password once" : ""}</span>
      <button class="action" disabled={!perf.chosen.size || perf.running} onclick={() => perf.runTasks()}>{perf.running ? "Running…" : "Run"}</button>
    {:else if perf.section === "snapshots"}
      <span class="count tabular">{perf.pickedSnapshots.size ? plural(perf.pickedSnapshots.size, "snapshot") + " selected" : `All ${plural(r.snapshots.length, "snapshot")}`}</span>
      <button class="action" disabled={!r.snapshots.length || perf.deletingSnapshots} onclick={() => perf.deleteSnapshots()}>
        {perf.deletingSnapshots ? "Removing…" : perf.pickedSnapshots.size ? "Remove Selected" : "Remove All"}
      </button>
    {:else if perf.section === "memory"}
      <span class="count tabular">{bytes(r.memory.used)} of {bytes(r.memory.total)} in use</span>
      <button class="action" disabled={perf.running} onclick={() => perf.runTasks(["free-ram"])}>{perf.taskState.get("free-ram") === "running" ? "Freeing…" : "Free Up RAM"}</button>
    {:else}
      <span class="count"><TriangleAlert size={13} /> Switching an item off is reversible; Remove moves its file to the Trash.</span>
    {/if}
  </footer>
</div>

<style>
  li.off .name,
  li.off .detail {
    opacity: 0.55;
  }
  li {
    min-height: 50px;
  }
  .text {
    flex: 1;
  }
  .detail.wrap {
    white-space: normal;
  }
  .tag {
    margin-left: 6px;
    font-style: normal;
    vertical-align: 1px;
  }
  .tag.live {
    color: #fff !important;
    background: #3f9b5c !important;
  }
  .tag.bad {
    color: #fff !important;
    background: #d9415e !important;
  }
  .lock {
    display: inline-grid;
    margin-left: 6px;
    color: #a35f00;
    vertical-align: -1px;
  }
  .kind {
    width: 30px !important;
    height: 30px !important;
    background: linear-gradient(160deg, #b8b4c6, #77728a) !important;
  }
  .settings {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--m-muted);
    padding: 4px 8px;
    border-radius: 6px;
  }
  .settings:hover {
    background: #ebe8f1;
  }
  .task {
    align-items: flex-start;
    padding-top: 12px;
    padding-bottom: 12px;
  }
  .task :global(.box) {
    margin-top: 2px;
  }
  .res {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: #2f7d4a;
  }
  .res.bad {
    color: #c0392b;
  }
  .res.why {
    color: var(--m-accent);
  }
  .state {
    width: 20px;
    display: grid;
    place-items: center;
    padding-top: 2px;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid #d4d0de;
    border-top-color: var(--m-accent);
    animation: spin 0.8s linear infinite;
  }
  .memory {
    display: block;
    padding: 14px 10px 18px;
  }
  .memory:hover {
    background: none;
  }
  .sub {
    min-height: 0;
    padding: 14px 10px 4px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--m-muted);
  }
  .sub:hover {
    background: none;
  }
  .sub span {
    margin-left: 6px;
    font-weight: 500;
    text-transform: none;
    letter-spacing: 0;
  }
  .full {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 500;
    color: var(--m-muted);
  }
  .push {
    margin-left: auto;
  }
  footer .count {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
