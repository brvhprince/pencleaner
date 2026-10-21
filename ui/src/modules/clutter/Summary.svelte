<script lang="ts">
  import { RotateCcw } from "@lucide/svelte";
  import { fileSrc } from "../../lib/api";
  import FileIcon from "../../lib/components/FileIcon.svelte";
  import { bytes, plural } from "../../lib/format";
  import { clutter, type Section } from "./clutter.svelte";

  const r = $derived(clutter.report!);
  const dup = $derived(clutter.stats("duplicates"));
  const sim = $derived(clutter.stats("similar"));
  const large = $derived(clutter.stats("large"));
  const dl = $derived(clutter.stats("downloads"));
  const previews = $derived(r.similar[0]?.files.slice(0, 3) ?? []);
  const topSource = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const f of r.downloads) if (f.source) counts.set(f.source, (counts.get(f.source) ?? 0) + f.size);
    return [...counts].sort((a, b) => b[1] - a[1])[0]?.[0] ?? null;
  });

  function open(s: Section) {
    clutter.review(s);
  }
</script>

<section class="summary">
  <div class="topbar" data-tauri-drag-region>
    <button class="link" onclick={() => clutter.startOver()}><RotateCcw size={13} /> Start Over</button>
    <span class="title" data-tauri-drag-region>My Clutter</span>
  </div>

  <header>
    <h1>You have {plural(clutter.files.size, "file")} to sort through.</h1>
    <p>Start with the quick picks below, or review everything by hand.</p>
    <button class="btn lg" onclick={() => open("duplicates")}>Review All Files</button>
  </header>

  <div class="grid">
    <button class="card glass dup" onclick={() => open("duplicates")} disabled={!dup.count}>
      <h3>{dup.count ? `${plural(dup.count, "Duplicate Set")} Found` : "No Duplicates"}</h3>
      <p>{dup.count ? `Remove ${bytes(dup.size)} of extra copies and keep one of each.` : "Every file here is one of a kind."}</p>
      <div class="art docs">
        <FileIcon kind="document" size={44} /><FileIcon kind="archive" size={62} /><FileIcon kind="image" size={44} />
      </div>
      <span class="btn">Review</span>
    </button>

    <button class="card glass" onclick={() => open("large")} disabled={!large.count}>
      <h3>{bytes(large.size)} of Large Files</h3>
      <p>{plural(clutter.oldLarge.length, "file")} over 50 MB {clutter.oldLarge.length === 1 ? "hasn't" : "haven't"} been opened in a year.</p>
      <div class="art row"><FileIcon kind="video" size={36} /><FileIcon kind="archive" size={36} /><FileIcon kind="installer" size={36} /></div>
      <span class="btn">Review</span>
    </button>

    <button class="card glass sim" onclick={() => open("similar")} disabled={!sim.count}>
      <h3>{sim.count ? `${plural(sim.count, "Group")} of Similar Images` : "No Similar Images"}</h3>
      <p>{sim.count ? `Keep the best shot of each and free ${bytes(sim.size)}.` : "No near-identical pictures found."}</p>
      {#if previews.length}
        <div class="art thumbs">
          {#each previews as f, i (f.path)}<img src={fileSrc(f.path)} alt="" style:--i={i} loading="lazy" />{/each}
        </div>
      {/if}
      <span class="btn">Review</span>
    </button>

    <button class="card glass" onclick={() => open("downloads")} disabled={!dl.count}>
      <h3>{bytes(dl.size)} in Downloads</h3>
      <p>{plural(dl.count, "item")}{topSource ? `, most of it from ${topSource}` : ""}. Installers and archives are usually safe to clear.</p>
      <div class="art row"><FileIcon kind="folder" size={36} /><FileIcon kind="installer" size={36} /><FileIcon kind="document" size={36} /></div>
      <span class="btn">Review</span>
    </button>
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
    padding: 6px 0 22px;
  }
  h1 {
    font-size: 22px;
    font-weight: 500;
  }
  header p {
    margin: 6px 0 14px;
    font-size: 15px;
    color: var(--text-2);
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
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
    transition: background 160ms ease;
  }
  .card:hover:not(:disabled) {
    background: var(--glass-2);
  }
  .card:disabled {
    opacity: 0.6;
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
    max-width: 360px;
  }
  .card .btn {
    margin-top: auto;
    align-self: flex-end;
  }
  .art {
    display: flex;
    align-items: flex-end;
    gap: 14px;
  }
  .row {
    position: absolute;
    left: 18px;
    bottom: 18px;
    gap: 8px;
  }
  .docs {
    position: absolute;
    left: 50%;
    top: 55%;
    transform: translate(-50%, -50%);
  }
  .dup {
    grid-column: 1;
    grid-row: 1 / span 2;
  }
  .sim {
    grid-column: 2 / span 2;
    grid-row: 1;
  }
  .thumbs {
    position: absolute;
    right: 22px;
    top: 52%;
    transform: translateY(-50%);
  }
  .thumbs img {
    width: 92px;
    height: 70px;
    object-fit: cover;
    border-radius: 8px;
    border: 2px solid rgba(255, 255, 255, 0.85);
    box-shadow: 0 8px 22px rgba(0, 0, 0, 0.3);
    margin-left: -34px;
    transform: rotate(calc((var(--i) - 1) * 6deg)) translateY(calc(var(--i) * -4px));
  }
</style>
