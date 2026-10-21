<script lang="ts">
  import { Check, CircleAlert } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import Illustration from "../../lib/illustrations/Illustration.svelte";
  import { bytes } from "../../lib/format";
  import { cleanup } from "./clean.svelte";
</script>

<section class="cleaning">
  <div class="drag" data-tauri-drag-region></div>
  <div class="card glass">
    <Illustration name="housekeeping" size={200} hero active />
    <div class="side">
      <h2>Cleaning up…</h2>
      <p class="muted tabular">{cleanup.done} of {cleanup.total}</p>
      <div class="bar"><div style:width="{cleanup.total ? (cleanup.done / cleanup.total) * 100 : 0}%"></div></div>
      <ul>
        {#each cleanup.log as row (row.id)}
          {@const s = cleanup.status.get(row.id)}
          <li in:fly={{ y: 10, duration: 200 }}>
            <span class="name">{row.name}</span>
            <span class="size tabular">{bytes(row.size)}</span>
            <span class="state">
              {#if s === "done"}<Check size={15} />{:else if s === "failed"}<CircleAlert size={15} class="bad" />{:else}<span class="spin"></span>{/if}
            </span>
          </li>
        {/each}
      </ul>
    </div>
  </div>
</section>

<style>
  .cleaning {
    height: 100%;
    display: grid;
    place-items: center;
    padding: 30px;
  }
  .drag {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--titlebar-h);
  }
  .card {
    width: min(780px, 100%);
    display: grid;
    grid-template-columns: 260px 1fr;
    align-items: center;
    gap: 24px;
    padding: 40px 40px 40px 24px;
    background: rgba(255, 255, 255, 0.1);
  }
  h2 {
    font-size: 28px;
    font-weight: 300;
  }
  .side > p {
    margin-top: 4px;
  }
  .bar {
    height: 4px;
    margin: 14px 0 16px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.15);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: #fff;
    transition: width 240ms ease;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    min-height: 228px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .size {
    color: var(--text-2);
    font-size: 12px;
  }
  .state {
    display: grid;
    place-items: center;
    width: 20px;
  }
  .state :global(.bad) {
    color: #ffb4b4;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #fff;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
