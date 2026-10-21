<script lang="ts" module>
  import { Archive, File, FileText, Folder, Image, Music, Package, Video } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { FileKind } from "../api";

  const LOOK: Record<FileKind, { icon: Component; bg: string }> = {
    image: { icon: Image, bg: "linear-gradient(160deg,#ffb36b,#f0743e)" },
    video: { icon: Video, bg: "linear-gradient(160deg,#ff8fa3,#d9415e)" },
    audio: { icon: Music, bg: "linear-gradient(160deg,#f59ad8,#c43f9b)" },
    archive: { icon: Archive, bg: "linear-gradient(160deg,#c9b38a,#94784a)" },
    installer: { icon: Package, bg: "linear-gradient(160deg,#9aa6b8,#5d6a7e)" },
    document: { icon: FileText, bg: "linear-gradient(160deg,#8fb8ff,#4a76e0)" },
    folder: { icon: Folder, bg: "linear-gradient(160deg,#7cc0f7,#3f8fde)" },
    other: { icon: File, bg: "linear-gradient(160deg,#b8b4c6,#8a8599)" },
  };
</script>

<script lang="ts">
  let { kind, size = 30 }: { kind: FileKind; size?: number } = $props();
  const look = $derived(LOOK[kind]);
</script>

<span class="icon" style:width="{size}px" style:height="{size}px" style:background={look.bg}>
  <look.icon size={Math.round(size * 0.5)} />
</span>

<style>
  .icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 26%;
    color: #fff;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3);
  }
</style>
