<script lang="ts">
  import { AppWindow, File, Link, Package } from "@lucide/svelte";
  import type { EntryKind } from "../api";

  let { kind, size = 22 }: { kind: EntryKind | "other"; size?: number } = $props();
</script>

{#if kind === "folder"}
  <!-- Finder-style folder, drawn here so it reads as a folder at any size. -->
  <svg width={size} height={size * 0.82} viewBox="0 0 40 33" aria-hidden="true">
    <path d="M2 5a3 3 0 0 1 3-3h10l4 4h16a3 3 0 0 1 3 3v19a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3z" fill="#4c9fe6" />
    <path d="M2 11a3 3 0 0 1 3-3h30a3 3 0 0 1 3 3v17a3 3 0 0 1-3 3H5a3 3 0 0 1-3-3z" fill="#84c8fa" />
    <path d="M2 11a3 3 0 0 1 3-3h30a3 3 0 0 1 3 3v1H2z" fill="#a6d8fd" />
  </svg>
{:else if kind === "app"}
  <span class="glyph app" style:width="{size}px" style:height="{size}px"><AppWindow size={size * 0.62} /></span>
{:else if kind === "bundle"}
  <span class="glyph" style:width="{size}px" style:height="{size}px"><Package size={size * 0.62} /></span>
{:else if kind === "symlink"}
  <span class="glyph" style:width="{size}px" style:height="{size}px"><Link size={size * 0.6} /></span>
{:else if kind === "other"}
  <span class="glyph dots" style:width="{size}px" style:height="{size}px">•••</span>
{:else}
  <span class="glyph file" style:width="{size}px" style:height="{size}px"><File size={size * 0.66} /></span>
{/if}

<style>
  svg,
  .glyph {
    flex: none;
  }
  .glyph {
    display: inline-grid;
    place-items: center;
    border-radius: 22%;
    background: rgba(255, 255, 255, 0.14);
    color: #fff;
  }
  .app {
    background: linear-gradient(160deg, #6aa8ff, #3b5fe0);
  }
  .file {
    background: rgba(255, 255, 255, 0.9);
    color: #6b6b80;
  }
  .dots {
    font-size: 10px;
    letter-spacing: 1px;
    border-radius: 50%;
  }
</style>
