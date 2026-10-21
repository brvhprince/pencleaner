<script lang="ts">
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Scanning from "../lib/components/Scanning.svelte";
  import { bytes, count } from "../lib/format";
  import { MODULE } from "../lib/modules";
  import { cleanup } from "./cleanup/clean.svelte";
  import Cleaning from "./cleanup/Cleaning.svelte";
  import ConfirmSheet from "./cleanup/ConfirmSheet.svelte";
  import Done from "./cleanup/Done.svelte";
  import Manager from "./cleanup/Manager.svelte";
  import Summary from "./cleanup/Summary.svelte";

  const module = MODULE.cleanup;

  // Browser preview only: #autoscan&review&confirm drives the flow for screenshots.
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await cleanup.scan();
      if (q.has("review")) cleanup.review(q.get("review") || null);
      if (q.has("confirm")) {
        cleanup.selected.add(cleanup.items.find((i) => i.warning)!.id);
        cleanup.requestClean();
      }
      if (q.has("clean")) cleanup.clean();
    })();
  });
</script>

{#if cleanup.phase === "home"}
  <ModuleHome {module} />
  <ScanButton label="Scan" onclick={() => cleanup.scan()} />
{:else if cleanup.phase === "scanning"}
  <Scanning
    illustration={module.illustration}
    title="Looking for junk…"
    detail={cleanup.progress?.current ?? ""}
    stats={cleanup.progress ? `${count(cleanup.progress.items)} items checked · ${bytes(cleanup.progress.bytes)} found` : "Starting…"}
  />
  <ScanButton label="Stop" busy onclick={() => cleanup.stop()} />
{:else if cleanup.phase === "results"}
  {#if cleanup.view === "summary"}
    <Summary />
    <ScanButton label="Clean" onclick={() => cleanup.requestClean()} disabled={!cleanup.selected.size} />
  {:else}
    <Manager />
  {/if}
{:else if cleanup.phase === "cleaning"}
  <Cleaning />
{:else}
  <Done />
{/if}
{#if cleanup.confirming}<ConfirmSheet />{/if}
