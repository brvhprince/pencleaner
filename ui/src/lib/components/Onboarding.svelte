<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { api } from "../api";
  import Illustration from "../illustrations/Illustration.svelte";
  import { app } from "../state.svelte";

  const open = $derived(!!app.system && !app.system.fullDiskAccess && !app.onboardingDismissed);
  let checking = $state(false);

  async function recheck() {
    checking = true;
    await app.refresh();
    checking = false;
    if (app.system?.fullDiskAccess) app.toast("Full Disk Access is on. Thanks!", "success");
    else app.toast("Still no access. macOS may need PenCleaner to quit and reopen first.");
  }

  // Coming back from System Settings is the usual moment access changes.
  $effect(() => {
    if (!open) return;
    const onFocus = () => app.refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });
</script>

{#if open}
  <div class="backdrop" transition:fade={{ duration: 180 }}>
    <div class="sheet glass" role="dialog" aria-modal="true" aria-labelledby="fda-title" transition:scale={{ start: 0.96, duration: 240 }}>
      <Illustration name="permissions" size={130} hero />
      <h2 id="fda-title">Let PenCleaner see the whole disk</h2>
      <p>
        macOS hides Mail, Safari, app containers and other folders from apps unless you grant
        <strong>Full Disk Access</strong>. Without it, scans miss junk and Space Lens under-reports what's using
        your storage.
      </p>
      <ol>
        <li>Open <em>System Settings → Privacy &amp; Security → Full Disk Access</em>.</li>
        <li>
          Turn on <strong>{app.system?.devBuild ? "your terminal or IDE" : "PenCleaner"}</strong>{#if app.system?.devBuild}
            &nbsp;(dev builds run inside it){/if}. Use + to add it if it isn't listed.
        </li>
        <li>Come back here. macOS may ask to reopen the app.</li>
      </ol>
      <div class="actions">
        <button class="btn" onclick={() => (app.onboardingDismissed = true)}>Not Now</button>
        <button class="btn" onclick={recheck} disabled={checking}>Check Again</button>
        <button class="btn primary" onclick={() => api.openFullDiskAccessSettings()}>Open System Settings</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgba(10, 4, 24, 0.45);
  }
  .sheet {
    width: 500px;
    padding: 30px 34px 26px;
    text-align: center;
    background: color-mix(in srgb, var(--surface) 70%, rgba(20, 10, 40, 0.9));
    box-shadow: var(--shadow);
  }
  h2 {
    margin: 18px 0 10px;
    font-size: 20px;
    font-weight: 600;
  }
  p {
    color: var(--text-2);
    line-height: 1.5;
  }
  ol {
    text-align: left;
    margin: 18px 0 0;
    padding-left: 20px;
    line-height: 1.55;
    color: var(--text-2);
  }
  li + li {
    margin-top: 6px;
  }
  em {
    font-style: normal;
    color: var(--text);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 24px;
  }
</style>
