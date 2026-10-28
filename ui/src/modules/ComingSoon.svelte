<script lang="ts">
  import ModuleHome from "../lib/components/ModuleHome.svelte";
  import ScanButton from "../lib/components/ScanButton.svelte";
  import type { ModuleDef } from "../lib/modules";
  import { app } from "../lib/state.svelte";

  let { module }: { module: ModuleDef } = $props();
</script>

<ModuleHome {module} />
<ScanButton label="Scan" onclick={() => app.toast(`${module.title} arrives in ${module.comingIn}. everything else works today.`)} />
