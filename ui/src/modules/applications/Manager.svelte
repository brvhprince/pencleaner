<script lang="ts">
  import { ChevronDown, ChevronLeft, ChevronRight, Lock, PackageX, ScanSearch, Search } from "@lucide/svelte";
  import { api, type AppEntry, type AppTarget, type RelatedFile } from "../../lib/api";
  import AppIcon from "../../lib/components/AppIcon.svelte";
  import Checkbox from "../../lib/components/Checkbox.svelte";
  import FileIcon from "../../lib/components/FileIcon.svelte";
  import { ago, bytes, middleEllipsis, plural } from "../../lib/format";
  import { SECTIONS, apps, isUnused } from "./apps.svelte";

  let query = $state("");
  let filter = $state<"all" | "unused" | "large" | "store">("all");
  let sort = $state<"size" | "name" | "used">("size");

  const r = $derived(apps.report!);
  const meta = $derived(SECTIONS.find((s) => s.id === apps.section)!);
  const q = $derived(query.trim().toLowerCase());

  const appRows = $derived.by(() => {
    const list = r.apps.filter((a) => {
      if (q && !`${a.name} ${a.bundleId}`.toLowerCase().includes(q)) return false;
      if (filter === "unused") return isUnused(a);
      if (filter === "large") return a.size + a.relatedSize > 1e9;
      if (filter === "store") return a.appStore;
      return true;
    });
    const by = {
      size: (a: AppEntry, b: AppEntry) => b.size + b.relatedSize - (a.size + a.relatedSize),
      name: (a: AppEntry, b: AppEntry) => a.name.localeCompare(b.name),
      used: (a: AppEntry, b: AppEntry) => (a.lastUsed ?? 0) - (b.lastUsed ?? 0),
    };
    return list.sort(by[sort]);
  });
  const leftoverRows = $derived(r.leftovers.filter((l) => !q || l.id.includes(q)));
  const installerRows = $derived(r.installers.filter((i) => !q || i.name.toLowerCase().includes(q)));

  const count = (s: (typeof SECTIONS)[number]["id"]) =>
    s === "apps" ? r.apps.length : s === "leftovers" ? r.leftovers.length : r.installers.length;

  const visible = $derived<AppTarget[]>(
    apps.section === "apps"
      ? appRows.map((a) => ({ kind: "app", id: a.id }))
      : apps.section === "leftovers"
        ? leftoverRows.map((l) => ({ kind: "leftover", id: l.id }))
        : installerRows.map((i) => ({ kind: "installer", id: i.path })),
  );

  function toggleOpen(id: string) {
    if (apps.expanded.has(id)) apps.expanded.delete(id);
    else apps.expanded.add(id);
  }

  function labels(files: RelatedFile[]) {
    return [...new Set(files.map((f) => f.label))].join(", ");
  }
</script>

{#snippet files(list: RelatedFile[])}
  <ul class="files">
    {#each list as f (f.path)}
      <li>
        <span class="flabel">{f.label}</span>
        <span class="fpath" title={f.path}>{middleEllipsis(f.path, 80)}</span>
        {#if f.admin}<span class="lock" title="Needs your administrator password"><Lock size={12} /></span>{/if}
        <span class="fsize tabular">{bytes(f.size)}</span>
        <button class="reveal" title="Show in Finder" onclick={() => api.revealInFinder(f.path)}><ScanSearch size={13} /></button>
      </li>
    {/each}
  </ul>
{/snippet}

<div class="manager">
  <header data-tauri-drag-region>
    <button class="back" onclick={() => (apps.view = "summary")}><ChevronLeft size={16} /> Back</button>
    <h2 data-tauri-drag-region>Applications Manager</h2>
    <div class="tools">
      <label class="search">
        <Search size={13} />
        <input placeholder="Search" bind:value={query} spellcheck="false" />
      </label>
      {#if apps.section === "apps"}
        <label class="sort">
          Sort by
          <select bind:value={sort}>
            <option value="size">Size</option>
            <option value="name">Name</option>
            <option value="used">Last opened</option>
          </select>
        </label>
      {/if}
    </div>
  </header>

  <div class="body">
    <nav aria-label="Sections">
      <h4>Applications</h4>
      {#each SECTIONS as s (s.id)}
        <div class="group" class:active={s.id === apps.section} role="button" tabindex="0" onclick={() => (apps.section = s.id)} onkeydown={(e) => e.key === "Enter" && (apps.section = s.id)}>
          <s.icon size={16} />
          <span class="gname">{s.title}</span>
          <span class="pill tabular">{count(s.id)}</span>
        </div>
      {/each}
    </nav>

    <section class="list">
      <div class="ghead">
        <h3>{meta.title}</h3>
        <p>{meta.blurb}</p>
        <div class="controls">
          {#if apps.section === "apps"}
            <div class="chips">
              {#each [["all", "All"], ["unused", "Unused 6+ months"], ["large", "Over 1 GB"], ["store", "App Store"]] as [id, label] (id)}
                <button class:on={filter === id} onclick={() => (filter = id as typeof filter)}>{label}</button>
              {/each}
            </div>
            <span class="push">Select:</span>
            <button onclick={() => apps.select(visible, false)}>None</button>
          {:else}
            <span>Select:</span>
            {#if apps.section === "leftovers"}
              <button onclick={() => apps.select(apps.recommendedLeftovers(), true)} title="Vendors with nothing else installed, untouched for a month">Recommended</button>
            {:else}
              <button onclick={() => apps.select(r.installers.filter((i) => i.recommended).map((i) => ({ kind: "installer", id: i.path })), true)}>Recommended</button>
            {/if}
            <button onclick={() => apps.select(visible, true)}>All</button>
            <button onclick={() => apps.select(visible, false)}>None</button>
          {/if}
        </div>
      </div>

      <ul>
        {#if apps.section === "apps"}
          {#each appRows as a (a.id)}
            {@const t = { kind: "app", id: a.id } as const}
            <li class="block">
              <div class="line">
                <Checkbox state={apps.has(t)} onchange={() => apps.toggle(t)} label="Select {a.name}" />
                <div class="row" role="button" tabindex="0" onclick={() => apps.toggle(t)} onkeydown={(e) => e.key === " " && (e.preventDefault(), apps.toggle(t))}>
                  <AppIcon path={a.path} size={32} />
                  <span class="text">
                    <span class="name">
                      {a.name}
                      {#if a.version}<span class="ver">{a.version}</span>{/if}
                      {#if a.appStore}<em class="tag">App Store</em>{/if}
                      {#if a.running}<em class="tag live">Running</em>{/if}
                    </span>
                    <span class="detail">
                      {a.lastUsed ? `Opened ${ago(a.lastUsed)}` : "Never opened"} ·
                      {#if a.sharesSupportFiles}
                        another copy is installed, so its support files stay
                      {:else}
                        {bytes(a.size)} app + {bytes(a.relatedSize)} in {plural(a.related.length, "support file")}
                      {/if}
                    </span>
                  </span>
                  <span class="size tabular">{bytes(a.size + a.relatedSize)}</span>
                </div>
                <button class="reveal shown" title="Show support files" onclick={() => toggleOpen(a.id)} disabled={!a.related.length}>
                  {#if apps.expanded.has(a.id)}<ChevronDown size={15} />{:else}<ChevronRight size={15} />{/if}
                </button>
              </div>
              {#if apps.expanded.has(a.id)}{@render files(a.related)}{/if}
            </li>
          {:else}
            <li class="empty">No apps match.</li>
          {/each}
        {:else if apps.section === "leftovers"}
          {#each leftoverRows as l (l.id)}
            {@const t = { kind: "leftover", id: l.id } as const}
            <li class="block">
              <div class="line">
                <Checkbox state={apps.has(t)} onchange={() => apps.toggle(t)} label="Select {l.id}" />
                <div class="row" role="button" tabindex="0" onclick={() => apps.toggle(t)} onkeydown={(e) => e.key === " " && (e.preventDefault(), apps.toggle(t))}>
                  <span class="kind"><PackageX size={16} /></span>
                  <span class="text">
                    <span class="name">{l.id}</span>
                    <span class="detail">{labels(l.files)}{l.vendorInstalled ? " · same vendor still has apps installed, check first" : ""}</span>
                  </span>
                  <span class="age">{l.lastActivity ? `Touched ${ago(l.lastActivity)}` : ""}</span>
                  <span class="size tabular">{bytes(l.size)}</span>
                </div>
                <button class="reveal shown" title="Show files" onclick={() => toggleOpen(l.id)}>
                  {#if apps.expanded.has(l.id)}<ChevronDown size={15} />{:else}<ChevronRight size={15} />{/if}
                </button>
              </div>
              {#if apps.expanded.has(l.id)}{@render files(l.files)}{/if}
            </li>
          {:else}
            <li class="empty">No leftovers. Removed apps cleaned up after themselves.</li>
          {/each}
        {:else}
          {#each installerRows as i (i.path)}
            {@const t = { kind: "installer", id: i.path } as const}
            <li>
              <Checkbox state={apps.has(t)} onchange={() => apps.toggle(t)} label="Select {i.name}" />
              <div class="row" role="button" tabindex="0" onclick={() => apps.toggle(t)} onkeydown={(e) => e.key === " " && (e.preventDefault(), apps.toggle(t))}>
                <FileIcon kind="installer" />
                <span class="text">
                  <span class="name">{i.name}</span>
                  <span class="detail">{i.installedApp ? `${i.installedApp} is already installed` : "No matching app installed"}</span>
                </span>
                <span class="age">{i.added ? `Added ${ago(i.added)}` : ""}</span>
                <span class="size tabular">{bytes(i.size)}</span>
              </div>
              <button class="reveal" title="Show in Finder" onclick={() => api.revealInFinder(i.path)}><ScanSearch size={14} /></button>
            </li>
          {:else}
            <li class="empty">No installer files found.</li>
          {/each}
        {/if}
      </ul>
    </section>
  </div>

  <footer>
    <span class="count tabular">
      {apps.selected.size ? plural(apps.selected.size, "item") + " selected" : "No items selected"} <i>|</i> {bytes(apps.selectedSize)}
    </span>
    <button class="action" disabled={!apps.selected.size} onclick={() => (apps.sheet = "confirm")}>
      {apps.selectedApps.length ? "Uninstall" : "Remove"}
    </button>
  </footer>
</div>

<style>
  .block {
    display: block;
    padding: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px 0 10px;
  }
  .ver {
    margin-left: 6px;
    font-weight: 500;
    font-size: 11.5px;
    color: var(--m-muted);
  }
  .tag {
    margin-left: 6px;
    font-style: normal;
    vertical-align: 1px;
  }
  .tag.live {
    color: #fff;
    background: #d48806;
  }
  .kind {
    background: linear-gradient(160deg, #b8b4c6, #77728a) !important;
  }
  .chips {
    display: flex;
    gap: 4px;
  }
  .chips button {
    font-weight: 500;
    color: var(--m-muted);
  }
  .chips button.on {
    color: #fff;
    background: var(--m-accent);
  }
  .push {
    margin-left: auto;
  }
  :global(.manager) .reveal.shown {
    opacity: 1;
  }
  .files {
    list-style: none;
    margin: 0 0 8px 64px;
    padding: 6px 8px;
    border-radius: 8px;
    background: #f6f5f9;
    overflow: visible;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
    font-size: 11.5px;
  }
  .flabel {
    width: 118px;
    flex: none;
    font-weight: 600;
    color: var(--m-text);
  }
  .fpath {
    flex: 1;
    min-width: 0;
    color: var(--m-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lock {
    display: grid;
    color: #a35f00;
  }
  .fsize {
    width: 66px;
    text-align: right;
    font-weight: 600;
  }
  :global(.manager) .row .age {
    width: 150px;
  }
</style>
