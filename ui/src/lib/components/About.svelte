<script lang="ts">
  import { ExternalLink, X } from "@lucide/svelte";
  import { fade, scale } from "svelte/transition";
  import { api, type AboutInfo } from "../api";

  let { onclose }: { onclose: () => void } = $props();

  const REPO = "https://github.com/brvhprince/pencleaner";
  const COPYRIGHT = `© ${new Date().getFullYear()} Prince Takyi Akomea`;

  const CREDITS: { name: string; what: string; url: string }[] = [
    { name: "Apple XProtect", what: "malware signatures, already on your Mac", url: "https://support.apple.com/guide/security/protecting-against-malware-sec469d47bd8/web" },
    { name: "yara-x", what: "runs those signatures (BSD-3-Clause)", url: "https://github.com/VirusTotal/yara-x" },
    { name: "Homebrew", what: "app catalog for the Updater", url: "https://formulae.brew.sh" },
    { name: "Tauri", what: "app shell", url: "https://tauri.app" },
    { name: "Svelte", what: "interface", url: "https://svelte.dev" },
    { name: "Lucide", what: "icons (ISC)", url: "https://lucide.dev" },
  ];

  let info = $state<AboutInfo | null>(null);
  $effect(() => {
    api.about().then((i) => (info = i));
  });

  const built = $derived(info ? new Date(info.builtAt * 1000).toLocaleDateString([], { dateStyle: "medium" }) : "");
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 150 }} onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="sheet glass" role="dialog" aria-modal="true" aria-labelledby="about-title" transition:scale={{ start: 0.96, duration: 220 }}>
    <button class="close" onclick={onclose} aria-label="Close"><X size={15} /></button>

    <img class="icon" src="/app-icon.png" alt="" width="76" height="76" />
    <h2 id="about-title">PenCleaner</h2>
    <p class="tagline">Keep your Mac clean, fast and roomy.</p>
    {#if info}
      <p class="version tabular">
        Version {info.version} <span title="Built from commit {info.commit}{info.commit.endsWith('+') ? ' with uncommitted changes' : ''}">({info.commit})</span> · {built}
      </p>
    {/if}

    {#if info}
      <dl class="facts">
        <div><dt>macOS</dt><dd>{info.macos} · {info.arch}</dd></div>
        <div><dt>Full Disk Access</dt><dd class:off={!info.fullDiskAccess}>{info.fullDiskAccess ? "On" : "Off"}</dd></div>
        <div><dt>XProtect signatures</dt><dd>{info.xprotect ? `v${info.xprotect}` : "Not found"}</dd></div>
      </dl>
    {/if}

    <div class="credits">
      <h3>Built with</h3>
      <ul>
        {#each CREDITS as c (c.name)}
          <li><button class="credit" title={c.what} onclick={() => api.openUrl(c.url)}>{c.name}</button></li>
        {/each}
      </ul>
    </div>

    <p class="privacy">
      Everything runs on this Mac. It goes online only to check for app updates (App Store, the apps' own feeds,
      Homebrew's catalog) and, during Protection scans, to ask Apple whether programs are notarized.
    </p>

    <div class="foot">
      <span class="muted">{COPYRIGHT}</span>
      <button class="btn" onclick={() => api.openUrl(REPO)}>GitHub <ExternalLink size={12} /></button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: grid;
    place-items: center;
    background: rgba(8, 4, 20, 0.45);
  }
  .sheet {
    position: relative;
    width: 440px;
    max-height: 88vh;
    overflow-y: auto;
    padding: 24px 28px 16px;
    text-align: center;
    background: color-mix(in srgb, var(--surface) 55%, rgba(16, 10, 34, 0.94));
    box-shadow: var(--shadow);
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: var(--text-2);
  }
  .close:hover {
    background: var(--glass-hover);
    color: var(--text);
  }
  .icon {
    filter: drop-shadow(0 10px 24px rgba(0, 0, 0, 0.35));
  }
  h2 {
    margin-top: 10px;
    font-size: 24px;
    font-weight: 600;
  }
  .tagline {
    margin-top: 4px;
    color: var(--text-2);
  }
  .version {
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-3);
  }
  .facts {
    margin: 14px 0 0;
    padding: 10px 14px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.06);
    text-align: left;
    font-size: 12px;
  }
  .facts div {
    display: flex;
    justify-content: space-between;
    padding: 3px 0;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
    font-weight: 600;
  }
  dd.off {
    color: #ffd08a;
  }
  .credits {
    margin-top: 14px;
    text-align: left;
  }
  h3 {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
    margin-bottom: 6px;
  }
  .credits ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 12px;
  }
  .credit {
    font-weight: 600;
    text-decoration: underline;
    text-decoration-color: rgba(255, 255, 255, 0.3);
    text-underline-offset: 2px;
  }
  .credit:hover {
    text-decoration-color: currentColor;
  }
  .privacy {
    margin-top: 12px;
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--text-2);
    text-align: left;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    font-size: 12px;
  }
</style>
