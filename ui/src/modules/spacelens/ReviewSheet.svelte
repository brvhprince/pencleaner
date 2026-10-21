<script lang="ts">
  import { Check, CircleAlert, X } from "@lucide/svelte";
  import { fade, scale } from "svelte/transition";
  import { SvelteMap } from "svelte/reactivity";
  import type { Entry } from "../../lib/api";
  import EntryIcon from "../../lib/components/EntryIcon.svelte";
  import { bytes, middleEllipsis, plural } from "../../lib/format";
  import { lens } from "./lens.svelte";

  let { onclose }: { onclose: () => void } = $props();

  type Status = "pending" | "working" | "done" | "failed";
  let stage = $state<"review" | "removing">("review");
  const status = new SvelteMap<string, Status>();
  // Frozen once removal starts, since removed paths leave the selection.
  let frozen = $state<{ items: Entry[]; size: number } | null>(null);
  const live = $derived([...lens.selected.values()].sort((a, b) => b.size - a.size));
  const items = $derived(frozen?.items ?? live);
  const total = $derived(frozen?.size ?? lens.selectedSize);

  async function remove() {
    frozen = { items: live, size: lens.selectedSize };
    stage = "removing";
    for (const e of items) status.set(e.path, "pending");
    await lens.removeSelected((ev) => {
      if (ev.status === "started") status.set(ev.path, "working");
      else if (ev.status === "removed") status.set(ev.path, "done");
      else status.set(ev.path, "failed");
    });
    onclose();
  }

  function onKey(ev: KeyboardEvent) {
    if (ev.key === "Escape" && stage === "review") onclose();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" transition:fade={{ duration: 160 }}>
  <div class="sheet glass" role="dialog" aria-modal="true" aria-labelledby="review-title" transition:scale={{ start: 0.97, duration: 220 }}>
    <h2 id="review-title">{stage === "review" ? "Review items to remove" : "Moving to the Trash…"}</h2>
    <p class="muted">
      {plural(items.length, "item")}, {bytes(total)}. They go to the Trash, so you can put them back until you
      empty it.
    </p>

    <ul>
      {#each items as e (e.path)}
        {@const s = status.get(e.path)}
        <li>
          <EntryIcon kind={e.kind} size={22} />
          <div class="text">
            <span class="name">{e.name}</span>
            <span class="path">{middleEllipsis(e.path, 64)}</span>
          </div>
          <span class="size tabular">{bytes(e.size)}</span>
          {#if stage === "review"}
            <button class="x" onclick={() => lens.selected.delete(e.path)} aria-label="Keep {e.name}"><X size={14} /></button>
          {:else if s === "done"}
            <span class="state ok"><Check size={15} /></span>
          {:else if s === "failed"}
            <span class="state bad"><CircleAlert size={15} /></span>
          {:else}
            <span class="state spin" class:active={s === "working"}></span>
          {/if}
        </li>
      {/each}
    </ul>

    <div class="actions">
      {#if stage === "review"}
        <button class="btn lg" onclick={onclose}>Cancel</button>
        <button class="btn lg accent" onclick={remove} disabled={items.length === 0}>Move to Trash</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: rgba(8, 4, 20, 0.4);
  }
  .sheet {
    width: 560px;
    max-height: 78vh;
    display: flex;
    flex-direction: column;
    padding: 26px 26px 20px;
    background: color-mix(in srgb, var(--surface) 65%, rgba(18, 10, 40, 0.92));
    box-shadow: var(--shadow);
  }
  h2 {
    font-size: 19px;
    font-weight: 600;
  }
  p {
    margin: 6px 0 16px;
    line-height: 1.45;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    min-height: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 8px 6px;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .path {
    font-size: 11px;
    color: var(--text-3);
    white-space: nowrap;
  }
  .size {
    font-size: 12px;
    color: var(--text-2);
  }
  .x,
  .state {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    flex: none;
  }
  .x:hover {
    background: var(--glass-hover);
  }
  .ok {
    color: #8ff0b0;
  }
  .bad {
    color: #ff9c9c;
  }
  .spin {
    border: 2px solid rgba(255, 255, 255, 0.2);
    width: 16px;
    height: 16px;
  }
  .spin.active {
    border-top-color: #fff;
    animation: spin 0.8s linear infinite;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
