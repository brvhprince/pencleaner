<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { inTauri } from "./lib/api";
  import About from "./lib/components/About.svelte";
  import Onboarding from "./lib/components/Onboarding.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { MODULE, MODULES } from "./lib/modules";
  import { app } from "./lib/state.svelte";
  import Applications from "./modules/Applications.svelte";
  import Cleanup from "./modules/Cleanup.svelte";
  import Performance from "./modules/Performance.svelte";
  import Protection from "./modules/Protection.svelte";
  import Clutter from "./modules/Clutter.svelte";
  import Activity from "./modules/Activity.svelte";
  import SettingsPage from "./modules/Settings.svelte";
  import SmartCare from "./modules/SmartCare.svelte";
  import SpaceLens from "./modules/SpaceLens.svelte";
  import Tools from "./modules/Tools.svelte";

  const current = $derived(MODULE[app.module]);
  let showAbout = $state(!inTauri && location.hash.includes("about"));

  // PenCleaner → About PenCleaner in the menu bar.
  $effect(() => {
    if (!inTauri) return;
    const offs = [
      listen("open-about", () => (showAbout = true)),
      listen("open-settings", () => app.go("settings")),
      // From the menu bar popover.
      listen<string>("navigate", (e) => {
        if (e.payload in MODULE) app.go(e.payload as keyof typeof MODULE);
      }),
    ];
    return () => {
      for (const off of offs) off.then((f) => f());
    };
  });

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
        <!-- A render error in one module shows a recoverable card instead of freezing the app. -->
        <svelte:boundary onerror={(e) => console.error(`[${app.module}]`, e)}>
        {#if app.module === "spacelens"}
          <SpaceLens />
        {:else if app.module === "cleanup"}
          <Cleanup />
        {:else if app.module === "clutter"}
          <Clutter />
        {:else if app.module === "applications"}
          <Applications />
        {:else if app.module === "performance"}
          <Performance />
        {:else if app.module === "protection"}
          <Protection />
        {:else if app.module === "tools"}
          <Tools />
        {:else if app.module === "smartcare"}
          <SmartCare />
        {:else if app.module === "activity"}
          <Activity />
        {:else if app.module === "settings"}
          <SettingsPage />
        {/if}
          {#snippet failed(error, reset)}
            <div class="crash glass" role="alert">
              <h2>{current.title} hit a problem</h2>
              <p>{error instanceof Error ? error.message : String(error)}</p>
              <button class="btn lg" onclick={reset}>Try Again</button>
            </div>
          {/snippet}
        </svelte:boundary>
      </div>
    {/key}
  </main>

  <Onboarding />
  {#if showAbout}<About onclose={() => (showAbout = false)} />{/if}
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
  .crash {
    position: absolute;
    inset: 0;
    margin: auto;
    width: 440px;
    height: fit-content;
    padding: 24px;
    text-align: center;
  }
  .crash h2 {
    font-size: 18px;
    font-weight: 600;
  }
  .crash p {
    margin: 8px 0 16px;
    color: var(--text-2);
    font-size: 12px;
    word-break: break-word;
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
