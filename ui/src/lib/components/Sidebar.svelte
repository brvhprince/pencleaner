<script lang="ts">
  import { ShieldAlert } from "@lucide/svelte";
  import { MODULES } from "../modules";
  import { app } from "../state.svelte";

  const primary = MODULES.filter((m) => !m.secondary);
  const secondary = MODULES.filter((m) => m.secondary);
</script>

<nav class="sidebar" class:compact={app.compact} aria-label="Modules">
  <div class="drag" data-tauri-drag-region></div>
  {#snippet item(m: (typeof MODULES)[number])}
    <button
      class="item"
      class:active={app.module === m.id}
      onclick={() => app.go(m.id)}
      title={app.compact ? m.title : undefined}
      aria-current={app.module === m.id ? "page" : undefined}
    >
      <span class="icon"><m.icon size={17} strokeWidth={1.9} /></span>
      <span class="label">{m.title}</span>
      {#if app.phases[m.id] === "scanning"}<span class="busy" aria-label="Scanning"></span>{/if}
    </button>
  {/snippet}

  <div class="group">
    {#each primary as m (m.id)}{@render item(m)}{/each}
  </div>
  <div class="group bottom">
    {#if app.system && !app.system.fullDiskAccess}
      <button class="item warn" onclick={() => (app.onboardingDismissed = false)} title="Full Disk Access is off">
        <span class="icon"><ShieldAlert size={17} /></span>
        <span class="label">Full Disk Access off</span>
      </button>
    {/if}
    {#each secondary as m (m.id)}{@render item(m)}{/each}
  </div>
</nav>

<style>
  .sidebar {
    position: relative;
    z-index: 2;
    display: flex;
    flex-direction: column;
    width: var(--sidebar-w);
    padding: var(--titlebar-h) 14px 18px;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    transition: width 420ms var(--ease), padding 420ms var(--ease);
    flex: none;
  }
  .sidebar.compact {
    width: var(--sidebar-w-compact);
    padding-left: 13px;
    padding-right: 13px;
  }
  .drag {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--titlebar-h);
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .bottom {
    margin-top: auto;
  }
  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    height: 40px;
    padding: 0 12px;
    border-radius: 11px;
    color: rgba(255, 255, 255, 0.86);
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    transition: background 180ms ease;
  }
  .item:hover {
    background: rgba(255, 255, 255, 0.07);
  }
  .item.active {
    background: rgba(255, 255, 255, 0.14);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 22px;
    flex: none;
  }
  .label {
    transition: opacity 240ms ease;
  }
  .compact .label {
    opacity: 0;
  }
  .warn {
    color: #ffd08a;
  }
  .busy {
    position: absolute;
    top: 6px;
    left: 26px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.35);
    border-top-color: #fff;
    animation: spin 0.9s linear infinite;
  }
  .compact .busy {
    left: 30px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
