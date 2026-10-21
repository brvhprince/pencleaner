<script lang="ts">
  import { hierarchy, pack } from "d3-hierarchy";
  import { scale } from "svelte/transition";
  import type { Entry, Listing } from "../../lib/api";
  import EntryIcon from "../../lib/components/EntryIcon.svelte";
  import { bytes } from "../../lib/format";
  import { lens } from "./lens.svelte";

  let { listing }: { listing: Listing } = $props();

  const SHOWN = 11;
  let w = $state(0);
  let h = $state(0);

  interface Leaf {
    entry: Entry | null; // null = "Other items"
    value: number;
  }
  interface Bubble {
    key: string;
    x: number;
    y: number;
    r: number;
    entry: Entry | null;
    size: number;
  }

  const bubbles = $derived.by((): Bubble[] => {
    if (w < 50 || h < 50) return [];
    const top = listing.entries.slice(0, SHOWN).filter((e) => e.size > 0);
    const otherSize =
      listing.entries.slice(SHOWN).reduce((s, e) => s + e.size, 0) + listing.restSize;
    const max = Math.max(top[0]?.size ?? 0, otherSize, 1);
    // A floor keeps small items visible as dots instead of vanishing.
    const floor = max * 0.012;
    const leaves: Leaf[] = top.map((e) => ({ entry: e, value: Math.max(e.size, floor) }));
    if (otherSize > 0) leaves.push({ entry: null, value: Math.max(otherSize, floor) });
    if (!leaves.length) return [];

    const root = hierarchy<{ children?: Leaf[] } | Leaf>({ children: leaves })
      .sum((d) => ("value" in d ? d.value : 0))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    pack<{ children?: Leaf[] } | Leaf>().size([w, h]).padding(12)(root);
    return root.leaves().map((l) => {
      const leaf = l.data as Leaf;
      const p = l as unknown as { x: number; y: number; r: number };
      return {
        key: leaf.entry?.path ?? `${listing.path}::other`,
        x: p.x,
        y: p.y,
        r: p.r,
        entry: leaf.entry,
        size: leaf.entry?.size ?? otherSize,
      };
    });
  });

  function activate(b: Bubble) {
    if (b.entry && (b.entry.kind === "folder" || b.entry.kind === "bundle" || b.entry.kind === "app") && b.entry.items > 0) {
      lens.open(b.entry.path);
    }
  }
</script>

<div class="map" bind:clientWidth={w} bind:clientHeight={h}>
  {#key listing.path}
    {#each bubbles as b, i (b.key)}
      <button
        class="bubble"
        class:hot={lens.hovered === b.key}
        class:picked={b.entry && lens.selected.has(b.entry.path)}
        style:left="{b.x - b.r}px"
        style:top="{b.y - b.r}px"
        style:width="{b.r * 2}px"
        style:height="{b.r * 2}px"
        style:padding="{b.r * 0.22}px"
        onclick={() => activate(b)}
        onmouseenter={() => (lens.hovered = b.key)}
        onmouseleave={() => (lens.hovered = null)}
        title={b.entry ? `${b.entry.name} — ${bytes(b.size)}` : `Other items — ${bytes(b.size)}`}
        in:scale={{ start: 0.6, duration: 380, delay: i * 22 }}
      >
        {#if b.r > 22}
          <EntryIcon kind={b.entry?.kind ?? "other"} size={Math.min(64, b.r * 0.62)} />
        {/if}
        {#if b.r > 42}
          <span class="name" style:font-size="{Math.min(15, 9 + b.r / 18)}px">{b.entry?.name ?? "Other items"}</span>
          <span class="size" style:font-size="{Math.min(14, 8.5 + b.r / 20)}px">{bytes(b.size)}</span>
        {/if}
      </button>
    {/each}
  {/key}
</div>

<style>
  .map {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .bubble {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    border-radius: 50%;
    text-align: center;
    background: radial-gradient(circle at 32% 24%, rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.07) 62%);
    border: 1px solid rgba(255, 255, 255, 0.24);
    box-shadow:
      inset 0 0 28px rgba(255, 255, 255, 0.08),
      0 10px 30px rgba(0, 0, 0, 0.2);
    transition:
      left 450ms var(--ease),
      top 450ms var(--ease),
      width 450ms var(--ease),
      height 450ms var(--ease),
      transform 260ms var(--ease),
      background 200ms ease,
      border-color 200ms ease;
  }
  .bubble:hover,
  .bubble.hot {
    transform: scale(1.035);
    background: radial-gradient(circle at 32% 24%, rgba(255, 255, 255, 0.4), rgba(255, 255, 255, 0.12) 62%);
    border-color: rgba(255, 255, 255, 0.45);
  }
  .bubble.picked {
    border: 2px solid #fff;
    box-shadow:
      0 0 0 4px color-mix(in srgb, var(--accent-strong) 60%, transparent),
      0 10px 30px rgba(0, 0, 0, 0.2);
  }
  .name {
    max-width: 100%;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 4px;
  }
  .size {
    font-weight: 600;
    color: var(--text-2);
  }
</style>
