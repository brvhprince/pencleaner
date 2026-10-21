<script lang="ts">
  import Illustration from "../illustrations/Illustration.svelte";
  import { middleEllipsis } from "../format";

  interface Props {
    illustration: string;
    title: string;
    detail?: string;
    stats?: string;
  }
  let { illustration, title, detail = "", stats = "" }: Props = $props();
</script>

<div class="drag" data-tauri-drag-region></div>
<section class="scanning">
  <Illustration name={illustration} size={230} hero active />
  <h2>{title}</h2>
  <p class="detail" title={detail}>{middleEllipsis(detail, 90) || " "}</p>
  {#if stats}<p class="stats tabular">{stats}</p>{/if}
</section>

<style>
  .drag {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--titlebar-h);
    z-index: 3;
  }
  .scanning {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 0 40px 110px;
    text-align: center;
  }
  h2 {
    margin-top: 28px;
    font-size: 30px;
    font-weight: 400;
  }
  .detail {
    margin-top: 10px;
    font-size: 13px;
    color: var(--text-2);
    min-height: 18px;
    max-width: 640px;
    white-space: nowrap;
  }
  .stats {
    margin-top: 6px;
    font-size: 12px;
    color: var(--text-3);
  }
</style>
