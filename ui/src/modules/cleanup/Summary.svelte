<script lang="ts">
  import { ChevronRight, RotateCcw, ShieldAlert } from "@lucide/svelte";
  import { bytes } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import { CATEGORIES, cleanup } from "./clean.svelte";

  const cards = $derived(
    CATEGORIES.map((c) => ({ ...c, ...cleanup.totals(c.id), groups: cleanup.groupsOf(c.id) })),
  );
  const notes = $derived(cleanup.groups.filter((g) => g.note).map((g) => g.note!));
</script>

<section class="summary">
  <div class="topbar" data-tauri-drag-region>
    <button class="link" onclick={() => cleanup.startOver()}><RotateCcw size={13} /> Start Over</button>
    <span class="title" data-tauri-drag-region>Cleanup</span>
  </div>

  <header>
    <p class="eyebrow">Ready to clean</p>
    <h1 class="tabular">{bytes(cleanup.selectedSize)}</h1>
    <p class="muted">
      selected of <strong>{bytes(cleanup.foundSize)}</strong> found. Everything pre-selected is safe to remove and rebuilds on its
      own; the rest is waiting for your call.
    </p>
    <button class="btn lg" onclick={() => cleanup.review()}>Review All Items</button>
  </header>

  <div class="cards">
    {#each cards as c (c.id)}
      <button class="card glass" class:empty={c.found === 0} onclick={() => cleanup.review(c.groups.find((g) => g.items.length)?.id ?? null)} disabled={c.found === 0}>
        <span class="badge"><c.icon size={17} strokeWidth={2} /></span>
        <span class="name">{c.title}</span>
        <span class="blurb">{c.blurb}</span>
        <span class="size tabular">{c.found ? bytes(c.found) : "Nothing found"}</span>
        {#if c.found}
          <span class="picked tabular">{c.picked ? `${bytes(c.picked)} selected` : "None selected"}</span>
          <span class="go"><ChevronRight size={15} /></span>
        {/if}
      </button>
    {/each}
  </div>

  {#if notes.length && !app.system?.fullDiskAccess}
    <p class="note"><ShieldAlert size={14} /> {notes[0]} <button class="inline" onclick={() => (app.onboardingDismissed = false)}>Fix this</button></p>
  {/if}
</section>

<style>
  .summary {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 30px 118px;
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
    margin: auto 0 28px;
  }
  .eyebrow {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  h1 {
    font-size: 64px;
    font-weight: 200;
    letter-spacing: -0.02em;
    margin: 4px 0 6px;
  }
  header p.muted {
    max-width: 470px;
    margin: 0 auto 18px;
    line-height: 1.5;
    font-size: 13.5px;
  }
  strong {
    color: var(--text);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 12px;
    margin-bottom: auto;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 16px 14px 14px;
    min-height: 168px;
    text-align: left;
    transition: background 160ms ease, transform 200ms var(--ease);
  }
  .card:hover:not(:disabled) {
    background: var(--glass-2);
    transform: translateY(-2px);
  }
  .card.empty {
    opacity: 0.55;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    margin-bottom: 8px;
    background: linear-gradient(160deg, var(--accent-strong), var(--accent));
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }
  .name {
    font-weight: 600;
    font-size: 13.5px;
  }
  .blurb {
    font-size: 11.5px;
    color: var(--text-2);
    line-height: 1.35;
  }
  .size {
    margin-top: auto;
    padding-top: 10px;
    font-size: 20px;
    font-weight: 300;
  }
  .picked {
    font-size: 11.5px;
    color: var(--text-2);
  }
  .go {
    position: absolute;
    right: 12px;
    bottom: 14px;
    color: var(--text-3);
  }
  .note {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin-top: 14px;
    font-size: 12px;
    color: #ffd08a;
  }
  .inline {
    text-decoration: underline;
    color: inherit;
  }
</style>
