<script lang="ts">
  import { Check, CircleAlert, Lock, TriangleAlert } from "@lucide/svelte";
  import { fade, scale } from "svelte/transition";
  import AppIcon from "../../lib/components/AppIcon.svelte";
  import { bytes, plural } from "../../lib/format";
  import { apps } from "./apps.svelte";

  // Frozen when the sheet opens: removed items leave the selection as they go.
  const keys = [...apps.selected];
  const rows = keys.map((k) => ({
    key: k,
    name: apps.nameOf(k),
    size: apps.sizeOf(k),
    app: apps.report?.apps.find((a) => `app:${a.id}` === k) ?? null,
  }));
  const total = rows.reduce((s, r) => s + r.size, 0);
  const running = rows.filter((r) => r.app?.running);
  const needsPassword = rows.some((r) => r.app?.related.some((f) => f.admin));

  function close() {
    apps.sheet = "closed";
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && apps.sheet !== "working" && close()} />

<div class="backdrop" transition:fade={{ duration: 150 }}>
  <div class="sheet glass" role="dialog" aria-modal="true" aria-labelledby="rm-title" transition:scale={{ start: 0.97, duration: 200 }}>
    <h2 id="rm-title">
      {#if apps.sheet === "confirm"}
        {apps.selectedApps.length ? `Uninstall ${plural(apps.selectedApps.length, "app")}?` : `Remove ${plural(rows.length, "item")}?`}
      {:else if apps.sheet === "working"}
        Moving to the Trash…
      {:else}
        Done
      {/if}
    </h2>
    {#if apps.sheet === "confirm"}
      <p class="muted">
        {bytes(total)} goes to the Trash, apps together with their support files, so you can put things back until you empty it.
      </p>
      {#if running.length}
        <p class="warn"><TriangleAlert size={14} /> {running.map((r) => r.name).join(", ")} {running.length === 1 ? "is" : "are"} running and will be skipped. Quit {running.length === 1 ? "it" : "them"} first to uninstall.</p>
      {/if}
      {#if needsPassword}
        <p class="note"><Lock size={13} /> Some items were installed for all users. macOS will ask for your password once.</p>
      {/if}
    {:else if apps.sheet === "done" && apps.result}
      <p class="muted">{plural(apps.result.cleaned.length, "item")} removed, {bytes(apps.result.bytesFreed)} moved to the Trash.</p>
    {/if}

    <ul>
      {#each rows as r (r.key)}
        {@const s = apps.status.get(r.key)}
        <li>
          {#if r.app}<AppIcon path={r.app.path} size={26} />{:else}<span class="dot"></span>{/if}
          <span class="name">{r.name}</span>
          {#if s === "failed"}<span class="err" title={apps.errors.get(r.key)}>{apps.errors.get(r.key)}</span>{/if}
          <span class="size tabular">{bytes(r.size)}</span>
          <span class="state">
            {#if s === "done"}<Check size={15} />{:else if s === "failed"}<CircleAlert size={15} />{:else if s === "working"}<span class="spin"></span>{/if}
          </span>
        </li>
      {/each}
    </ul>

    <div class="actions">
      {#if apps.sheet === "confirm"}
        <button class="btn lg" onclick={close}>Cancel</button>
        <button class="btn lg accent" onclick={() => apps.remove()}>Move to Trash</button>
      {:else if apps.sheet === "done"}
        <button class="btn lg primary" onclick={close}>Done</button>
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
    background: rgba(4, 8, 30, 0.45);
  }
  .sheet {
    width: 540px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    padding: 24px;
    background: color-mix(in srgb, var(--surface) 60%, rgba(8, 12, 40, 0.94));
    box-shadow: var(--shadow);
  }
  h2 {
    font-size: 19px;
    font-weight: 600;
  }
  .sheet > p {
    margin-top: 6px;
    line-height: 1.45;
  }
  .warn,
  .note {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    font-size: 12px;
  }
  .warn {
    color: #ffd08a;
  }
  .note {
    color: var(--text-2);
  }
  ul {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 2px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  .dot {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.14);
    flex: none;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .err {
    max-width: 200px;
    font-size: 11px;
    color: #ffb4b4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .size {
    font-size: 12px;
    color: var(--text-2);
  }
  .state {
    display: grid;
    place-items: center;
    width: 20px;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.25);
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
