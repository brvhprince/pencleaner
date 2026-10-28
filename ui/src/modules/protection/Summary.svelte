<script lang="ts">
  import { Bell, Bug, CircleCheck, EyeOff, KeyRound, RotateCcw, ShieldCheck, TriangleAlert } from "@lucide/svelte";
  import Switch from "../../lib/components/Switch.svelte";
  import { bytes, plural } from "../../lib/format";
  import { protection } from "./protect.svelte";

  const r = $derived(protection.report!);
  const clean = $derived(protection.threats === 0);
</script>

<section class="summary">
  <div class="topbar" data-tauri-drag-region>
    <button class="link" onclick={() => protection.startOver()}><RotateCcw size={13} /> Start Over</button>
    <span class="title" data-tauri-drag-region>Protection</span>
  </div>

  <header class:bad={!clean}>
    <span class="seal">{#if clean}<ShieldCheck size={30} />{:else}<TriangleAlert size={30} />{/if}</span>
    <h1>{clean ? "No threats found." : `${plural(protection.threats, "threat")} found.`}</h1>
    <p class="muted">
      Checked {r.filesScanned.toLocaleString()} programs against {r.rules ? `${r.rules.rules} XProtect signatures (v${r.rules.version ?? "?"})` : "your background items"}.
      Everything ran on this Mac.
    </p>
    {#if !clean}<button class="btn lg primary" onclick={() => protection.review("threats")}>Review Threats</button>{/if}
  </header>

  <div class="cards">
    <button class="card glass" onclick={() => protection.review("threats")}>
      <span class="badge"><Bug size={16} /></span>
      <h3>Malware & Suspicious Items</h3>
      <p class="big tabular">{protection.threats ? protection.threats : "None"}</p>
      <p class="muted">{r.detections.length} malware · {r.suspicious.length} suspicious background {r.suspicious.length === 1 ? "item" : "items"}</p>
      {#if clean}<span class="ok"><CircleCheck size={13} /> All clear</span>{/if}
    </button>
    <button class="card glass" onclick={() => protection.review("privacy")}>
      <span class="badge"><EyeOff size={16} /></span>
      <h3>Privacy</h3>
      <p class="big tabular">{bytes(protection.privacySize)}</p>
      <p class="muted">{plural(r.privacy.length, "trace")} across {new Set(r.privacy.map((i) => i.owner)).size} apps</p>
    </button>
    <button class="card glass" onclick={() => protection.review("permissions")}>
      <span class="badge"><KeyRound size={16} /></span>
      <h3>App Permissions</h3>
      <p class="big tabular">{r.permissionsError ? "—" : r.permissions.length}</p>
      <p class="muted">
        {r.permissionsError ? "Needs Full Disk Access to read" : protection.orphanGrants.length ? `${plural(protection.orphanGrants.length, "harmless leftover")} from removed apps` : "grants to review"}
      </p>
    </button>
    <div class="card glass">
      <span class="badge"><Bell size={16} /></span>
      <h3>Background Watcher</h3>
      <div class="row">
        <p class="muted">{r.watcher ? "On: you'll be told when something new installs itself." : "Off"}</p>
        <Switch on={r.watcher} label="Background watcher" onchange={(v) => protection.setWatcher(v)} />
      </div>
      {#if r.alerts.length}
        <button class="alerts" onclick={() => protection.review("watcher")}>{plural(r.alerts.length, "alert")}</button>
      {/if}
    </div>
  </div>
</section>

<style>
  .summary {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 0 28px 28px;
    --switch-on: #fff;
    --switch-off: rgba(255, 255, 255, 0.25);
  }
  .topbar {
    display: flex;
    align-items: center;
    height: var(--titlebar-h);
    flex: none;
  }
  .title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-weight: 600;
    color: var(--text-2);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
    color: var(--text-2);
  }
  header {
    margin: auto 0 28px;
    text-align: center;
  }
  .seal {
    display: inline-grid;
    place-items: center;
    width: 64px;
    height: 64px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.14);
    box-shadow: 0 0 40px color-mix(in srgb, var(--accent-strong) 50%, transparent);
  }
  header.bad .seal {
    background: #ff5d6c;
  }
  h1 {
    margin: 16px 0 8px;
    font-size: 30px;
    font-weight: 400;
  }
  header p {
    max-width: 520px;
    margin: 0 auto 16px;
    line-height: 1.5;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
    margin-bottom: auto;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 16px;
    min-height: 190px;
    text-align: left;
  }
  button.card:hover {
    background: var(--glass-2);
  }
  .badge {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    margin-bottom: 8px;
    background: linear-gradient(160deg, var(--accent-strong), var(--accent));
  }
  h3 {
    font-size: 13.5px;
    font-weight: 600;
  }
  .big {
    margin-top: auto;
    font-size: 26px;
    font-weight: 300;
  }
  .card .muted {
    font-size: 12px;
    line-height: 1.4;
  }
  .ok {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 4px;
    font-size: 12px;
    font-weight: 600;
  }
  .row {
    margin-top: auto;
    display: flex;
    align-items: flex-end;
    gap: 10px;
  }
  .row .muted {
    flex: 1;
  }
  .alerts {
    margin-top: 8px;
    font-size: 12px;
    font-weight: 600;
    text-decoration: underline;
  }
</style>
