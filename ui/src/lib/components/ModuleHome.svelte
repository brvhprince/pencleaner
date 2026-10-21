<script lang="ts">
  import type { Snippet } from "svelte";
  import Illustration from "../illustrations/Illustration.svelte";
  import type { ModuleDef } from "../modules";

  interface Props {
    module: ModuleDef;
    /** Pickers or options under the feature list. */
    children?: Snippet;
  }
  let { module, children }: Props = $props();
</script>

<div class="drag" data-tauri-drag-region></div>
<section class="home">
  <div class="art">
    <Illustration name={module.illustration} size={250} hero />
  </div>
  <div class="copy">
    {#if module.comingIn}<span class="soon">Coming in {module.comingIn}</span>{/if}
    <h1>{module.title}</h1>
    <p class="tagline">{module.tagline}</p>
    <ul>
      {#each module.features as f (f.label)}
        <li>
          <span class="badge"><f.icon size={15} strokeWidth={2} /></span>
          {f.label}
        </li>
      {/each}
    </ul>
    {#if children}<div class="extra">{@render children()}</div>{/if}
  </div>
</section>

<style>
  .drag {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--titlebar-h);
    z-index: 3;
  }
  .home {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    align-items: center;
    gap: 24px;
    padding: 20px 56px 120px 36px;
  }
  .art {
    display: grid;
    place-items: center;
  }
  h1 {
    font-size: 36px;
    font-weight: 300;
    letter-spacing: -0.01em;
    margin-bottom: 10px;
  }
  .tagline {
    font-size: 15px;
    line-height: 1.45;
    color: var(--text-2);
    max-width: 330px;
    margin-bottom: 26px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 14px;
    font-size: 14px;
    font-weight: 600;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    background: linear-gradient(160deg, var(--accent-strong), var(--accent));
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.35), 0 4px 12px rgba(0, 0, 0, 0.18);
  }
  .extra {
    margin-top: 30px;
  }
  .soon {
    display: inline-block;
    margin-bottom: 14px;
    padding: 4px 10px;
    border-radius: 99px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.02em;
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid rgba(255, 255, 255, 0.16);
  }
</style>
