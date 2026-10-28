<script lang="ts">
  import {
    AppWindow,
    Bot,
    Clock,
    Copy,
    Cpu,
    Download,
    FileClock,
    Hammer,
    Images,
    Mail,
    Orbit,
    Package,
    Power,
    Rocket,
    Search,
    Sparkles,
    Trash,
    Wrench,
    Bug,
    EyeOff,
    KeyRound,
    RefreshCw,
  } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { MODULE, type ModuleId } from "../lib/modules";
  import { app } from "../lib/state.svelte";

  interface Tool {
    title: string;
    text: string;
    icon: Component;
    module: ModuleId;
  }

  const TOOLS: Tool[] = [
    { title: "Space Lens", text: "Map your disk as bubbles and drill into whatever is biggest.", icon: Orbit, module: "spacelens" },
    { title: "System Junk", text: "Caches, logs and temporary files that macOS and apps leave behind.", icon: Sparkles, module: "cleanup" },
    { title: "Developer Junk", text: "node_modules, Rust targets, Gradle and Xcode build output from projects you aren't touching.", icon: Hammer, module: "cleanup" },
    { title: "AI Clutter", text: "Local models and caches from Ollama, LM Studio and Hugging Face.", icon: Bot, module: "cleanup" },
    { title: "Mail Attachments", text: "Copies of attachments Mail saved locally.", icon: Mail, module: "cleanup" },
    { title: "Trash Bins", text: "Empty the Trash on every connected drive at once.", icon: Trash, module: "cleanup" },
    { title: "Malware Scan", text: "Checks apps and background items against Apple's own XProtect signatures, offline.", icon: Bug, module: "protection" },
    { title: "Privacy", text: "Clear browsing history, cookies and recent-items lists. Never passwords or bookmarks.", icon: EyeOff, module: "protection" },
    { title: "App Permissions", text: "See which apps can use your camera, mic and screen, and revoke the ones you don't want.", icon: KeyRound, module: "protection" },
    { title: "App Updater", text: "Newer versions from the App Store, developer feeds and Homebrew, verified before installing.", icon: RefreshCw, module: "applications" },
    { title: "Large & Old Files", text: "Big files you haven't opened in a long time.", icon: FileClock, module: "clutter" },
    { title: "Duplicate Finder", text: "Identical files stored in more than one place.", icon: Copy, module: "clutter" },
    { title: "Similar Images", text: "Near-identical photos and screenshots. Keep the best one.", icon: Images, module: "clutter" },
    { title: "Downloads", text: "Installers, archives and one-off files piling up in Downloads.", icon: Download, module: "clutter" },
    { title: "Uninstaller", text: "Remove apps along with every support file they created.", icon: AppWindow, module: "applications" },
    { title: "App Leftovers", text: "Support files from apps that are already gone.", icon: Package, module: "applications" },
    { title: "Maintenance Tasks", text: "Flush DNS, reindex Spotlight, rebuild Launch Services.", icon: Wrench, module: "performance" },
    { title: "Free Up RAM", text: "Ask macOS to purge inactive memory.", icon: Cpu, module: "performance" },
    { title: "Login Items", text: "Apps that open every time you log in.", icon: Power, module: "performance" },
    { title: "Background Items", text: "Launch agents and helpers running behind the scenes.", icon: Rocket, module: "performance" },
    { title: "Time Machine Snapshots", text: "Local snapshots quietly holding on to deleted files' space.", icon: Clock, module: "performance" },
  ];

  let query = $state("");
  const shown = $derived(
    TOOLS.filter((t) => `${t.title} ${t.text}`.toLowerCase().includes(query.trim().toLowerCase())),
  );
</script>

<section class="tools">
  <header data-tauri-drag-region>
    <div>
      <h1>My Tools</h1>
      <p class="muted">Focused tools for one job at a time.</p>
    </div>
    <label class="search glass">
      <Search size={14} />
      <input placeholder="Search…" bind:value={query} spellcheck="false" />
    </label>
  </header>

  <div class="grid">
    {#each shown as t (t.title)}
      {@const m = MODULE[t.module]}
      <article class="card glass">
        <span class="badge" style:background="linear-gradient(160deg, {m.theme.accentStrong}, {m.theme.accent})">
          <t.icon size={17} strokeWidth={2} />
        </span>
        <h3>{t.title}</h3>
        <p>{t.text}</p>
        <div class="foot">
          <button class="btn" onclick={() => app.go(t.module)}>Open</button>
        </div>
      </article>
    {:else}
      <p class="muted">No tools match “{query}”.</p>
    {/each}
  </div>
</section>

<style>
  /* The header stays put; only the grid scrolls, so cards never slide under it. */
  .tools {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 0 0 28px;
  }
  header {
    flex: none;
    padding-right: 28px !important;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    padding: 46px 0 18px;
  }
  h1 {
    font-size: 32px;
    font-weight: 400;
  }
  header p {
    margin-top: 6px;
    font-size: 14px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 220px;
    height: 32px;
    padding: 0 10px;
    border-radius: 9px;
    color: var(--text-2);
  }
  input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: 0;
    color: var(--text);
    font: inherit;
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    align-content: start;
    padding: 6px 28px 28px 0;
    -webkit-mask-image: linear-gradient(to bottom, transparent 0, #000 14px);
    mask-image: linear-gradient(to bottom, transparent 0, #000 14px);
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 14px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    min-height: 176px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.35);
    margin-bottom: 6px;
  }
  h3 {
    font-size: 14px;
    font-weight: 600;
  }
  .card p {
    color: var(--text-2);
    line-height: 1.4;
    font-size: 12.5px;
  }
  .foot {
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
