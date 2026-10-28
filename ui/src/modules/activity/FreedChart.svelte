<script lang="ts">
  // Space freed per day: one series, so no legend; the card title names it.
  // Mark color validated (dataviz validate_palette.js) against both ends of the
  // My Activity background: #6c88f0 passes lightness band and 3:1 contrast.
  import type { ActivityEvent } from "../../lib/api";
  import { bytes } from "../../lib/format";

  let { events, days = 30 }: { events: ActivityEvent[]; days?: number } = $props();

  const BAR = "#6c88f0";
  const DAY = 86400;
  const H = 150;
  const PAD = { top: 20, right: 6, bottom: 22, left: 46 };

  let w = $state(0);
  let hover = $state<number | null>(null);
  let table = $state(false);

  const start = $derived.by(() => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d.getTime() / 1000 - (days - 1) * DAY;
  });

  const series = $derived.by(() => {
    const out = Array.from({ length: days }, (_, i) => ({ at: start + i * DAY, bytes: 0 }));
    for (const e of events) {
      const i = Math.floor((e.at - start) / DAY);
      if (i >= 0 && i < days) out[i].bytes += e.bytes;
    }
    return out;
  });

  // Clean ticks: 1/2/5 × 10^k gigabytes, at most four including zero.
  const scale = $derived.by(() => {
    const max = Math.max(...series.map((d) => d.bytes), 1);
    const raw = max / 3;
    const pow = 10 ** Math.floor(Math.log10(raw));
    const step = [1, 2, 5, 10].map((m) => m * pow).find((s) => s >= raw) ?? raw;
    const top = Math.ceil(max / step) * step;
    const ticks = Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);
    return { top, ticks };
  });

  const plotW = $derived(Math.max(0, w - PAD.left - PAD.right));
  const slot = $derived(plotW / days);
  const barW = $derived(Math.min(24, Math.max(3, slot - 2)));
  const y = (v: number) => PAD.top + (H - PAD.top - PAD.bottom) * (1 - v / scale.top);
  const peak = $derived(series.reduce((best, d, i) => (d.bytes > series[best].bytes ? i : best), 0));

  /** Column path: square at the baseline, 4px rounded data-end. */
  function column(x: number, top: number, base: number, width: number) {
    const h = base - top;
    const r = Math.min(4, width / 2, h);
    return `M${x},${base}V${top + r}Q${x},${top} ${x + r},${top}H${x + width - r}Q${x + width},${top} ${x + width},${top + r}V${base}Z`;
  }

  const fmtDay = (t: number) => new Date(t * 1000).toLocaleDateString([], { day: "numeric", month: "short" });
</script>

<div class="chart" bind:clientWidth={w}>
  {#if !table}
    <svg width={w} height={H} role="img" aria-label="Space freed per day over the last {days} days">
      {#each scale.ticks as t (t)}
        <line class="grid" x1={PAD.left} x2={w - PAD.right} y1={y(t)} y2={y(t)} />
        <text class="tick" x={PAD.left - 8} y={y(t) + 4} text-anchor="end">{t === 0 ? "0" : bytes(t)}</text>
      {/each}
      {#each series as d, i (d.at)}
        {@const x = PAD.left + i * slot + (slot - barW) / 2}
        {#if d.bytes > 0}
          <path d={column(x, y(d.bytes), y(0), barW)} fill={BAR} opacity={hover === null || hover === i ? 1 : 0.55} />
        {/if}
        {#if i === peak && d.bytes > 0}
          <text class="label" x={x + barW / 2} y={y(d.bytes) - 6} text-anchor="middle">{bytes(d.bytes)}</text>
        {/if}
        <!-- Hit target: the whole slot, taller than the bar. -->
        <rect
          class="hit"
          x={PAD.left + i * slot}
          y={PAD.top}
          width={slot}
          height={H - PAD.top - PAD.bottom}
          role="button"
          tabindex="0"
          aria-label="{fmtDay(d.at)}: {bytes(d.bytes)} freed"
          onpointerenter={() => (hover = i)}
          onpointerleave={() => (hover = null)}
          onfocus={() => (hover = i)}
          onblur={() => (hover = null)}
        />
      {/each}
      {#each [0, Math.floor(days / 2), days - 1] as i (i)}
        <text class="tick" x={PAD.left + i * slot + slot / 2} y={H - 6} text-anchor="middle">{i === days - 1 ? "Today" : fmtDay(series[i].at)}</text>
      {/each}
    </svg>
    {#if hover !== null}
      {@const d = series[hover]}
      <div class="tip" style:left="{Math.min(Math.max(PAD.left + hover * slot + slot / 2, 60), w - 60)}px" style:top="{Math.max(y(d.bytes) - 50, 0)}px">
        <strong class="tabular">{bytes(d.bytes)}</strong>
        <span>{fmtDay(d.at)}</span>
      </div>
    {/if}
  {:else}
    <table>
      <thead><tr><th>Day</th><th>Space freed</th></tr></thead>
      <tbody>
        {#each series.filter((d) => d.bytes > 0).reverse() as d (d.at)}
          <tr><td>{fmtDay(d.at)}</td><td class="tabular">{bytes(d.bytes)}</td></tr>
        {:else}
          <tr><td colspan="2">Nothing freed in the last {days} days.</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
  <button class="toggle" onclick={() => (table = !table)}>{table ? "Show as chart" : "Show as table"}</button>
</div>

<style>
  .chart {
    position: relative;
    width: 100%;
  }
  svg {
    display: block;
    overflow: visible;
  }
  .grid {
    stroke: rgba(255, 255, 255, 0.1);
    stroke-width: 1;
  }
  .tick {
    fill: var(--text-3);
    font-size: 10.5px;
  }
  .label {
    fill: var(--text);
    font-size: 11px;
    font-weight: 600;
  }
  .hit {
    fill: transparent;
    outline: none;
  }
  .hit:focus-visible {
    stroke: var(--text-2);
    stroke-width: 1;
  }
  path {
    transition: opacity 120ms ease;
  }
  .tip {
    position: absolute;
    transform: translateX(-50%);
    pointer-events: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 6px 10px;
    border-radius: 8px;
    background: rgba(14, 12, 30, 0.92);
    border: 1px solid rgba(255, 255, 255, 0.14);
    font-size: 11px;
    white-space: nowrap;
  }
  .tip strong {
    font-size: 13px;
    color: var(--text);
  }
  .tip span {
    color: var(--text-2);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    text-align: left;
    padding: 5px 4px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }
  th {
    color: var(--text-3);
    font-weight: 600;
  }
  .toggle {
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--text-2);
    text-decoration: underline;
  }
</style>
