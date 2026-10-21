<script lang="ts">
  import { ChevronLeft, ScanSearch, Search } from "@lucide/svelte";
  import { api, fileSrc, type FileInfo, type FileKind, type FileSet } from "../../lib/api";
  import Checkbox from "../../lib/components/Checkbox.svelte";
  import FileIcon from "../../lib/components/FileIcon.svelte";
  import { ago, bytes, plural } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import { SECTIONS, clutter, lastTouched } from "./clutter.svelte";

  const PAGE = 120;
  const YEAR = 365 * 86400;
  const MONTH = 30 * 86400;

  let query = $state("");
  let shown = $state(PAGE);
  let largeFilter = $state<"old" | "all">(clutter.oldLarge.length ? "old" : "all");
  let dlFilter = $state<"all" | "installers" | "archives" | "documents" | "media" | "folders">("all");

  const r = $derived(clutter.report!);
  const meta = $derived(SECTIONS.find((s) => s.id === clutter.section)!);
  const q = $derived(query.trim().toLowerCase());
  const matches = (f: FileInfo) => !q || f.path.toLowerCase().includes(q);
  const now = Date.now() / 1000;

  const DL_KINDS: Record<typeof dlFilter, FileKind[] | null> = {
    all: null,
    installers: ["installer"],
    archives: ["archive"],
    documents: ["document"],
    media: ["image", "video", "audio"],
    folders: ["folder"],
  };

  const large = $derived(
    r.large.filter((f) => matches(f) && (largeFilter === "all" || (lastTouched(f) ?? 0) < now - YEAR)),
  );
  const downloads = $derived(r.downloads.filter((f) => matches(f) && (!DL_KINDS[dlFilter] || DL_KINDS[dlFilter]!.includes(f.kind))));
  const sets = $derived(
    (clutter.section === "similar" ? r.similar : r.duplicates).filter((s) => s.files.some(matches)),
  );
  const selectedCount = $derived(clutter.selected.size);

  function crumbs(path: string) {
    const home = app.system?.home ?? "";
    const rel = path.startsWith(home) ? path.slice(home.length) : path;
    return rel.split("/").filter(Boolean).slice(0, -1).join("  ›  ") || "/";
  }

  function go(id: (typeof SECTIONS)[number]["id"]) {
    clutter.section = id;
    shown = PAGE;
  }

  function selectFiles(files: FileInfo[], on: boolean) {
    if (on) for (const f of files) clutter.selected.add(f.path);
    else clutter.unselect(files.map((f) => f.path));
  }

  function suggestedDownloads() {
    // Installers and archives that have sat around for a month are almost always done with.
    const old = r.downloads.filter((f) => (f.kind === "installer" || f.kind === "archive") && (f.added ?? f.created ?? now) < now - MONTH);
    selectFiles(old, true);
  }

  function setState(s: FileSet): "all" | "some" | "none" {
    const n = s.files.filter((f) => clutter.selected.has(f.path)).length;
    return n === 0 ? "none" : n === s.files.length ? "all" : "some";
  }
</script>

{#snippet fileRow(f: FileInfo, meta: string, badge: string | null = null)}
  <li class:checked={clutter.selected.has(f.path)}>
    <Checkbox state={clutter.selected.has(f.path)} onchange={() => clutter.toggle(f.path)} label="Select {f.name}" />
    <div class="row" role="button" tabindex="0" onclick={() => clutter.toggle(f.path)} onkeydown={(e) => e.key === " " && (e.preventDefault(), clutter.toggle(f.path))}>
      <FileIcon kind={f.kind} />
      <span class="text">
        <span class="name">{f.name}{#if badge}<em class="badge">{badge}</em>{/if}</span>
        <span class="detail" title={f.path}>{crumbs(f.path)}</span>
      </span>
      <span class="age">{meta}</span>
      <span class="size tabular">{bytes(f.size)}</span>
    </div>
    <button class="reveal" title="Show in Finder" onclick={() => api.revealInFinder(f.path)}><ScanSearch size={14} /></button>
  </li>
{/snippet}

<div class="manager">
  <header data-tauri-drag-region>
    <button class="back" onclick={() => (clutter.view = "summary")}><ChevronLeft size={16} /> Back</button>
    <h2 data-tauri-drag-region>My Clutter Manager</h2>
    <div class="tools">
      <label class="search">
        <Search size={13} />
        <input placeholder="Search" bind:value={query} spellcheck="false" />
      </label>
    </div>
  </header>

  <div class="body">
    <nav aria-label="Sections">
      <h4>My Clutter</h4>
      {#each SECTIONS as s (s.id)}
        {@const st = clutter.stats(s.id)}
        <div class="group" class:active={s.id === clutter.section} role="button" tabindex="0" onclick={() => go(s.id)} onkeydown={(e) => e.key === "Enter" && go(s.id)}>
          <s.icon size={16} />
          <span class="gname">{s.title}</span>
          <span class="pill tabular">{st.count}</span>
        </div>
      {/each}
    </nav>

    <section class="list">
      <div class="ghead">
        <h3>{meta.title}</h3>
        <p>{meta.blurb}</p>
        <div class="controls">
          {#if clutter.section === "duplicates" || clutter.section === "similar"}
            <span>Select:</span>
            <button onclick={() => clutter.autoSelect(sets)}>All but the best copy</button>
            <button onclick={() => selectFiles(sets.flatMap((s) => s.files), false)}>None</button>
          {:else if clutter.section === "large"}
            <div class="chips">
              <button class:on={largeFilter === "old"} onclick={() => (largeFilter = "old")}>Not opened in a year</button>
              <button class:on={largeFilter === "all"} onclick={() => (largeFilter = "all")}>All large files</button>
            </div>
            <span class="push">Select:</span>
            <button onclick={() => selectFiles(large, true)}>All</button>
            <button onclick={() => selectFiles(large, false)}>None</button>
          {:else}
            <div class="chips">
              {#each Object.keys(DL_KINDS) as k (k)}
                <button class:on={dlFilter === k} onclick={() => (dlFilter = k as typeof dlFilter)}>{k[0].toUpperCase() + k.slice(1)}</button>
              {/each}
            </div>
            <span class="push">Select:</span>
            <button onclick={suggestedDownloads} title="Installers and archives older than a month">Suggested</button>
            <button onclick={() => selectFiles(downloads, false)}>None</button>
          {/if}
        </div>
      </div>

      <ul>
        {#if clutter.section === "large"}
          {#each large.slice(0, shown) as f (f.path)}
            {@render fileRow(f, f.lastUsed ? `Opened ${ago(f.lastUsed)}` : f.modified ? `Never opened · ${ago(f.modified)}` : "Never opened")}
          {:else}
            <li class="empty">No large files {largeFilter === "old" ? "that went unopened for a year" : ""}.</li>
          {/each}
        {:else if clutter.section === "downloads"}
          {#each downloads.slice(0, shown) as f (f.path)}
            {@render fileRow(f, [f.source, f.added ? `added ${ago(f.added)}` : null].filter(Boolean).join(" · "))}
          {:else}
            <li class="empty">Nothing here.</li>
          {/each}
        {:else}
          {#each sets.slice(0, shown) as s (s.id)}
            <li class="set">
              <div class="sethead">
                <Checkbox state={setState(s)} onchange={() => (setState(s) === "none" ? clutter.autoSelect([s]) : selectFiles(s.files, false))} label="Select copies of {s.files[0].name}" />
                <strong>{s.files[0].name}</strong>
                <span class="muted">{plural(s.files.length, clutter.section === "similar" ? "image" : "copy", clutter.section === "similar" ? "images" : "copies")}</span>
                <span class="saves tabular">{s.reclaimable ? `Saves up to ${bytes(s.reclaimable)}` : "Clones: frees no space"}</span>
              </div>
              {#if clutter.section === "similar"}
                <div class="thumbs">
                  {#each s.files as f (f.path)}
                    <div
                      class="thumb"
                      class:picked={clutter.selected.has(f.path)}
                      role="button"
                      tabindex="0"
                      onclick={() => clutter.toggle(f.path)}
                      ondblclick={() => api.revealInFinder(f.path)}
                      onkeydown={(e) => e.key === " " && (e.preventDefault(), clutter.toggle(f.path))}
                      title="{f.path} (double-click to show in Finder)"
                    >
                      <img src={fileSrc(f.path)} alt={f.name} loading="lazy" />
                      {#if f.path === s.keep}<em class="best">Best</em>{/if}
                      <span class="tick"><Checkbox state={clutter.selected.has(f.path)} onchange={() => clutter.toggle(f.path)} label="Select {f.name}" /></span>
                      <span class="cap"><span>{f.name}</span><span class="tabular">{f.width && f.height ? `${f.width}×${f.height} · ` : ""}{bytes(f.size)}</span></span>
                    </div>
                  {/each}
                </div>
              {:else}
                <ul class="copies">
                  {#each s.files as f (f.path)}
                    {@render fileRow(f, f.modified ? `Modified ${ago(f.modified)}` : "", f.path === s.keep ? "Keep" : null)}
                  {/each}
                </ul>
              {/if}
            </li>
          {:else}
            <li class="empty">No {clutter.section === "similar" ? "similar images" : "duplicates"} found.</li>
          {/each}
        {/if}
        {#if (clutter.section === "large" ? large : clutter.section === "downloads" ? downloads : sets).length > shown}
          <li class="more"><button onclick={() => (shown += PAGE)}>Show more</button></li>
        {/if}
      </ul>
    </section>
  </div>

  <footer>
    <span class="count tabular">
      {#if clutter.removing}
        Moving {clutter.removing.done} of {clutter.removing.total} to the Trash…
      {:else}
        {selectedCount ? plural(selectedCount, "file") + " selected" : "No files selected"} <i>|</i> {bytes(clutter.selectedSize)}
      {/if}
    </span>
    <button class="action" disabled={!selectedCount || !!clutter.removing} onclick={() => clutter.remove()}>Move to Trash</button>
  </footer>
</div>

<style>
  :global(.manager) .row .age {
    width: 210px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .badge {
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 5px;
    font-style: normal;
    font-size: 10.5px;
    font-weight: 600;
    color: #fff;
    background: var(--m-accent);
    vertical-align: 1px;
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
  .set {
    display: block;
    padding: 6px 4px 10px;
    border-bottom: 1px solid var(--m-line);
  }
  .set:hover {
    background: none;
  }
  .sethead {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 4px;
  }
  .sethead strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sethead .muted {
    color: var(--m-muted);
    white-space: nowrap;
  }
  .saves {
    margin-left: auto;
    font-size: 12px;
    font-weight: 600;
    color: var(--m-accent);
    white-space: nowrap;
  }
  .copies {
    padding: 0 0 0 22px;
    overflow: visible;
  }
  .thumbs {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 6px 10px 2px 36px;
  }
  .thumb {
    position: relative;
    width: 150px;
    border-radius: 10px;
    overflow: hidden;
    background: #eeecf3;
    outline: 2px solid transparent;
    transition: outline-color 120ms ease;
  }
  .thumb.picked {
    outline-color: var(--m-accent);
  }
  .thumb img {
    display: block;
    width: 100%;
    height: 104px;
    object-fit: cover;
  }
  .thumb.picked img {
    opacity: 0.7;
  }
  .best {
    position: absolute;
    top: 6px;
    left: 6px;
    padding: 1px 7px;
    border-radius: 5px;
    font-size: 10.5px;
    font-weight: 700;
    font-style: normal;
    color: #fff;
    background: var(--m-accent);
  }
  .tick {
    position: absolute;
    top: 6px;
    right: 6px;
    padding: 2px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.92);
  }
  .cap {
    display: flex;
    flex-direction: column;
    padding: 6px 8px 7px;
    text-align: left;
    font-size: 11px;
    color: var(--m-muted);
  }
  .cap span:first-child {
    color: var(--m-text);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .more {
    justify-content: center;
    padding: 12px;
  }
  .more button {
    font-weight: 600;
    color: var(--m-accent);
  }
</style>
