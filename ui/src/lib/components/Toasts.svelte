<script lang="ts">
  import { fly } from "svelte/transition";
  import { app } from "../state.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.tone}" transition:fly={{ y: -12, duration: 220 }}>{t.message}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    top: 14px;
    right: 16px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    max-width: 340px;
    padding: 10px 14px;
    border-radius: 11px;
    font-size: 12.5px;
    line-height: 1.4;
    background: rgba(24, 14, 44, 0.86);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: var(--shadow);
    backdrop-filter: blur(20px);
  }
  .success {
    border-color: rgba(120, 230, 160, 0.45);
  }
  .error {
    border-color: rgba(255, 120, 120, 0.55);
  }
</style>
