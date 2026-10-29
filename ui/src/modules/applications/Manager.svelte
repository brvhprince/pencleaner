<script lang="ts">
  import { ChevronDown, ChevronLeft, ChevronRight, CircleAlert, CircleCheck, EyeOff, Lock, PackageX, RefreshCw, ScanSearch, Search, ShieldCheck } from "@lucide/svelte";
  import { PLATFORM, pick } from "../../lib/modules";
  import { api, type AppEntry, type AppTarget, type AppUpdate, type RelatedFile } from "../../lib/api";
  import AppIcon from "../../lib/components/AppIcon.svelte";
  import Checkbox from "../../lib/components/Checkbox.svelte";
  import FileIcon from "../../lib/components/FileIcon.svelte";
  import { ago, bytes, middleEllipsis, plural } from "../../lib/format";
  import { SECTIONS, apps, isUnused, unusedDays } from "./apps.svelte";

  let query = $state("");
  let filter = $state<"all" | "unused" | "large" | "store">("all");
  let sort = $state<"size" | "name" | "used">("size");

  const r = $derived(apps.report!);
  const meta = $derived(SECTIONS.find((s) => s.id === apps.section)!);
  const q = $derived(query.trim().toLowerCase());

  const appRows = $derived.by(() => {
    const list = r.apps.filter((a) => {
      if (q && !`${a.name} ${a.bundleId}`.toLowerCase().includes(q)) return false;
      if (filter === "unused") return isUnused(a);
      if (filter === "large") return a.size + a.relatedSize > 1e9;
      if (filter === "store") return a.appStore;
      return true;
    });
    const by = {
      size: (a: AppEntry, b: AppEntry) => b.size + b.relatedSize - (a.size + a.relatedSize),
      name: (a: AppEntry, b: AppEntry) => a.name.localeCompare(b.name),
      used: (a: AppEntry, b: AppEntry) => (a.lastUsed ?? 0) - (b.lastUsed ?? 0),
    };
    return list.sort(by[sort]);
  });
  const leftoverRows = $derived(r.leftovers.filter((l) => !q || l.id.includes(q)));
  const installerRows = $derived(r.installers.filter((i) => !q || i.name.toLowerCase().includes(q)));

  const count = (s: (typeof SECTIONS)[number]["id"]) =>
    s === "apps"
      ? r.apps.length
      : s === "updates"
        ? (apps.updates?.updates.length ?? (apps.checkingUpdates ? "…" : 0))
        : s === "leftovers"
          ? r.leftovers.length
          : r.installers.length;

  const SOURCE: Record<AppUpdate["source"], string> = { appStore: "App Store", sparkle: "Developer feed", homebrew: "Homebrew", catalog: "Homebrew catalog" };
  const updateRows = $derived(
    (apps.updates?.updates ?? []).filter((u) => !q || u.name.toLowerCase().includes(q)),
  );
  const storeCount = $derived(updateRows.filter((u) => u.source === "appStore" && !u.requires).length);

  function actionLabel(u: AppUpdate) {
    if (u.oneClick) return "Update";
    if (u.source === "appStore") return "App Store";
    if (u.source === "homebrew") return "How to";
    return u.actionNote?.startsWith("Opens the developer") ? "Website" : "Open App";
  }

  const visible = $derived<AppTarget[]>(
    apps.section === "apps"
      ? appRows.map((a) => ({ kind: "app", id: a.id }))
      : apps.section === "leftovers"
        ? leftoverRows.map((l) => ({ kind: "leftover", id: l.id }))
        : installerRows.map((i) => ({ kind: "installer", id: i.path })),
  );

  function toggleOpen(id: string) {
    if (apps.expanded.has(id)) apps.expanded.delete(id);
    else apps.expanded.add(id);
  }

  function labels(files: RelatedFile[]) {
    return [...new Set(files.map((f) => f.label))].join(", ");
  }
</script>

{#snippet files(list: RelatedFile[])}
  <ul class="files">
    {#each list as f (f.path)}
      <li>
        <span class="flabel">{f.label}</span>
        <span class="fpath" title={f.path}>{middleEllipsis(f.path, 80)}</span>
        {#if f.admin}<span class="lock" title="Needs your administrator password"><Lock size={12} /></span>{/if}
        <span class="fsize tabular">{bytes(f.size)}</span>
        <button class="reveal" title={pick("Show in Finder", "Show in Explorer")} onclick={() => api.revealInFinder(f.path)}><ScanSearch size={13} /></button>
      </li>
    {/each}
  </ul>
{/snippet}

<div class="manager">
  <header data-tauri-drag-region>
    <button class="back" onclick={() => (apps.view = "summary")}><ChevronLeft size={16} /> Back</button>
    <h2 data-tauri-drag-region>Applications Manager</h2>
    <div class="tools">
      <label class="search">
        <Search size={13} />
        <input placeholder="Search" bind:value={query} spellcheck="false" />
      </label>
      {#if apps.section === "apps"}
        <label class="sort">
          Sort by
          <select bind:value={sort}>
            <option value="size">Size</option>
            <option value="name">Name</option>
            <option value="used">Last opened</option>
          </select>
        </label>
      {/if}
    </div>
  </header>

  <div class="body">
    <nav aria-label="Sections">
      <h4>Applications</h4>
      {#each SECTIONS as s (s.id)}
        <div class="group" class:active={s.id === apps.section} role="button" tabindex="0" onclick={() => (apps.section = s.id)} onkeydown={(e) => e.key === "Enter" && (apps.section = s.id)}>
          <s.icon size={16} />
          <span class="gname">{s.title}</span>
          <span class="pill tabular">{count(s.id)}</span>
        </div>
      {/each}
    </nav>

    <section class="list">
      <div class="ghead">
        <h3>{meta.title}</h3>
        <p>{meta.blurb}</p>
        <div class="controls">
          {#if apps.section === "updates"}
            <span>Select:</span>
            <button onclick={() => { for (const u of apps.installable) if (!u.running) apps.updatePicked.add(u.id); }}>All one-click</button>
            <button onclick={() => apps.updatePicked.clear()}>None</button>
            {#if storeCount}<button class="push" onclick={() => api.apps.openAppStoreUpdates()}>Open App Store Updates ({storeCount})</button>{/if}
            <button class:push={!storeCount} onclick={() => apps.checkUpdates()} disabled={apps.checkingUpdates}><RefreshCw size={11} /> Check Again</button>
          {:else if apps.section === "apps"}
            <div class="chips">
              {#each PLATFORM === "windows" ? [["all", "All"], ["large", "Over 1 GB"]] : [["all", "All"], ["unused", `Unused ${unusedDays() >= 365 ? "a year" : `${Math.round(unusedDays() / 30)}+ months`}`], ["large", "Over 1 GB"], ["store", "App Store"]] as [id, label] (id)}
                <button class:on={filter === id} onclick={() => (filter = id as typeof filter)}>{label}</button>
              {/each}
            </div>
            <span class="push">Select:</span>
            <button onclick={() => apps.select(visible, false)}>None</button>
          {:else}
            <span>Select:</span>
            {#if apps.section === "leftovers"}
              <button onclick={() => apps.select(apps.recommendedLeftovers(), true)} title="Vendors with nothing else installed, untouched for a month">Recommended</button>
            {:else}
              <button onclick={() => apps.select(r.installers.filter((i) => i.recommended).map((i) => ({ kind: "installer", id: i.path })), true)}>Recommended</button>
            {/if}
            <button onclick={() => apps.select(visible, true)}>All</button>
            <button onclick={() => apps.select(visible, false)}>None</button>
          {/if}
        </div>
      </div>

      <ul>
        {#if apps.section === "updates"}
          {#if apps.checkingUpdates && !apps.updates}
            <li class="empty"><span class="spin"></span> {apps.updateStep || "Checking for updates…"}</li>
          {:else}
            {#each updateRows as u (u.id)}
              {@const st = apps.updateState.get(u.id)}
              <li class="block">
                <div class="line">
                  {#if u.oneClick && !u.requires}
                    <Checkbox state={apps.updatePicked.has(u.id)} onchange={() => (apps.updatePicked.has(u.id) ? apps.updatePicked.delete(u.id) : apps.updatePicked.add(u.id))} label="Select {u.name}" disabled={apps.installing} />
                  {:else}
                    <span class="nobox"></span>
                  {/if}
                  <div class="row" role="button" tabindex="0" onclick={() => toggleOpen(u.id)} onkeydown={(e) => e.key === "Enter" && toggleOpen(u.id)}>
                    {#if u.appPath}<AppIcon path={u.appPath} size={32} />{:else}<FileIcon kind="installer" size={32} />{/if}
                    <span class="text">
                      <span class="name">
                        {u.name}
                        <em class="tag">{SOURCE[u.source]}</em>
                        {#if u.signed}<span class="shield" title={u.source === "appStore" ? "Delivered by Apple" : "Signed by the developer; verified before installing"}><ShieldCheck size={12} /></span>{/if}
                        {#if u.running}<em class="tag live">Running</em>{/if}
                      </span>
                      <span class="detail">
                        {#if st}
                          <span class="st {st.status}">{#if st.status === "working"}<span class="spin"></span>{:else if st.status === "done"}<CircleCheck size={11} />{:else}<CircleAlert size={11} />{/if} {st.message}</span>
                        {:else if u.requires}
                          <span class="st failed">{u.requires}</span>
                        {:else}
                          <span class="ver tabular">{u.installed} → <strong>{u.latest}</strong></span>{#if u.actionNote} · {u.actionNote}{/if}
                        {/if}
                      </span>
                    </span>
                    {#if u.size}<span class="size tabular">{bytes(u.size)}</span>{/if}
                  </div>
                  <button class="skip" title="Hide this version; a newer one will show up again" disabled={apps.installing} onclick={() => apps.skipUpdate(u)}>Skip</button>
                  <button class="act" disabled={apps.installing || !!u.requires || st?.status === "done"} onclick={() => apps.installUpdates([u.id])}>{actionLabel(u)}</button>
                </div>
                {#if apps.expanded.has(u.id)}
                  <div class="notes">
                    {#if u.notes}<p>{u.notes}</p>{:else}<p class="muted">No release notes provided.</p>{/if}
                    {#if u.notesUrl}<button class="link" onclick={() => api.openUrl(u.notesUrl!)}>More details</button>{/if}
                  </div>
                {/if}
              </li>
            {:else}
              <li class="empty">Everything is up to date.</li>
            {/each}
            {#if apps.updates}
              <li class="footnote">
                Checked {apps.updates.checked} apps.
                {#if apps.updates.unchecked.length}<span title={apps.updates.unchecked.join(", ")}>{apps.updates.unchecked.length} have no source PenCleaner can check; most update themselves.</span>{/if}
                {#if apps.updates.errors.length}<span title={apps.updates.errors.map(([a, b]) => `${a}: ${b}`).join("\n")}>{plural(apps.updates.errors.length, "source")} didn't answer.</span>{/if}
              </li>
            {/if}
          {/if}
        {:else if apps.section === "apps"}
          {#each appRows as a (a.id)}
            {@const t = { kind: "app", id: a.id } as const}
            <li class="block">
              <div class="line">
                <Checkbox state={apps.has(t)} onchange={() => apps.toggle(t)} label="Select {a.name}" />
                <div class="row" role="button" tabindex="0" onclick={() => apps.toggle(t)} onkeydown={(e) => e.key === " " && (e.preventDefault(), apps.toggle(t))}>
                  <AppIcon path={a.path} size={32} />
                  <span class="text">
                    <span class="name">
                      {a.name}
                      {#if a.version}<span class="ver">{a.version}</span>{/if}
                      {#if a.appStore}<em class="tag">App Store</em>{/if}
                      {#if a.running}<em class="tag live">Running</em>{/if}
                    </span>
                    <span class="detail">
                      {#if PLATFORM === "windows"}
                        {[a.publisher, a.installed ? `installed ${ago(a.installed)}` : null].filter(Boolean).join(" · ") || "Installed"} ·
                      {:else}
                        {a.lastUsed ? `Opened ${ago(a.lastUsed)}` : "Never opened"} ·
                      {/if}
                      {#if a.sharesSupportFiles}
                        another copy is installed, so its support files stay
                      {:else}
                        {bytes(a.size)} app + {bytes(a.relatedSize)} in {plural(a.related.length, pick("support file", "data folder"))}
                      {/if}
                    </span>
                  </span>
                  <span class="size tabular">{bytes(a.size + a.relatedSize)}</span>
                </div>
                <button class="reveal shown" title="Show support files" onclick={() => toggleOpen(a.id)} disabled={!a.related.length}>
                  {#if apps.expanded.has(a.id)}<ChevronDown size={15} />{:else}<ChevronRight size={15} />{/if}
                </button>
              </div>
              {#if apps.expanded.has(a.id)}{@render files(a.related)}{/if}
            </li>
          {:else}
            <li class="empty">No apps match.</li>
          {/each}
        {:else if apps.section === "leftovers"}
          {#each leftoverRows as l (l.id)}
            {@const t = { kind: "leftover", id: l.id } as const}
            <li class="block">
              <div class="line">
                <Checkbox state={apps.has(t)} onchange={() => apps.toggle(t)} label="Select {l.id}" />
                <div class="row" role="button" tabindex="0" onclick={() => apps.toggle(t)} onkeydown={(e) => e.key === " " && (e.preventDefault(), apps.toggle(t))}>
                  <span class="kind"><PackageX size={16} /></span>
                  <span class="text">
                    <span class="name">{l.id}</span>
                    <span class="detail">{labels(l.files)}{l.vendorInstalled ? " · same vendor still has apps installed, check first" : ""}</span>
                  </span>
                  <span class="age">{l.lastActivity ? `Touched ${ago(l.lastActivity)}` : ""}</span>
                  <span class="size tabular">{bytes(l.size)}</span>
                </div>
                <button class="reveal" title="Ignore these leftovers: never show them again" onclick={() => apps.ignoreLeftover(l.id)}><EyeOff size={14} /></button>
                <button class="reveal shown" title="Show files" onclick={() => toggleOpen(l.id)}>
                  {#if apps.expanded.has(l.id)}<ChevronDown size={15} />{:else}<ChevronRight size={15} />{/if}
                </button>
              </div>
              {#if apps.expanded.has(l.id)}{@render files(l.files)}{/if}
            </li>
          {:else}
            <li class="empty">No leftovers. Removed apps cleaned up after themselves.</li>
          {/each}
        {:else}
          {#each installerRows as i (i.path)}
            {@const t = { kind: "installer", id: i.path } as const}
            <li>
              <Checkbox state={apps.has(t)} onchange={() => apps.toggle(t)} label="Select {i.name}" />
              <div class="row" role="button" tabindex="0" onclick={() => apps.toggle(t)} onkeydown={(e) => e.key === " " && (e.preventDefault(), apps.toggle(t))}>
                <FileIcon kind="installer" />
                <span class="text">
                  <span class="name">{i.name}</span>
                  <span class="detail">{i.installedApp ? `${i.installedApp} is already installed` : "No matching app installed"}</span>
                </span>
                <span class="age">{i.added ? `Added ${ago(i.added)}` : ""}</span>
                <span class="size tabular">{bytes(i.size)}</span>
              </div>
              <button class="reveal" title={pick("Show in Finder", "Show in Explorer")} onclick={() => api.revealInFinder(i.path)}><ScanSearch size={14} /></button>
            </li>
          {:else}
            <li class="empty">No installer files found.</li>
          {/each}
        {/if}
      </ul>
    </section>
  </div>

  <footer>
    {#if apps.section === "updates"}
      <span class="count tabular">
        {apps.updatePicked.size ? plural(apps.updatePicked.size, "update") + " selected" : "No one-click updates selected"}
        {#if apps.updatePicked.size}<i>|</i> old versions go to the Trash{/if}
      </span>
      <button class="action" disabled={!apps.updatePicked.size || apps.installing} onclick={() => apps.installUpdates([...apps.updatePicked])}>
        {apps.installing ? "Updating…" : "Update"}
      </button>
    {:else}
      <span class="count tabular">
        {apps.selected.size ? plural(apps.selected.size, "item") + " selected" : "No items selected"} <i>|</i> {bytes(apps.selectedSize)}
      </span>
      <button class="action" disabled={!apps.selected.size} onclick={() => (apps.sheet = "confirm")}>
        {apps.selectedApps.length ? "Uninstall" : "Remove"}
      </button>
    {/if}
  </footer>
</div>

<style>
  .nobox {
    width: 16px;
    flex: none;
  }
  .shield {
    display: inline-grid;
    margin-left: 5px;
    color: #2f7d4a;
    vertical-align: -1px;
  }
  .ver strong {
    color: var(--m-text);
  }
  .st {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 600;
  }
  .st.done {
    color: #2f7d4a;
  }
  .st.failed {
    color: #c0392b;
  }
  .st.working {
    color: var(--m-accent);
  }
  .spin {
    display: inline-block;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid #d4d0de;
    border-top-color: var(--m-accent);
    animation: spin 0.8s linear infinite;
    vertical-align: -1px;
  }
  .skip {
    flex: none;
    font-size: 12px;
    font-weight: 600;
    color: var(--m-muted);
    padding: 0 6px;
  }
  .skip:hover:not(:disabled) {
    color: var(--m-text);
  }
  .act {
    flex: none;
    min-width: 82px;
    height: 26px;
    padding: 0 12px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 600;
    color: var(--m-accent);
    background: color-mix(in srgb, var(--m-accent) 12%, white);
  }
  .act:hover:not(:disabled) {
    background: color-mix(in srgb, var(--m-accent) 20%, white);
  }
  .act:disabled {
    opacity: 0.45;
  }
  .notes {
    margin: 0 8px 10px 72px;
    padding: 10px 12px;
    border-radius: 8px;
    background: #f6f5f9;
    font-size: 12px;
    line-height: 1.5;
  }
  .notes p {
    white-space: pre-line;
    color: var(--m-text);
  }
  .notes .muted {
    color: var(--m-muted);
  }
  .notes .link {
    margin-top: 6px;
    font-weight: 600;
    color: var(--m-accent);
  }
  .footnote {
    min-height: 0 !important;
    padding: 14px 10px !important;
    font-size: 11.5px;
    color: var(--m-muted);
    gap: 6px;
  }
  .footnote:hover {
    background: none !important;
  }
  .empty .spin {
    margin-right: 6px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .block {
    display: block;
    padding: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px 0 10px;
  }
  .ver {
    margin-left: 6px;
    font-weight: 500;
    font-size: 11.5px;
    color: var(--m-muted);
  }
  .tag {
    margin-left: 6px;
    font-style: normal;
    vertical-align: 1px;
  }
  .tag.live {
    color: #fff;
    background: #d48806;
  }
  .kind {
    background: linear-gradient(160deg, #b8b4c6, #77728a) !important;
  }
  .chips {
    display: flex;
    gap: 4px;
  }
  .chips button {
    font-weight: 500;
    color: var(--m-muted);
  }
  .chips button.on {
    color: #fff;
    background: var(--m-accent);
  }
  .push {
    margin-left: auto;
  }
  :global(.manager) .reveal.shown {
    opacity: 1;
  }
  .files {
    list-style: none;
    margin: 0 0 8px 64px;
    padding: 6px 8px;
    border-radius: 8px;
    background: #f6f5f9;
    overflow: visible;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
    font-size: 11.5px;
  }
  .flabel {
    width: 118px;
    flex: none;
    font-weight: 600;
    color: var(--m-text);
  }
  .fpath {
    flex: 1;
    min-width: 0;
    color: var(--m-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lock {
    display: grid;
    color: #a35f00;
  }
  .fsize {
    width: 66px;
    text-align: right;
    font-weight: 600;
  }
  :global(.manager) .row .age {
    width: 150px;
  }
</style>
