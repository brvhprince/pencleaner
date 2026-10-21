<script lang="ts">
  import { onMount } from "svelte";
  import { inTauri } from "./lib/api";
  import Onboarding from "./lib/components/Onboarding.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { MODULE, MODULES } from "./lib/modules";
  import { app } from "./lib/state.svelte";
  import Cleanup from "./modules/Cleanup.svelte";
  import ComingSoon from "./modules/ComingSoon.svelte";
  import SpaceLens from "./modules/SpaceLens.svelte";
  import Tools from "./modules/Tools.svelte";

  const current = $derived(MODULE[app.module]);

  onMount(() => {
    // Browser preview only: #module=tools etc. jumps straight to a module.
    const wanted = new URLSearchParams(location.hash.slice(1)).get("module");
    if (!inTauri && wanted && wanted in MODULE) app.go(wanted as keyof typeof MODULE);
    app.refresh().catch((e) => app.toast(`Couldn't read system info: ${e}`, "error"));
  });
</script>

<div
  class="window"
  style:--accent={current.theme.accent}
  style:--accent-strong={current.theme.accentStrong}
  style:--surface={current.theme.surface}
>
  <!-- One layer per module so switching cross-fades instead of snapping. -->
  {#each MODULES as m (m.id)}
    <div class="bg" class:on={m.id === app.module} style:background={m.theme.bg}></div>
  {/each}
  <div class="grain"></div>

  <Sidebar />
  <main>
    {#key app.module}
      <div class="page">
        {#if app.module === "spacelens"}
          <SpaceLens />
        {:else if app.module === "cleanup"}
          <Cleanup />
        {:else if app.module === "tools"}
          <Tools />
        {:else}
          <ComingSoon module={current} />
        {/if}
      </div>
    {/key}
  </main>

  <Onboarding />
  <Toasts />
</div>

<style>
  .window {
    /* Illustration palette follows the module theme; set here, where --surface lives. */
    --empty-background: color-mix(in srgb, var(--surface) 86%, white 14%);
    --empty-primary: var(--accent-strong);
    position: relative;
    display: flex;
    height: 100%;
    overflow: hidden;
  }
  .bg {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 520ms ease;
  }
  .bg.on {
    opacity: 1;
  }
  .grain {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(120% 70% at 50% 120%, rgba(0, 0, 0, 0.28), transparent 60%);
  }
  main {
    position: relative;
    flex: 1;
    min-width: 0;
    z-index: 1;
  }
  .page {
    position: absolute;
    inset: 0;
    animation: enter 360ms var(--ease);
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
