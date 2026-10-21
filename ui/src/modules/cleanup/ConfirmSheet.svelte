<script lang="ts">
  import { TriangleAlert } from "@lucide/svelte";
  import { fade, scale } from "svelte/transition";
  import { bytes } from "../../lib/format";
  import { cleanup } from "./clean.svelte";
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (cleanup.confirming = false)} />

<div class="backdrop" transition:fade={{ duration: 150 }}>
  <div class="sheet glass" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" transition:scale={{ start: 0.97, duration: 200 }}>
    <h2 id="confirm-title"><TriangleAlert size={18} /> A few items need a second look</h2>
    <p class="muted">You selected these along with everything else. Untick any you'd rather keep.</p>
    <ul>
      {#each cleanup.risky as item (item.id)}
        <li>
          <button class="tick" onclick={() => cleanup.selected.delete(item.id)} title="Keep {item.name}">Keep</button>
          <div>
            <strong>{item.name}</strong> <span class="tabular muted">{bytes(item.size)}</span>
            <p>{item.warning}</p>
          </div>
        </li>
      {/each}
    </ul>
    <div class="actions">
      <button class="btn lg" onclick={() => (cleanup.confirming = false)}>Go Back</button>
      <button class="btn lg accent" onclick={() => cleanup.clean()}>Clean {bytes(cleanup.selectedSize)}</button>
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
    background: rgba(5, 10, 6, 0.45);
  }
  .sheet {
    width: 540px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    padding: 24px;
    background: color-mix(in srgb, var(--surface) 60%, rgba(10, 18, 12, 0.94));
    box-shadow: var(--shadow);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 18px;
    font-weight: 600;
  }
  .sheet > p {
    margin: 6px 0 14px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    gap: 12px;
    padding: 10px 2px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  li p {
    margin-top: 3px;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-2);
  }
  .tick {
    align-self: flex-start;
    padding: 3px 9px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--glass-2);
    border: 1px solid var(--glass-border);
  }
  .tick:hover {
    background: var(--glass-hover);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
