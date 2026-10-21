<script lang="ts">
  import {
    AppWindow,
    BrainCircuit,
    ChevronLeft,
    File,
    Folder,
    FolderGit2,
    ScanSearch,
    Search,
    Smartphone,
    Trash,
    TriangleAlert,
  } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { api, type CleanupItem, type ItemKind } from "../../lib/api";
  import Checkbox from "../../lib/components/Checkbox.svelte";
  import { ago, bytes, plural } from "../../lib/format";
  import { CATEGORIES, cleanup } from "./clean.svelte";

  const ICONS: Record<ItemKind, Component> = {
    folder: Folder,
    file: File,
    app: AppWindow,
    project: FolderGit2,
    model: BrainCircuit,
    device: Smartphone,
    trash: Trash,
  };

  let query = $state("");
  let sort = $state<"size" | "name" | "age">("size");

  const group = $derived(cleanup.groups.find((g) => g.id === cleanup.focusGroup) ?? cleanup.groups[0]);
  const rows = $derived.by(() => {
    if (!group) return [];
    const q = query.trim().toLowerCase();
    const list = group.items.filter((i) => !q || `${i.name} ${i.detail ?? ""}`.toLowerCase().includes(q));
    const by: Record<typeof sort, (a: CleanupItem, b: CleanupItem) => number> = {
      size: (a, b) => b.size - a.size,
      name: (a, b) => a.name.localeCompare(b.name),
      age: (a, b) => (a.lastActivity ?? Infinity) - (b.lastActivity ?? Infinity),
    };
    return [...list].sort(by[sort]);
  });
  const selectedCount = $derived(cleanup.items.filter((i) => cleanup.selected.has(i.id)).length);

  function selectRows(on: boolean) {
    for (const i of rows) {
      if (on) cleanup.selected.add(i.id);
      else cleanup.selected.delete(i.id);
    }
  }
</script>

<div class="manager">
  <header data-tauri-drag-region>
    <button class="back" onclick={() => (cleanup.view = "summary")}><ChevronLeft size={16} /> Back</button>
    <h2 data-tauri-drag-region>Cleanup Manager</h2>
    <div class="tools">
      <label class="search">
        <Search size={13} />
        <input placeholder="Search" bind:value={query} spellcheck="false" />
      </label>
      <label class="sort">
        Sort by
        <select bind:value={sort}>
          <option value="size">Size</option>
          <option value="name">Name</option>
          <option value="age">Last used</option>
        </select>
      </label>
    </div>
  </header>

  <div class="body">
    <nav aria-label="Groups">
      {#each CATEGORIES as c (c.id)}
        {@const gs = cleanup.groupsOf(c.id)}
        {#if gs.length}
          <h4>{c.title}</h4>
          {#each gs as g (g.id)}
            <div
              class="group"
              class:active={g.id === group?.id}
              role="button"
              tabindex="0"
              onclick={() => (cleanup.focusGroup = g.id)}
              onkeydown={(e) => e.key === "Enter" && (cleanup.focusGroup = g.id)}
            >
              <Checkbox state={cleanup.groupState(g)} onchange={() => cleanup.toggleGroup(g)} label="Select all of {g.title}" disabled={!g.items.length} />
              <span class="gname">{g.title}</span>
              <span class="pill tabular" class:on={cleanup.groupSelected(g) > 0}>{bytes(cleanup.groupSelected(g) || g.size)}</span>
            </div>
          {/each}
        {/if}
      {/each}
    </nav>

    <section class="list">
      {#if group}
        <div class="ghead">
          <h3>{group.title}</h3>
          <p>{group.description}</p>
          {#if group.note}<p class="note"><TriangleAlert size={13} /> {group.note}</p>{/if}
          <div class="controls">
            <span>Select:</span>
            <button onclick={() => selectRows(true)}>All</button>
            <button onclick={() => selectRows(false)}>None</button>
            <button onclick={() => cleanup.applyRecommendations(group)}>Recommended</button>
            {#if group.id === "projects"}
              <label class="stale">
                Recommend projects untouched for
                <select value={cleanup.staleDays} onchange={(e) => cleanup.setStaleDays(+e.currentTarget.value)}>
                  {#each [7, 14, 30, 60, 90, 180, 365] as d (d)}<option value={d}>{d < 365 ? `${d} days` : "a year"}</option>{/each}
                </select>
              </label>
            {/if}
          </div>
        </div>

        <ul>
          {#each rows as item (item.id)}
            {@const Icon = ICONS[item.kind]}
            <li class:checked={cleanup.selected.has(item.id)}>
              <Checkbox state={cleanup.selected.has(item.id)} onchange={() => cleanup.toggle(item.id)} label="Select {item.name}" />
              <div class="row" role="button" tabindex="0" onclick={() => cleanup.toggle(item.id)} onkeydown={(e) => e.key === " " && (e.preventDefault(), cleanup.toggle(item.id))}>
                <span class="kind {item.kind}"><Icon size={15} /></span>
                <span class="text">
                  <span class="name">{item.name}</span>
                  {#if item.detail}<span class="detail" title={item.detail}>{item.detail}</span>{/if}
                </span>
                {#if item.warning}<span class="warn" title={item.warning}><TriangleAlert size={14} /></span>{/if}
                {#if item.mode === "trash"}<span class="tag" title="Moved to the Trash instead of deleted">Trash</span>{/if}
                <span class="age">{item.lastActivity ? ago(item.lastActivity) : ""}</span>
                <span class="size tabular">{bytes(item.size)}</span>
              </div>
              {#if item.path}
                <button class="reveal" title="Show in Finder" onclick={() => api.revealInFinder(item.path!)}><ScanSearch size={14} /></button>
              {/if}
            </li>
          {:else}
            <li class="empty">Nothing matches “{query}”.</li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>

  <footer>
    <span class="count tabular">{selectedCount ? plural(selectedCount, "item") + " selected" : "No items selected"} <i>|</i> {bytes(cleanup.selectedSize)}</span>
    <button class="clean" disabled={!selectedCount} onclick={() => cleanup.requestClean()}>Clean</button>
  </footer>
</div>

<style>
  .manager {
    --m-bg: #f8f7fb;
    --m-panel: #ffffff;
    --m-text: #1f1c29;
    --m-muted: #6f6a7e;
    --m-line: #ebe8f1;
    --m-accent: color-mix(in srgb, var(--accent) 74%, black);
    --check-border: #bdb7cb;
    --check-on: var(--m-accent);
    position: absolute;
    inset: 12px 14px 14px 10px;
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    background: var(--m-bg);
    color: var(--m-text);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }
  header {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    height: 50px;
    padding: 0 14px;
    border-bottom: 1px solid var(--m-line);
    background: var(--m-panel);
    flex: none;
  }
  h2 {
    font-size: 13.5px;
    font-weight: 600;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-weight: 500;
    color: var(--m-accent);
    justify-self: start;
  }
  .tools {
    justify-self: end;
    display: flex;
    gap: 12px;
    align-items: center;
    color: var(--m-muted);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 9px;
    border-radius: 8px;
    background: #f0eef5;
  }
  .search input {
    width: 130px;
    border: 0;
    outline: 0;
    background: none;
    font: inherit;
    color: var(--m-text);
  }
  .sort {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
  }
  select {
    font: inherit;
    font-weight: 600;
    color: var(--m-accent);
    border: 0;
    background: none;
    outline: 0;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 268px minmax(0, 1fr);
  }
  nav {
    overflow-y: auto;
    padding: 12px 10px;
    border-right: 1px solid var(--m-line);
  }
  h4 {
    margin: 12px 8px 6px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--m-muted);
  }
  h4:first-child {
    margin-top: 2px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 36px;
    padding: 0 8px;
    border-radius: 9px;
    font-weight: 500;
  }
  .group:hover {
    background: #efedf4;
  }
  .group.active {
    background: color-mix(in srgb, var(--m-accent) 13%, white);
  }
  .gname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pill {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 6px;
    color: var(--m-muted);
    background: #ebe8f1;
  }
  .pill.on {
    color: #fff;
    background: var(--m-accent);
  }
  .list {
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--m-panel);
  }
  .ghead {
    padding: 18px 22px 10px;
    border-bottom: 1px solid var(--m-line);
  }
  h3 {
    font-size: 16px;
    font-weight: 600;
  }
  .ghead p {
    margin-top: 4px;
    color: var(--m-muted);
    line-height: 1.4;
    max-width: 560px;
  }
  .ghead .note {
    display: flex;
    gap: 5px;
    align-items: center;
    color: #a35f00;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 12px;
    font-weight: 600;
    font-size: 12px;
  }
  .controls button {
    padding: 3px 8px;
    border-radius: 6px;
    color: var(--m-accent);
  }
  .controls button:hover {
    background: #f0eef5;
  }
  .stale {
    margin-left: auto;
    font-weight: 500;
    color: var(--m-muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 6px 12px 12px;
    overflow-y: auto;
    flex: 1;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px 0 10px;
    border-radius: 9px;
  }
  li:hover {
    background: #f6f5f9;
  }
  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 11px;
    min-height: 50px;
    outline-offset: -2px;
  }
  .kind {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    flex: none;
    color: #fff;
    background: linear-gradient(160deg, #7cc0f7, #3f8fde);
  }
  .kind.project {
    background: linear-gradient(160deg, #9b8cff, #6247e0);
  }
  .kind.model {
    background: linear-gradient(160deg, #f59ad8, #c43f9b);
  }
  .kind.device {
    background: linear-gradient(160deg, #9aa6b8, #5d6a7e);
  }
  .kind.trash {
    background: linear-gradient(160deg, #a5b0bd, #707d8d);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail {
    font-size: 11.5px;
    color: var(--m-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .warn {
    color: #d48806;
    display: grid;
  }
  .tag {
    font-size: 10.5px;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 5px;
    background: #eeeaf6;
    color: var(--m-muted);
  }
  .age {
    width: 92px;
    text-align: right;
    font-size: 11.5px;
    color: var(--m-muted);
  }
  .size {
    width: 72px;
    text-align: right;
    font-weight: 600;
    font-size: 12px;
  }
  .reveal {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    color: var(--m-muted);
    opacity: 0;
  }
  li:hover .reveal {
    opacity: 1;
  }
  .reveal:hover {
    background: #ebe8f1;
  }
  .empty {
    padding: 24px 10px;
    color: var(--m-muted);
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    height: 56px;
    border-top: 1px solid var(--m-line);
    background: var(--m-panel);
    flex: none;
  }
  .count {
    font-weight: 600;
  }
  .count i {
    font-style: normal;
    color: #c9c4d6;
    margin: 0 8px;
  }
  .clean {
    position: absolute;
    right: 14px;
    height: 34px;
    padding: 0 22px;
    border-radius: 9px;
    font-weight: 600;
    color: #fff;
    background: var(--m-accent);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--m-accent) 40%, transparent);
  }
  .clean:disabled {
    opacity: 0.4;
    box-shadow: none;
  }
</style>
