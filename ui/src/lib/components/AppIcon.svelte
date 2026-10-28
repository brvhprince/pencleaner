<script lang="ts" module>
  import { api } from "../api";

  // Shared across every icon on screen; Rust caches too, this just avoids the IPC hop.
  const cache = new Map<string, Promise<string | null>>();

  function load(path: string) {
    let p = cache.get(path);
    if (!p) {
      p = api.apps.icon(path).catch(() => null);
      cache.set(path, p);
    }
    return p;
  }
</script>

<script lang="ts">
  import { AppWindow } from "@lucide/svelte";

  let { path, size = 32 }: { path: string; size?: number } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    let live = true;
    src = null;
    load(path).then((url) => {
      if (live) src = url;
    });
    return () => (live = false);
  });
</script>

{#if src}
  <img {src} alt="" width={size} height={size} />
{:else}
  <span class="fallback" style:width="{size}px" style:height="{size}px"><AppWindow size={size * 0.55} /></span>
{/if}

<style>
  img {
    flex: none;
    object-fit: contain;
  }
  .fallback {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 22%;
    color: #fff;
    background: linear-gradient(160deg, #8fb8ff, #4a76e0);
  }
</style>
