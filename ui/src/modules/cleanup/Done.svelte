<script lang="ts">
  import { CircleAlert } from "@lucide/svelte";
  import Illustration from "../../lib/illustrations/Illustration.svelte";
  import { bytes, plural } from "../../lib/format";
  import { CATEGORIES, cleanup } from "./clean.svelte";

  const result = $derived(cleanup.result);
  const parts = $derived(CATEGORIES.filter((c) => result?.byCategory[c.id]).map((c) => ({ ...c, bytes: result!.byCategory[c.id]! })));
</script>

{#if result}
  <section class="done">
    <div class="drag" data-tauri-drag-region></div>
    <Illustration name="trash" size={170} hero />
    <p class="eyebrow">All done</p>
    <h1 class="tabular">{bytes(result.bytesFreed)}</h1>
    <p class="muted">cleaned from {plural(result.cleaned.length, "item")}.</p>

    {#if parts.length}
      <ul class="parts">
        {#each parts as p (p.id)}
          <li class="glass"><p.icon size={15} /><span>{p.title}</span><strong class="tabular">{bytes(p.bytes)}</strong></li>
        {/each}
      </ul>
    {/if}

    {#if result.failed.length}
      <div class="failed glass">
        <p><CircleAlert size={14} /> {plural(result.failed.length, "item")} couldn't be cleaned</p>
        <ul>
          {#each result.failed.slice(0, 4) as [id, err] (id)}
            <li title={err}><span>{cleanup.items.find((i) => i.id === id)?.name ?? id}</span><em>{err}</em></li>
          {/each}
        </ul>
      </div>
    {/if}
    {#if cleanup.trashed}
      <p class="muted trashed">{bytes(cleanup.trashed)} of that went to the Trash. Empty it to get the space back.</p>
    {/if}

    <button class="btn lg primary" onclick={() => cleanup.finish()}>{cleanup.items.length ? "Back to Results" : "Done"}</button>
  </section>
{/if}

<style>
  .done {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 20px 40px 40px;
    text-align: center;
  }
  .drag {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--titlebar-h);
  }
  .eyebrow {
    margin-top: 18px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  h1 {
    font-size: 58px;
    font-weight: 200;
    margin: 2px 0;
  }
  .parts {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    padding: 0;
    margin: 20px 0 0;
  }
  .parts li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 10px;
  }
  .parts strong {
    font-weight: 600;
  }
  .failed {
    margin-top: 18px;
    padding: 12px 16px;
    max-width: 520px;
    text-align: left;
  }
  .failed p {
    display: flex;
    gap: 6px;
    align-items: center;
    font-weight: 600;
    color: #ffd0d0;
  }
  .failed ul {
    list-style: none;
    padding: 0;
    margin: 8px 0 0;
    font-size: 12px;
  }
  .failed li {
    display: flex;
    gap: 8px;
    padding: 3px 0;
  }
  .failed em {
    color: var(--text-2);
    font-style: normal;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btn {
    margin-top: 26px;
  }
  .trashed {
    margin-top: 14px;
    font-size: 12px;
  }
</style>
