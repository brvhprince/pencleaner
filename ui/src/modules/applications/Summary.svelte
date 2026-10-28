<script lang="ts">
  import { RefreshCw, RotateCcw } from "@lucide/svelte";
  import AppIcon from "../../lib/components/AppIcon.svelte";
  import FileIcon from "../../lib/components/FileIcon.svelte";
  import { bytes, plural } from "../../lib/format";
  import { apps, UNUSED_DAYS } from "./apps.svelte";

  const r = $derived(apps.report!);
  const unusedSize = $derived(apps.unused.reduce((s, a) => s + a.size + a.relatedSize, 0));
  const leftoverSize = $derived(r.leftovers.reduce((s, l) => s + l.size, 0));
  const installerSize = $derived(r.installers.reduce((s, i) => s + i.size, 0));
  const heaviest = $derived([...r.apps].sort((a, b) => b.relatedSize - a.relatedSize)[0]);
  const showcase = $derived((apps.unused.length ? apps.unused : r.apps).slice(0, 3));

  const updatesTitle = $derived(
    apps.checkingUpdates && !apps.updates
      ? "Checking for updates…"
      : apps.updates?.updates.length
        ? `${plural(apps.updates.updates.length, "Update")} Available`
        : "Apps Are Up to Date",
  );
  const updatesText = $derived(
    apps.checkingUpdates && !apps.updates
      ? apps.updateStep || "Asking the App Store, developer feeds and Homebrew."
      : apps.updates?.updates.length
        ? `${apps.installable.length} install in one click; the rest open the App Store or the app.`
        : `Checked ${apps.updates?.checked ?? 0} apps.`,
  );

  function quickLeftovers() {
    apps.select(apps.recommendedLeftovers(), true);
    apps.review("leftovers");
  }
</script>

<section class="summary">
  <div class="topbar" data-tauri-drag-region>
    <button class="link" onclick={() => apps.startOver()}><RotateCcw size={13} /> Start Over</button>
    <span class="title" data-tauri-drag-region>Applications</span>
  </div>

  <header>
    <h1>We've found {plural(r.apps.length, "app")} on your Mac.</h1>
    <button class="btn lg" onclick={() => apps.review("apps")}>Manage My Applications</button>
  </header>

  <div class="grid">
    <button class="card glass tall" onclick={() => apps.review("apps")}>
      <h3>{apps.unused.length ? `${plural(apps.unused.length, "app")} you haven't opened in ${UNUSED_DAYS / 30} months` : "Every app is in use"}</h3>
      <p>
        {apps.unused.length
          ? `Together with their support files they take ${bytes(unusedSize)}.`
          : "Nothing has gone unopened for half a year. Browse the list to uninstall anything else."}
      </p>
      <div class="icons">
        {#each showcase as a, i (a.id)}<span style:--i={i}><AppIcon path={a.path} size={i === 1 ? 84 : 60} /></span>{/each}
      </div>
      <span class="btn">Review</span>
    </button>

    <div class="card glass split">
      <div>
        <div class="head"><span class="badge"><RefreshCw size={15} /></span><h3>{updatesTitle}</h3></div>
        <p>{updatesText}</p>
        {#if apps.updates?.updates.length}
          <div class="actions">
            <button class="btn" onclick={() => apps.review("updates")}>Review</button>
            {#if apps.installable.length}
              <button class="btn primary" onclick={() => { apps.review("updates"); apps.installUpdates([...apps.updatePicked]); }} disabled={apps.installing || !apps.updatePicked.size}>Update {apps.updatePicked.size}</button>
            {/if}
          </div>
        {/if}
      </div>
      <div>
        <h3>{r.leftovers.length ? `${bytes(leftoverSize)} of App Leftovers` : "No App Leftovers"}</h3>
        <p>{r.leftovers.length ? `Support files from ${plural(r.leftovers.length, "app")} that ${r.leftovers.length === 1 ? "is" : "are"} no longer installed.` : "Removed apps left nothing behind."}</p>
        {#if r.leftovers.length}
          <div class="actions">
            <button class="btn" onclick={() => apps.review("leftovers")}>Review</button>
            <button class="btn primary" onclick={quickLeftovers}>Select Safe</button>
          </div>
        {/if}
      </div>
    </div>

    <div class="card glass split">
      <div>
        <div class="head"><FileIcon kind="installer" size={30} /><h3>{r.installers.length ? `${bytes(installerSize)} of Installers` : "No Installers"}</h3></div>
        <p>{r.installers.length ? `${plural(r.installers.length, "disk image or package", "disk images and packages")} you no longer need.` : "No .dmg or .pkg files lying around."}</p>
        {#if r.installers.length}<button class="btn" onclick={() => apps.review("installers")}>Review</button>{/if}
      </div>
      {#if heaviest && heaviest.relatedSize > 0}
        <div>
          <div class="head"><AppIcon path={heaviest.path} size={30} /><h3>{heaviest.name} keeps {bytes(heaviest.relatedSize)}</h3></div>
          <p>in support files, the most of any app. Uninstalling removes them too.</p>
          <button class="btn" onclick={() => { apps.expanded.add(heaviest.id); apps.review("apps"); }}>Show</button>
        </div>
      {/if}
    </div>
  </div>
</section>

<style>
  .summary {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 28px 24px;
  }
  .topbar {
    display: flex;
    align-items: center;
    height: var(--titlebar-h);
    flex: none;
  }
  .title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-weight: 600;
    color: var(--text-2);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
    color: var(--text-2);
  }
  .link:hover {
    color: var(--text);
  }
  header {
    text-align: center;
    padding: 20px 0 30px;
  }
  h1 {
    font-size: 24px;
    font-weight: 500;
    margin-bottom: 16px;
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1.6fr;
    grid-template-rows: 1fr 1fr;
    gap: 12px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 18px;
    text-align: left;
    overflow: hidden;
  }
  button.card:hover {
    background: var(--glass-2);
  }
  .tall {
    grid-row: 1 / span 2;
  }
  h3 {
    font-size: 17px;
    font-weight: 500;
  }
  .card p {
    margin-top: 6px;
    font-size: 12.5px;
    line-height: 1.4;
    color: var(--text-2);
  }
  .card .btn,
  .actions {
    margin-top: auto;
    align-self: flex-end;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .icons {
    position: absolute;
    left: 50%;
    top: 56%;
    transform: translate(-50%, -50%);
    display: flex;
    align-items: center;
  }
  .icons span {
    margin: 0 -6px;
    filter: drop-shadow(0 10px 18px rgba(0, 0, 0, 0.3));
    transform: rotate(calc((var(--i) - 1) * 8deg));
    z-index: calc(2 - (var(--i) - 1) * (var(--i) - 1));
  }
  .split {
    flex-direction: row;
    align-items: stretch;
    gap: 0;
    padding: 0;
  }
  .split > div {
    position: relative;
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 18px;
  }
  .split > div + div {
    border-left: 1px solid var(--glass-border);
  }
  .badge {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    flex: none;
    background: linear-gradient(160deg, var(--accent-strong), var(--accent));
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
</style>
