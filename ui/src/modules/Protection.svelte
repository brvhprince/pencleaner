<script lang="ts">
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Scanning from "../lib/components/Scanning.svelte";
  import { bytes, count } from "../lib/format";
  import { MODULE } from "../lib/modules";
  import Manager from "./protection/Manager.svelte";
  import { protection, type Section } from "./protection/protect.svelte";
  import Summary from "./protection/Summary.svelte";

  const module = MODULE.protection;

  // Browser preview only: #autoscan&review=threats (add &threat for a fake detection).
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await protection.scan();
      if (q.has("review")) protection.review((q.get("review") || "threats") as Section);
    })();
  });
</script>

{#if protection.phase === "home"}
  <ModuleHome {module} />
  <ScanButton label="Scan" onclick={() => protection.scan()} />
{:else if protection.phase === "scanning"}
  <Scanning
    illustration={module.illustration}
    title="Checking for threats…"
    detail={protection.progress?.current ?? ""}
    stats={protection.progress ? `${count(protection.progress.items)} programs checked · ${bytes(protection.progress.bytes)}` : "Loading Apple's XProtect signatures…"}
  />
  <ScanButton label="Stop" busy onclick={() => protection.stop()} />
{:else if protection.view === "summary"}
  <Summary />
{:else}
  <Manager />
{/if}
