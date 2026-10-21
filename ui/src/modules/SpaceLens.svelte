<script lang="ts">
  import { ChevronLeft, ChevronRight, RotateCcw } from "@lucide/svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Scanning from "../lib/components/Scanning.svelte";
  import TargetPicker from "../lib/components/TargetPicker.svelte";
  import { bytes, count, plural } from "../lib/format";
  import { MODULE } from "../lib/modules";
  import { app } from "../lib/state.svelte";
  import Bubbles from "./spacelens/Bubbles.svelte";
  import EntryList from "./spacelens/EntryList.svelte";
  import { lens, type Target } from "./spacelens/lens.svelte";
  import ReviewSheet from "./spacelens/ReviewSheet.svelte";

  const module = MODULE.spacelens;
  let reviewing = $state(false);

  type Option = Target & { key: string; label: string; sub?: string };

  const volumeOptions = $derived<Option[]>(
    app.volumes.map((v) => ({
      key: v.mount,
      label: v.name,
      sub: `${bytes(v.used)} of ${bytes(v.total)} used`,
      name: v.name,
      path: v.mount,
      expected: v.used,
    })),
  );
  const homeOption = $derived<Option | null>(
    app.system
      ? { key: app.system.home, label: app.system.user, sub: "Home folder", name: app.system.user, path: app.system.home, expected: null }
      : null,
  );
  const options = $derived(homeOption ? [...volumeOptions, homeOption] : volumeOptions);

  // Default to the startup disk once volumes load.
  $effect(() => {
    if (!lens.target && volumeOptions[0]) lens.target = volumeOptions[0];
  });

  const pickerValue = $derived(
    lens.target ? (options.find((o) => o.path === lens.target!.path) ?? { ...lens.target, key: lens.target.path, label: lens.target.name }) : null,
  );

  // Browser preview only: #autoscan&open=/Applications&select&review drives the flow
  // so every state can be screenshotted headlessly.
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !lens.target || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await lens.scan();
      if (q.get("open")) await lens.open(q.get("open")!);
      if (q.has("select")) lens.listing?.entries.filter((e) => !e.protected).slice(0, 3).forEach((e) => lens.toggle(e));
      if (q.has("review")) reviewing = true;
    })();
  });

  async function chooseFolder() {
    const picked = await openDialog({ directory: true, multiple: false, title: "Choose a folder to scan" });
    if (typeof picked === "string") {
      lens.target = { name: picked.split("/").filter(Boolean).pop() ?? picked, path: picked, expected: null };
    }
  }

  const ratio = $derived(
    lens.target?.expected && lens.progress ? Math.min(0.99, lens.progress.bytes / lens.target.expected) : null,
  );

  const volume = $derived(
    app.volumes.find((v) => lens.target && (v.mount === lens.target.path || (v.isRoot && !lens.target.path.startsWith("/Volumes/")))) ??
      app.volumes[0],
  );

  const crumbs = $derived.by(() => {
    if (!lens.listing || !lens.target) return [];
    const root = lens.target.path;
    const rel = lens.listing.path.slice(root.length).split("/").filter(Boolean);
    const out = [{ name: lens.target.name, path: root }];
    let acc = root;
    for (const part of rel) {
      acc = acc.endsWith("/") ? acc + part : `${acc}/${part}`;
      out.push({ name: part, path: acc });
    }
    return out;
  });
</script>

{#if lens.phase === "home"}
  <ModuleHome {module}>
    <TargetPicker options={options} value={pickerValue} onpick={(o) => (lens.target = o)} onchoose={chooseFolder} />
  </ModuleHome>
  <ScanButton label="Scan" onclick={() => lens.scan()} disabled={!lens.target} />
{:else if lens.phase === "scanning"}
  <Scanning
    illustration={module.illustration}
    title="Visualizing your storage space…"
    detail={lens.progress?.current ?? ""}
    stats={lens.progress ? `${count(lens.progress.items)} items · ${bytes(lens.progress.bytes)}` : "Starting…"}
  />
  <ScanButton label="Stop" busy progress={ratio} onclick={() => lens.stop()} />
{:else if lens.listing}
  <div class="results">
    <div class="topbar" data-tauri-drag-region>
      <button class="link" onclick={() => lens.startOver()}><RotateCcw size={13} /> Start Over</button>
      <span class="title" data-tauri-drag-region>Space Lens</span>
      {#if lens.summary}
        <span class="meta muted tabular" data-tauri-drag-region>
          Mapped {count(lens.summary.items)} items{lens.summary.denied ? ` · ${plural(lens.summary.denied, "folder")} unreadable` : ""}
        </span>
      {/if}
    </div>

    <nav class="crumbs glass" aria-label="Location">
      <button class="nav" onclick={() => lens.goBack()} disabled={!lens.back.length} aria-label="Back"><ChevronLeft size={16} /></button>
      <button class="nav" onclick={() => lens.goForward()} disabled={!lens.forward.length} aria-label="Forward"><ChevronRight size={16} /></button>
      <div class="trail">
        {#each crumbs as c, i (c.path)}
          {#if i > 0}<ChevronRight size={12} />{/if}
          <button class="crumb" class:current={i === crumbs.length - 1} onclick={() => lens.open(c.path)}>{c.name}</button>
        {/each}
      </div>
    </nav>

    <div class="body">
      <aside><EntryList listing={lens.listing} /></aside>
      <div class="map"><Bubbles listing={lens.listing} /></div>
    </div>

    <footer>
      <div class="volume">
        {#if volume}
          <div class="vol-line"><strong>{volume.name}</strong><span class="muted tabular">{bytes(volume.used)} of {bytes(volume.total)} used</span></div>
          <div class="bar"><div style:width="{(volume.used / volume.total) * 100}%"></div></div>
        {/if}
      </div>
      <div class="selection tabular">
        {lens.selected.size ? plural(lens.selected.size, "item") + " selected" : "No items selected"}
        <span class="sep">|</span>
        {bytes(lens.selectedSize)}
      </div>
      <button class="btn lg accent" disabled={!lens.selected.size} onclick={() => (reviewing = true)}>Review and Remove</button>
    </footer>
  </div>
  {#if reviewing}<ReviewSheet onclose={() => (reviewing = false)} />{/if}
{/if}

<style>
  .results {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 22px 16px 18px;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 16px;
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
  .meta {
    margin-left: auto;
    font-size: 11.5px;
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
  .crumbs {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 40px;
    padding: 0 8px;
    border-radius: 12px;
    flex: none;
  }
  .nav {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.08);
  }
  .nav:disabled {
    opacity: 0.35;
  }
  .trail {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    overflow: hidden;
    color: var(--text-2);
  }
  .crumb {
    padding: 3px 6px;
    border-radius: 6px;
    white-space: nowrap;
    font-weight: 500;
  }
  .crumb:hover {
    background: var(--glass-hover);
    color: var(--text);
  }
  .crumb.current {
    color: var(--text);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: 20px;
    padding-top: 14px;
  }
  aside {
    min-height: 0;
  }
  .map {
    min-width: 0;
    min-height: 0;
    padding: 6px 10px 10px;
  }
  footer {
    display: grid;
    grid-template-columns: 320px 1fr auto;
    align-items: center;
    gap: 20px;
    padding-top: 12px;
    flex: none;
  }
  .vol-line {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    margin-bottom: 6px;
  }
  .bar {
    height: 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.15);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: linear-gradient(90deg, #fff, var(--accent-strong));
  }
  .selection {
    text-align: center;
    font-weight: 600;
  }
  .sep {
    margin: 0 8px;
    color: var(--text-3);
  }
</style>
