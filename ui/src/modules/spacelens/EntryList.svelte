<script lang="ts">
  import { ChevronDown, Info, ShieldAlert } from "@lucide/svelte";
  import type { Entry, Listing } from "../../lib/api";
  import { api } from "../../lib/api";
  import EntryIcon from "../../lib/components/EntryIcon.svelte";
  import { bytes, count } from "../../lib/format";
  import { lens } from "./lens.svelte";

  let { listing }: { listing: Listing } = $props();
  let menuOpen = $state(false);

  const anySelectedHere = $derived(listing.entries.some((e) => lens.selected.has(e.path)));

  function openable(e: Entry) {
    return e.kind !== "file" && e.kind !== "symlink" && e.items > 0;
  }

  function onRowClick(e: Entry) {
    if (openable(e)) lens.open(e.path);
  }

  function onKey(ev: KeyboardEvent, e: Entry) {
    if (ev.key === "Enter") onRowClick(e);
    if (ev.key === " ") {
      ev.preventDefault();
      lens.toggle(e);
    }
  }
</script>

<div class="panel glass">
  <header>
    <EntryIcon kind="folder" size={34} />
    <div>
      <h3 title={listing.path}>{listing.name}</h3>
      <p class="muted tabular">{bytes(listing.size)} &nbsp;|&nbsp; {count(listing.items)} items</p>
    </div>
  </header>

  <div class="select">
    <span>Select:</span>
    <button class="menu-btn" onclick={() => (menuOpen = !menuOpen)} aria-haspopup="menu" aria-expanded={menuOpen}>
      {anySelectedHere ? "Some" : "None"}
      <ChevronDown size={13} />
    </button>
    {#if menuOpen}
      <div class="menu glass" role="menu">
        <button role="menuitem" onclick={() => { lens.selectAll(); menuOpen = false; }}>All</button>
        <button role="menuitem" onclick={() => { lens.selectNone(); menuOpen = false; }}>None</button>
      </div>
    {/if}
  </div>

  {#if listing.denied}
    <p class="denied"><ShieldAlert size={14} /> Some of this folder couldn't be read. Grant Full Disk Access for the full picture.</p>
  {/if}

  <ul class="rows">
    {#each listing.entries as e (e.path)}
      {@const covered = lens.covered(e.path)}
      <li
        class="row"
        class:hot={lens.hovered === e.path}
        class:openable={openable(e)}
        onmouseenter={() => (lens.hovered = e.path)}
        onmouseleave={() => (lens.hovered = null)}
      >
        {#if e.protected}
          <span class="info" title="{e.protected}. PenCleaner won't remove it."><Info size={13} /></span>
        {:else}
          <input
            type="checkbox"
            checked={covered || lens.selected.has(e.path)}
            disabled={covered}
            onchange={() => lens.toggle(e)}
            aria-label="Select {e.name}"
          />
        {/if}
        <div
          class="main"
          role="button"
          tabindex="0"
          onclick={() => onRowClick(e)}
          ondblclick={() => !openable(e) && api.revealInFinder(e.path)}
          onkeydown={(ev) => onKey(ev, e)}
          title={e.path}
        >
          <EntryIcon kind={e.kind} size={24} />
          <span class="name" class:dot={e.name.startsWith(".")}>{e.name}</span>
          <span class="size tabular">{bytes(e.size)}</span>
        </div>
      </li>
    {/each}
    {#if listing.restCount > 0}
      <li class="row rest">
        <span class="info"><Info size={13} /></span>
        <div class="main">
          <EntryIcon kind="other" size={24} />
          <span class="name">{listing.restCount.toLocaleString()} more items</span>
          <span class="size tabular">{bytes(listing.restSize)}</span>
        </div>
      </li>
    {/if}
    {#if listing.entries.length === 0}
      <li class="empty muted">This folder is empty.</li>
    {/if}
  </ul>
</div>

<style>
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    padding: 16px 8px 8px 14px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 8px 12px 2px;
  }
  h3 {
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 230px;
  }
  header p {
    margin-top: 3px;
    font-size: 11.5px;
  }
  .select {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 0 8px 2px;
    font-weight: 600;
    font-size: 12px;
  }
  .menu-btn {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--text-2);
  }
  .menu {
    position: absolute;
    top: 22px;
    left: 44px;
    z-index: 4;
    display: flex;
    flex-direction: column;
    padding: 4px;
    min-width: 110px;
    background: rgba(30, 18, 60, 0.95);
  }
  .menu button {
    text-align: left;
    padding: 6px 10px;
    border-radius: 6px;
  }
  .menu button:hover {
    background: var(--glass-hover);
  }
  .denied {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin: 0 8px 8px 2px;
    font-size: 11.5px;
    color: #ffd08a;
    line-height: 1.35;
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0 4px 0 0;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 10px 0 4px;
    border-radius: 9px;
    transition: background 140ms ease;
  }
  .row.hot {
    background: rgba(255, 255, 255, 0.09);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    outline-offset: -2px;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name.dot {
    color: var(--text-2);
  }
  .size {
    font-size: 11.5px;
    color: var(--text-2);
  }
  .info {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.12);
    color: var(--text-2);
    flex: none;
  }
  input[type="checkbox"] {
    appearance: none;
    width: 16px;
    height: 16px;
    margin: 0;
    flex: none;
    border-radius: 4px;
    border: 1.5px solid rgba(255, 255, 255, 0.45);
    display: grid;
    place-items: center;
  }
  input[type="checkbox"]:checked {
    background: var(--accent-strong);
    border-color: var(--accent-strong);
  }
  input[type="checkbox"]:checked::after {
    content: "";
    width: 8px;
    height: 4px;
    border: 2px solid #fff;
    border-top: 0;
    border-right: 0;
    transform: translateY(-1px) rotate(-45deg);
  }
  input[type="checkbox"]:disabled {
    opacity: 0.5;
  }
  .empty {
    padding: 20px 8px;
  }
  .rest .name {
    color: var(--text-2);
  }
</style>
