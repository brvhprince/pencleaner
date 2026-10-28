<script lang="ts">
  import type { MemoryStats } from "../../lib/api";
  import { bytes } from "../../lib/format";

  let { memory, light = false }: { memory: MemoryStats; light?: boolean } = $props();

  const parts = $derived([
    { key: "app", label: "Apps", value: memory.app, color: "#ffb36b" },
    { key: "wired", label: "Wired", value: memory.wired, color: "#ff7a59" },
    { key: "compressed", label: "Compressed", value: memory.compressed, color: "#d9415e" },
    { key: "cached", label: "Cached files", value: memory.cached, color: light ? "#c9c4d6" : "rgba(255,255,255,0.35)" },
  ]);
  const pct = (v: number) => `${Math.max(0, Math.min(100, (v / memory.total) * 100))}%`;
  const swapPct = $derived(memory.swapTotal ? memory.swapUsed / memory.swapTotal : 0);
</script>

<div class="mem" class:light>
  <div class="bar" role="img" aria-label="{bytes(memory.used)} of {bytes(memory.total)} memory used">
    {#each parts as p (p.key)}<span style:width={pct(p.value)} style:background={p.color}></span>{/each}
  </div>
  <ul class="legend">
    {#each parts as p (p.key)}
      <li><i style:background={p.color}></i>{p.label} <strong class="tabular">{bytes(p.value)}</strong></li>
    {/each}
  </ul>
  <div class="swap">
    <span>Swap on disk</span>
    <div class="track"><div style:width="{swapPct * 100}%" class:hot={swapPct > 0.5}></div></div>
    <strong class="tabular">{bytes(memory.swapUsed)}</strong>
  </div>
</div>

<style>
  .bar {
    display: flex;
    height: 14px;
    border-radius: 7px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.12);
  }
  .light .bar {
    background: #ebe8f1;
  }
  .bar span {
    transition: width 600ms var(--ease);
  }
  .legend {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    padding: 0;
    margin: 10px 0 0;
    font-size: 12px;
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
  .light .legend li {
    color: var(--m-muted);
  }
  .legend strong {
    color: var(--text);
    font-weight: 600;
  }
  .light .legend strong {
    color: var(--m-text);
  }
  i {
    width: 9px;
    height: 9px;
    border-radius: 3px;
  }
  .swap {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
    font-size: 12px;
    color: var(--text-2);
  }
  .light .swap {
    color: var(--m-muted);
  }
  .track {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
  }
  .light .track {
    background: #ebe8f1;
  }
  .track div {
    height: 100%;
    background: #ffb36b;
  }
  .track div.hot {
    background: #ff6b6b;
  }
  .swap strong {
    color: var(--text);
  }
  .light .swap strong {
    color: var(--m-text);
  }
</style>
