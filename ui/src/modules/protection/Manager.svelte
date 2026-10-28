<script lang="ts">
  import { ChevronLeft, CircleCheck, ExternalLink, Lock, Settings, ShieldAlert, TriangleAlert } from "@lucide/svelte";
  import { api, type Grant, type Signer } from "../../lib/api";
  import AppIcon from "../../lib/components/AppIcon.svelte";
  import Checkbox from "../../lib/components/Checkbox.svelte";
  import Switch from "../../lib/components/Switch.svelte";
  import { ago, bytes, plural } from "../../lib/format";
  import { SECTIONS, protection } from "./protect.svelte";

  const r = $derived(protection.report!);
  const meta = $derived(SECTIONS.find((s) => s.id === protection.section)!);

  const count = (id: (typeof SECTIONS)[number]["id"]) =>
    id === "threats" ? protection.threats : id === "privacy" ? r.privacy.length : id === "permissions" ? r.permissions.length : r.alerts.length;

  const grantGroups = $derived.by(() => {
    const m = new Map<string, Grant[]>();
    for (const g of r.permissions) m.set(g.label, [...(m.get(g.label) ?? []), g]);
    return [...m];
  });

  function signerLabel(s: Signer) {
    switch (s.kind) {
      case "apple":
        return "Signed by Apple";
      case "developer":
        return `${s.name}${s.notarized ? " · notarized" : ""}`;
      case "homebrew":
        return "Installed by Homebrew";
      case "adHoc":
        return "Self-signed";
      case "unsigned":
        return "Unsigned";
      case "script":
        return "Script";
    }
  }
</script>

<div class="manager">
  <header data-tauri-drag-region>
    <button class="back" onclick={() => (protection.view = "summary")}><ChevronLeft size={16} /> Back</button>
    <h2 data-tauri-drag-region>Protection</h2>
    <div class="tools"></div>
  </header>

  <div class="body">
    <nav aria-label="Sections">
      <h4>Protection</h4>
      {#each SECTIONS as s (s.id)}
        <div class="group" class:active={s.id === protection.section} role="button" tabindex="0" onclick={() => (protection.section = s.id)} onkeydown={(e) => e.key === "Enter" && (protection.section = s.id)}>
          <s.icon size={16} />
          <span class="gname">{s.title}</span>
          <span class="pill tabular" class:on={s.id === "threats" && protection.threats > 0}>{count(s.id)}</span>
        </div>
      {/each}
    </nav>

    <section class="list">
      <div class="ghead">
        <h3>{meta.title}</h3>
        <p>{meta.blurb}</p>
        {#if protection.section === "privacy"}
          <div class="controls">
            <span>Select:</span>
            <button onclick={() => { for (const i of r.privacy) if (i.recommended && !i.running) protection.privacyPicked.add(i.id); }}>Recommended</button>
            <button onclick={() => protection.privacyPicked.clear()}>None</button>
          </div>
        {:else if protection.section === "permissions"}
          <div class="controls">
            <span>Revoking clears the app's permission; macOS asks again if the app needs it later.</span>
          </div>
        {/if}
      </div>

      <ul>
        {#if protection.section === "threats"}
          {#each r.detections as d (d.target)}
            <li class="threat high">
              <span class="sev"><ShieldAlert size={18} /></span>
              <span class="text">
                <span class="name">{d.target.split("/").pop()} <em class="tag bad">{d.rule}</em></span>
                <span class="detail" title={d.file}>{d.description ?? "Matched Apple's XProtect malware signature"} · {d.target}</span>
              </span>
              <button class="act danger" disabled={protection.busy.has(d.target)} onclick={() => protection.removeThreat("detection", d.target, d.target.split("/").pop() ?? "Threat")}>Remove</button>
            </li>
          {/each}
          {#each r.suspicious as f (f.item.id)}
            <li class="threat {f.severity}">
              <span class="sev"><TriangleAlert size={18} /></span>
              <span class="text">
                <span class="name">{f.item.name} <em class="tag" class:bad={f.severity === "high"}>{f.severity === "high" ? "Suspicious" : "Worth a look"}</em></span>
                <span class="reasons">{#each f.reasons as why (why)}<span>• {why}</span>{/each}</span>
                <span class="detail" title={f.program}>{signerLabel(f.signer)} · {f.program}</span>
              </span>
              <button class="reveal shown" title="Show in Finder" onclick={() => api.revealInFinder(f.item.plist ?? f.program)}><ExternalLink size={13} /></button>
              <button class="act" title="I know this one: don't flag it again" disabled={protection.busy.has(f.item.id)} onclick={() => protection.trust(f.item.id, f.item.name)}>Trust</button>
              <button class="act danger" disabled={protection.busy.has(f.item.id)} onclick={() => protection.removeThreat("suspicious", f.item.id, f.item.name)}>Remove</button>
            </li>
          {/each}
          {#if !protection.threats}
            <li class="empty clear"><CircleCheck size={16} /> No malware and nothing suspicious running in the background.</li>
          {/if}
          {#if r.errors.length}<li class="empty">{r.errors.join(" ")}</li>{/if}
        {:else if protection.section === "privacy"}
          {#each r.privacy as i (i.id)}
            <li class:off={i.running}>
              <Checkbox state={protection.privacyPicked.has(i.id)} onchange={() => (protection.privacyPicked.has(i.id) ? protection.privacyPicked.delete(i.id) : protection.privacyPicked.add(i.id))} label="Select {i.owner} {i.title}" disabled={i.running} />
              <div class="row">
                {#if i.appPath}<AppIcon path={i.appPath} size={30} />{:else}<span class="kind"><Settings size={15} /></span>{/if}
                <span class="text">
                  <span class="name">{i.owner} · {i.title} {#if i.running}<em class="tag live">Open: quit it first</em>{/if}</span>
                  <span class="detail" title={i.warning ?? ""}>{i.detail}{i.warning ? ` · ${i.warning}` : ""}</span>
                </span>
                <span class="size tabular">{bytes(i.size)}</span>
              </div>
            </li>
          {:else}
            <li class="empty">No browsing traces found.</li>
          {/each}
        {:else if protection.section === "permissions"}
          {#if r.permissionsError}
            <li class="empty"><Lock size={14} /> {r.permissionsError}</li>
          {/if}
          {#each grantGroups as [label, grants] (label)}
            <li class="sub">
              {label} <span>{grants.length}</span>
              <button class="settings" onclick={() => api.protection.openSettings(grants[0].settingsAnchor)}>Settings <ExternalLink size={11} /></button>
            </li>
            {#each grants as g (g.id)}
              <li class="grant">
                {#if g.appPath}<AppIcon path={g.appPath} size={28} />{:else}<span class="kind"><Settings size={14} /></span>{/if}
                <span class="text">
                  <span class="name">
                    {g.appName}{g.target ? ` → ${g.target}` : ""}
                    {#if g.orphaned}<em class="tag">App removed</em>{/if}
                    {#if g.system}<span class="lock" title="System-wide: revoking asks for your password"><Lock size={11} /></span>{/if}
                  </span>
                  <span class="detail">
                    {g.granted ? `Allowed ${ago(g.granted)}` : ""}
                    {#if g.orphaned}
                      · {g.lockedToSigner ? "Locked to the removed app's signature, so nothing else can use it" : "Not tied to a signature: an app reusing this ID could inherit it"}
                    {:else if !g.revocable}
                      · a command-line tool: change it in Settings
                    {/if}
                  </span>
                </span>
                {#if g.revocable}
                  <button class="act" disabled={protection.busy.has(g.id)} onclick={() => protection.revoke(g)}>Revoke</button>
                {:else if g.orphaned && g.lockedToSigner}
                  <span class="harmless">Harmless</span>
                {:else if g.orphaned}
                  <button class="act" onclick={() => api.protection.openSettings(g.settingsAnchor)}>Settings</button>
                {/if}
              </li>
            {/each}
          {/each}
        {:else}
          <li class="watch">
            <span class="text">
              <span class="name">Watch for new background items</span>
              <span class="detail wrap">
                macOS wakes PenCleaner only when something is added to a LaunchAgents or LaunchDaemons folder. It's checked
                against XProtect and the suspicious-item rules, and you get a notification. Nothing runs in between.
              </span>
            </span>
            <Switch on={r.watcher} label="Background watcher" onchange={(v) => protection.setWatcher(v)} />
          </li>
          {#if r.alerts.length}
            <li class="sub">Alerts <span>{r.alerts.length}</span> <button class="settings" onclick={() => protection.clearAlerts()}>Clear</button></li>
          {/if}
          {#each r.alerts as a, idx (a.at + a.id + idx)}
            <li class="threat {a.severity ?? 'info'}">
              <span class="sev">{#if a.severity}<TriangleAlert size={18} />{:else}<CircleCheck size={18} />{/if}</span>
              <span class="text">
                <span class="name">{a.name} {#if a.malware}<em class="tag bad">{a.malware}</em>{:else if a.severity}<em class="tag bad">Suspicious</em>{/if}</span>
                {#if a.reasons.length}<span class="reasons">{#each a.reasons as why (why)}<span>• {why}</span>{/each}</span>{/if}
                <span class="detail">Added {ago(a.at)} · {a.plist}</span>
              </span>
              <button class="reveal shown" title="Show in Finder" onclick={() => api.revealInFinder(a.plist)}><ExternalLink size={13} /></button>
            </li>
          {:else}
            <li class="empty">{r.watcher ? "Nothing new has installed itself since the watcher was switched on." : "Switch the watcher on to be told about new background items."}</li>
          {/each}
        {/if}
      </ul>
    </section>
  </div>

  <footer>
    {#if protection.section === "privacy"}
      <span class="count tabular">{protection.privacyPicked.size ? plural(protection.privacyPicked.size, "item") + " selected" : "No items selected"} <i>|</i> {bytes(protection.pickedSize)}</span>
      <button class="action" disabled={!protection.privacyPicked.size || protection.cleaningPrivacy} onclick={() => protection.cleanPrivacy()}>{protection.cleaningPrivacy ? "Clearing…" : "Clear"}</button>
    {:else if protection.section === "threats"}
      <span class="count">Removed items go to the Trash; anything that would restart them is stopped first.</span>
    {:else if protection.section === "permissions"}
      <span class="count">{plural(protection.orphanGrants.length, "leftover permission")} from removed apps: harmless, as macOS ties each to the app's signature.</span>
    {:else}
      <span class="count">{r.watcher ? "Watching LaunchAgents and LaunchDaemons folders." : "The watcher is off."}</span>
    {/if}
  </footer>
</div>

<style>
  li.off .name,
  li.off .detail,
  li.off .size {
    opacity: 0.55;
  }
  .threat {
    align-items: flex-start;
    padding-top: 12px;
    padding-bottom: 12px;
  }
  .sev {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    flex: none;
    color: #fff;
    background: #d48806;
  }
  .threat.high .sev {
    background: #d9415e;
  }
  .threat.info .sev {
    background: #3f9b5c;
  }
  .text {
    flex: 1;
  }
  .reasons {
    display: flex;
    flex-direction: column;
    margin: 3px 0;
    font-size: 12px;
    color: var(--m-text);
  }
  .tag.bad {
    color: #fff !important;
    background: #d9415e !important;
  }
  .tag.live {
    color: #fff !important;
    background: #d48806 !important;
  }
  .tag {
    margin-left: 6px;
    font-style: normal;
    vertical-align: 1px;
  }
  .lock {
    display: inline-grid;
    margin-left: 6px;
    color: #a35f00;
    vertical-align: -1px;
  }
  .kind {
    width: 30px !important;
    height: 30px !important;
    background: linear-gradient(160deg, #b8b4c6, #77728a) !important;
  }
  .act {
    flex: none;
    min-width: 76px;
    height: 26px;
    padding: 0 12px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 600;
    color: var(--m-accent);
    background: color-mix(in srgb, var(--m-accent) 12%, white);
  }
  .act.danger {
    color: #fff;
    background: #d9415e;
  }
  .act:disabled {
    opacity: 0.45;
  }
  .sub {
    min-height: 0 !important;
    padding: 14px 10px 4px !important;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--m-muted);
  }
  .sub:hover,
  .watch:hover,
  .empty:hover {
    background: none !important;
  }
  .sub span {
    margin-left: 6px;
    font-weight: 500;
  }
  .settings {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: none;
    color: var(--m-accent);
  }
  .harmless {
    flex: none;
    min-width: 76px;
    text-align: center;
    font-size: 12px;
    font-weight: 600;
    color: #2f7d4a;
  }
  .grant {
    min-height: 50px;
    gap: 11px !important;
  }
  .watch {
    gap: 16px !important;
    padding: 16px 12px !important;
  }
  .detail.wrap {
    white-space: normal;
    line-height: 1.45;
  }
  .empty {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .empty.clear {
    color: #2f7d4a;
    font-weight: 600;
  }
</style>
