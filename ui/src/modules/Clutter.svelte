<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { inTauri } from "../lib/api";
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import Scanning from "../lib/components/Scanning.svelte";
  import TargetPicker from "../lib/components/TargetPicker.svelte";
  import { bytes, count } from "../lib/format";
  import { MODULE } from "../lib/modules";
  import { app } from "../lib/state.svelte";
  import { clutter, type Section, type Target } from "./clutter/clutter.svelte";
  import Manager from "./clutter/Manager.svelte";
  import Summary from "./clutter/Summary.svelte";

  const module = MODULE.clutter;
  type Option = Target & { key: string; label: string; sub?: string };

  const home = $derived<Option | null>(
    app.system ? { key: app.system.home, label: app.system.user, sub: "Home folder", name: app.system.user, path: app.system.home } : null,
  );
  const options = $derived(home ? [home] : []);
  const value = $derived(
    clutter.target ? (options.find((o) => o.path === clutter.target!.path) ?? { ...clutter.target, key: clutter.target.path, label: clutter.target.name }) : null,
  );

  $effect(() => {
    if (!clutter.target && home) clutter.target = home;
  });

  async function chooseFolder() {
    const picked = await openDialog({ directory: true, multiple: false, title: "Choose a folder to sort through" });
    if (typeof picked === "string") clutter.target = { name: picked.split("/").filter(Boolean).pop() ?? picked, path: picked };
  }

  // Browser preview only: #autoscan&review=similar drives the flow for screenshots.
  let demoed = false;
  $effect(() => {
    const q = new URLSearchParams(location.hash.slice(1));
    if (inTauri || demoed || !clutter.target || !q.has("autoscan")) return;
    demoed = true;
    (async () => {
      await clutter.scan();
      if (q.has("review")) clutter.review((q.get("review") || "duplicates") as Section);
      if (q.has("select")) clutter.autoSelect(clutter.report!.duplicates);
    })();
  });
</script>

{#if clutter.phase === "home"}
  <ModuleHome {module}>
    <TargetPicker {options} {value} onpick={(o) => (clutter.target = o)} onchoose={chooseFolder} />
  </ModuleHome>
  <ScanButton label="Scan" onclick={() => clutter.scan()} disabled={!clutter.target} />
{:else if clutter.phase === "scanning"}
  <Scanning
    illustration={module.illustration}
    title="Digging through your files…"
    detail={clutter.progress?.current ?? ""}
    stats={clutter.progress ? `${count(clutter.progress.items)} files · ${bytes(clutter.progress.bytes)}` : "Starting…"}
  />
  <ScanButton label="Stop" busy onclick={() => clutter.stop()} />
{:else if clutter.view === "summary"}
  <Summary />
{:else}
  <Manager />
{/if}
