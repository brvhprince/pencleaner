<script lang="ts">
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Scanning from "../lib/components/Scanning.svelte";
  import { MODULE } from "../lib/modules";
  import Manager from "./performance/Manager.svelte";
  import { perf, type Section } from "./performance/perf.svelte";
  import Summary from "./performance/Summary.svelte";

  const module = MODULE.performance;

  // Browser preview only: #autoscan&review=background drives the flow for screenshots.
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await perf.scan();
      if (q.has("review")) perf.review((q.get("review") || "tasks") as Section);
      if (q.has("run")) perf.runTasks();
      if (q.has("full")) await perf.loadFullList();
    })();
  });
</script>

{#if perf.phase === "home"}
  <ModuleHome {module} />
  <ScanButton label="Scan" onclick={() => perf.scan()} />
{:else if perf.phase === "scanning"}
  <Scanning illustration={module.illustration} title="Checking your Mac's performance…" detail="Memory, login items, background services and snapshots." />
  <ScanButton label="Scanning" busy onclick={() => {}} disabled />
{:else if perf.view === "summary"}
  <Summary />
{:else}
  <Manager />
{/if}
