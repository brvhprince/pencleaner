<script lang="ts">
  import { ExternalLink, EyeOff, FolderMinus, FolderOpen, Globe, Power, Shield, SlidersHorizontal, Trash } from "@lucide/svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, inTauri, type IgnoreRule, type LoginStatus } from "../lib/api";
  import Switch from "../lib/components/Switch.svelte";
  import { ago, plural } from "../lib/format";
  import { MODULE, type ModuleId } from "../lib/modules";
  import { app } from "../lib/state.svelte";
  import { protection } from "./protection/protect.svelte";

  const s = $derived(app.settings);
  const folders = $derived(s.ignore.filter((r) => r.kind === "path"));
  const items = $derived(s.ignore.filter((r) => r.kind === "item"));

  let watcher = $state<boolean | null>(null);
  $effect(() => {
    api.protection.watcherStatus().then((on) => (watcher = on));
  });

  let login = $state<LoginStatus | null>(null);
  $effect(() => {
    api.loginItem.status().then((st) => (login = st));
  });
  // Coming back from System Settings: pick up an approval made there.
  $effect(() => {
    const again = () => api.loginItem.status().then((st) => (login = st));
    window.addEventListener("focus", again);
    return () => window.removeEventListener("focus", again);
  });

  async function setLogin(on: boolean) {
    try {
      login = await api.loginItem.set(on);
      if (login.needsApproval) app.toast("macOS has PenCleaner switched off in Login Items. Switch it on there.", "error");
    } catch (e) {
      app.toast(String(e), "error");
    }
  }

  async function setWatcher(on: boolean) {
    await protection.setWatcher(on);
    watcher = on;
  }

  async function excludeFolder() {
    const picked = inTauri
      ? await openDialog({ directory: true, multiple: false, title: "Exclude a folder from every scan" })
      : `${app.system?.home ?? ""}/Sites/archive`;
    if (typeof picked === "string") {
      await app.ignore("path", picked, picked.split("/").filter(Boolean).pop() ?? picked, "settings");
    }
  }

  function moduleTitle(id: string) {
    return id in MODULE ? MODULE[id as ModuleId].title : id === "updates" ? "App Updater" : "Settings";
  }

  /** "updates:/Applications/X.app@2.0" → "Skipped version" etc., for the list's subtitle. */
  function kindOf(r: IgnoreRule) {
    if (r.kind === "path") return "Folder: never scanned";
    const head = r.value.split(":")[0];
    const second = r.value.split(":")[1] ?? "";
    if (head === "updates") return "Skipped update";
    if (head === "protection") return "Trusted background item";
    if (head === "applications" && second === "leftover") return "Ignored leftovers";
    return `Hidden from ${moduleTitle(head)}`;
  }

  const CARE_EVERY = [[1, "day"], [3, "3 days"], [7, "week"], [14, "2 weeks"], [30, "month"]] as const;

  const STALE = [7, 14, 30, 60, 90, 180, 365];
  const LARGE = [25, 50, 100, 250, 500, 1000];
  const UNUSED = [[90, "3 months"], [180, "6 months"], [365, "a year"]] as const;
</script>

<section class="settings">
  <header data-tauri-drag-region>
    <h1>Settings</h1>
    <p class="muted">How PenCleaner scans, and what it should leave alone.</p>
  </header>

  <div class="cols">
    <div class="col">
      <div class="card glass">
        <h3><Power size={15} /> General</h3>
        <div class="row">
          <span>
            <strong>Open PenCleaner at login</strong>
            {#if login && !login.available}
              <em>Works in the installed app (macOS 13 or later)</em>
            {:else if login?.needsApproval}
              <em class="warn">Switched off in System Settings → Login Items</em>
            {:else}
              <em>Starts quietly in the menu bar, without its window</em>
            {/if}
          </span>
          {#if login?.needsApproval}
            <button class="btn" onclick={() => api.loginItem.openSettings()}>Open Settings <ExternalLink size={11} /></button>
          {:else if login}
            <Switch on={login.enabled} label="Open at login" disabled={!login.available} onchange={setLogin} />
          {/if}
        </div>
        <label class="row">
          <span>
            <strong>Scheduled Smart Care</strong>
            {#if s.careMode === "clean"}
              <em>Clears caches, logs and temp files on its own; never the Trash, projects or anything needing your password</em>
            {:else if s.careMode === "remind"}
              <em>A notification and a nudge in the menu bar when it's due</em>
            {:else}
              <em>Runs while PenCleaner is open, so pair it with Open at Login</em>
            {/if}
          </span>
          <select value={s.careMode} onchange={(e) => app.saveSettings({ careMode: e.currentTarget.value as "off" | "remind" | "clean" })}>
            <option value="off">Off</option>
            <option value="remind">Remind me</option>
            <option value="clean">Clean automatically</option>
          </select>
        </label>
        {#if s.careMode !== "off"}
          <label class="row">
            <span>
              <strong>Every</strong>
              <em>Counted from the last Smart Care, by hand or scheduled</em>
            </span>
            <select value={s.careEveryDays} onchange={(e) => app.saveSettings({ careEveryDays: +e.currentTarget.value })}>
              {#each CARE_EVERY as [d, label] (d)}<option value={d}>{label}</option>{/each}
            </select>
          </label>
        {/if}
      </div>

      <div class="card glass">
        <h3><SlidersHorizontal size={15} /> Scanning</h3>
        <label class="row">
          <span>
            <strong>Recommend cleaning projects untouched for</strong>
            <em>Cleanup → Project Build Artifacts</em>
          </span>
          <select value={s.staleProjectDays} onchange={(e) => app.saveSettings({ staleProjectDays: +e.currentTarget.value })}>
            {#each STALE as d (d)}<option value={d}>{d === 365 ? "a year" : `${d} days`}</option>{/each}
          </select>
        </label>
        <label class="row">
          <span>
            <strong>Count files as large from</strong>
            <em>My Clutter → Large & Old Files</em>
          </span>
          <select value={s.largeFileMb} onchange={(e) => app.saveSettings({ largeFileMb: +e.currentTarget.value })}>
            {#each LARGE as mb (mb)}<option value={mb}>{mb >= 1000 ? `${mb / 1000} GB` : `${mb} MB`}</option>{/each}
          </select>
        </label>
        <label class="row">
          <span>
            <strong>Call an app unused after</strong>
            <em>Applications</em>
          </span>
          <select value={s.unusedAppDays} onchange={(e) => app.saveSettings({ unusedAppDays: +e.currentTarget.value })}>
            {#each UNUSED as [d, label] (d)}<option value={d}>{label}</option>{/each}
          </select>
        </label>
      </div>

      <div class="card glass">
        <h3><Globe size={15} /> Going online</h3>
        <div class="row">
          <span>
            <strong>Check for app updates when scanning Applications</strong>
            <em>Asks the App Store, the apps' own feeds and Homebrew</em>
          </span>
          <Switch on={s.checkUpdatesOnScan} label="Check for updates on scan" onchange={(v) => app.saveSettings({ checkUpdatesOnScan: v })} />
        </div>
        <div class="row">
          <span>
            <strong>Ask Apple whether background items are notarized</strong>
            <em>Protection. Off: the check is skipped, never guessed</em>
          </span>
          <Switch on={s.onlineNotarization} label="Online notarization check" onchange={(v) => app.saveSettings({ onlineNotarization: v })} />
        </div>
      </div>

      <div class="card glass">
        <h3><Shield size={15} /> Protection</h3>
        <div class="row">
          <span>
            <strong>Watch for new background items</strong>
            <em>Wakes only when a LaunchAgents/LaunchDaemons folder changes</em>
          </span>
          {#if watcher !== null}<Switch on={watcher} label="Background watcher" onchange={setWatcher} />{/if}
        </div>
        <div class="row">
          <span>
            <strong>Offer to clean up after apps you trash</strong>
            <em>Drag an app to the Trash and PenCleaner offers to remove its leftovers</em>
          </span>
          <Switch on={s.offerTrashedAppCleanup} label="Clean up after trashed apps" onchange={(v) => app.saveSettings({ offerTrashedAppCleanup: v })} />
        </div>
      </div>
    </div>

    <div class="card glass ignore">
      <div class="ihead">
        <h3><EyeOff size={15} /> Ignore List</h3>
        <button class="btn" onclick={excludeFolder}><FolderMinus size={13} /> Exclude a Folder…</button>
      </div>
      <p class="muted small">
        Ignored items stay out of every scan, Smart Care included. Use <em>Ignore</em> on any result to add it here.
      </p>

      {#if !s.ignore.length}
        <p class="empty muted">Nothing ignored yet.</p>
      {:else}
        {#if folders.length}
          <h4>Folders <span>{folders.length}</span></h4>
          <ul>
            {#each folders as r (r.value)}
              <li>
                <span class="kind"><FolderOpen size={14} /></span>
                <span class="text">
                  <strong title={r.value}>{r.label}</strong>
                  <em title={r.value}>{r.value}</em>
                </span>
                <button class="x" onclick={() => app.unignore(r.kind, r.value)} title="Stop ignoring"><Trash size={13} /></button>
              </li>
            {/each}
          </ul>
        {/if}
        {#if items.length}
          <h4>Items <span>{items.length}</span></h4>
          <ul>
            {#each items as r (r.value)}
              <li>
                <span class="kind"><EyeOff size={14} /></span>
                <span class="text">
                  <strong>{r.label}</strong>
                  <em>{kindOf(r)} · {ago(r.added)}</em>
                </span>
                <button class="x" onclick={() => app.unignore(r.kind, r.value)} title="Stop ignoring"><Trash size={13} /></button>
              </li>
            {/each}
          </ul>
        {/if}
        <p class="muted small foot">{plural(s.ignore.length, "rule")}. Removing one brings the item back on the next scan.</p>
      {/if}
    </div>
  </div>
</section>

<style>
  .settings {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 28px 24px;
    --switch-on: #fff;
    --switch-off: rgba(255, 255, 255, 0.22);
  }
  header {
    padding: 46px 0 18px;
  }
  h1 {
    font-size: 32px;
    font-weight: 400;
  }
  header p {
    margin-top: 6px;
    font-size: 14px;
  }
  .cols {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 14px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow-y: auto;
    min-height: 0;
  }
  .card {
    padding: 14px 16px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 9px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
  }
  h3 + .row {
    border-top: 0;
  }
  .row > span {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row strong {
    font-weight: 500;
  }
  em {
    font-style: normal;
    font-size: 11.5px;
    color: var(--text-3);
  }
  em.warn {
    color: #ffc46b;
  }
  select {
    font: inherit;
    font-weight: 600;
    color: var(--text);
    background: rgba(255, 255, 255, 0.12);
    border: 1px solid var(--glass-border);
    border-radius: 7px;
    padding: 4px 8px;
    outline: none;
  }
  .ignore {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .ihead {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .small {
    font-size: 12px;
    line-height: 1.45;
    margin: 4px 0 8px;
  }
  .small em {
    color: var(--text);
    font-size: inherit;
  }
  h4 {
    margin: 10px 0 4px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  h4 span {
    margin-left: 4px;
    font-weight: 500;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 4px;
    border-radius: 8px;
  }
  li:hover {
    background: rgba(255, 255, 255, 0.06);
  }
  .kind {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    flex: none;
    background: rgba(255, 255, 255, 0.1);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .text strong,
  .text em {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .text strong {
    font-weight: 600;
  }
  .x {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    color: var(--text-2);
    opacity: 0;
  }
  li:hover .x {
    opacity: 1;
  }
  .x:hover {
    background: var(--glass-hover);
    color: var(--text);
  }
  .empty {
    padding: 20px 0;
  }
  .foot {
    margin-top: auto;
    padding-top: 8px;
  }
</style>
