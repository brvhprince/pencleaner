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
    <button class="action" disabled={!selectedCount} onclick={() => cleanup.requestClean()}>Clean</button>
  </footer>
</div>
