<script lang="ts">
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Scanning from "../lib/components/Scanning.svelte";
  import { count } from "../lib/format";
  import { MODULE } from "../lib/modules";
  import { apps, type Section } from "./applications/apps.svelte";
  import Manager from "./applications/Manager.svelte";
  import RemoveSheet from "./applications/RemoveSheet.svelte";
  import Summary from "./applications/Summary.svelte";

  const module = MODULE.applications;

  // Browser preview only: #autoscan&review=apps&expand&confirm drives the flow for screenshots.
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await apps.scan();
      if (q.has("review")) apps.review((q.get("review") || "apps") as Section);
      if (q.has("expand")) apps.expanded.add("/Applications/zoom.us.app");
      if (q.get("review") === "updates") {
        while (!apps.updates) await new Promise((r) => setTimeout(r, 100));
        apps.expanded.add("/Applications/Maccy.app");
        if (q.has("install")) apps.installUpdates([...apps.updatePicked, "/Applications/iTerm.app", "/Applications/WhatsApp.app"]);
      }
      if (q.has("confirm")) {
        apps.select([{ kind: "app", id: "/Applications/zoom.us.app" }, { kind: "app", id: "/Applications/Claude.app" }, { kind: "leftover", id: "net.metaquotes.wine" }], true);
        apps.sheet = "confirm";
      }
    })();
  });
</script>

{#if apps.phase === "home"}
  <ModuleHome {module} />
  <ScanButton label="Scan" onclick={() => apps.scan()} />
{:else if apps.phase === "scanning"}
  <Scanning
    illustration={module.illustration}
    title="Scanning application files…"
    detail={apps.progress?.current || "Looking for apps, leftovers and installers."}
    stats={apps.progress ? `${count(apps.progress.items)} items checked` : ""}
  />
  <ScanButton label="Scanning" busy onclick={() => {}} disabled />
{:else if apps.view === "summary"}
  <Summary />
{:else}
  <Manager />
{/if}
{#if apps.sheet !== "closed"}<RemoveSheet />{/if}
