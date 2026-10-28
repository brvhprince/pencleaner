<script lang="ts">
  import { ArrowDown, ArrowUp, BatteryCharging, BatteryFull, BatteryLow, Cpu, HardDrive, Laptop, MemoryStick, Settings, ShieldCheck, ShieldOff, Sparkles, Trash, Wifi } from "@lucide/svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { api, inTauri, type ActivityEvent, type CareStatus, type TrashedApp, type Vitals } from "./lib/api";
  import { plural } from "./lib/format";
  import { ago, bytes } from "./lib/format";

  let v = $state<Vitals | null>(null);
  let xprotect = $state<string | null>(null);
  let watcher = $state(false);
  let events = $state<ActivityEvent[]>([]);
  let care = $state<CareStatus | null>(null);
  let confirmTrash = $state(false);
  let emptying = $state(false);

  // "You just trashed an app": a compact card instead of the full popover.
  let trashed = $state<TrashedApp | null>(null);
  let trashedIcon = $state<string | null>(null);
  let trashedState = $state<"offer" | "removing" | "done">("offer");
  let trashedResult = $state("");

  async function offer(t: TrashedApp) {
    trashed = t;
    trashedState = "offer";
    trashedIcon = await api.apps.icon(t.path).catch(() => null);
  }

  async function removeLeftovers() {
    trashedState = "removing";
    try {
      const r = await api.trashedApp.remove();
      trashedResult = `Moved ${bytes(r.bytesFreed)} of leftovers to the Trash.`;
    } catch (e) {
      trashedResult = String(e);
    }
    trashedState = "done";
  }

  function dismiss() {
    api.trashedApp.dismiss();
    trashed = null;
  }

  async function refresh() {
    try {
      v = await api.vitals();
    } catch {
      // keep the last reading
    }
  }

  /** Slow-changing facts: once each time the popover opens. */
  async function refreshContext() {
    const [about, w, acts, c] = await Promise.all([
      api.about().catch(() => null),
      api.protection.watcherStatus().catch(() => false),
      api.activity.list().catch(() => []),
      api.care.status().catch(() => null),
    ]);
    care = c;
    xprotect = about?.xprotect ?? null;
    watcher = w;
    events = acts;
    confirmTrash = false;
  }

  // Poll only while the popover is visible and focused.
  let timer: ReturnType<typeof setInterval> | null = null;
  function start() {
    refresh();
    refreshContext();
    timer ??= setInterval(refresh, 2000);
  }
  function stop() {
    if (timer) clearInterval(timer);
    timer = null;
  }

  onMount(() => {
    start();
    const onFocus = () => start();
    const onBlur = () => stop();
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    const offs = inTauri
      ? [
          listen("menubar-shown", () => {
            trashed = null;
            start();
          }),
          listen<TrashedApp>("trashed-app", (e) => offer(e.payload)),
        ]
      : [];
    if (location.hash.includes("trashed")) {
      offer({ name: "Postman", bundleId: "com.postmanlabs.mac", path: "/Applications/Postman.app", size: 214e6, files: [{ path: "~/Library/Application Support/Postman", label: "Application Support", size: 210e6, admin: false }, { path: "~/Library/Caches/com.postmanlabs.mac", label: "Caches", size: 4e6, admin: false }] });
    }
    return () => {
      stop();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      for (const off of offs) off.then((f) => f());
    };
  });

  const lastOf = (module: string, action?: string) =>
    [...events].reverse().find((e) => e.module === module && (!action || e.action === action));
  const lastProtection = $derived(lastOf("protection"));
  const lastClean = $derived([...events].reverse().find((e) => e.bytes > 0));

  const DAY = 86400;
  const now = () => Date.now() / 1000;

  type Rec = { icon: typeof Sparkles; title: string; text: string; action: string; run: () => void };
  const rec = $derived.by((): Rec => {
    if (v && v.diskTotal && v.diskFree / v.diskTotal < 0.1) {
      return { icon: HardDrive, title: "Your disk is almost full", text: `Only ${bytes(v.diskFree)} left. Smart Care finds what's safe to remove.`, action: "Run Smart Care", run: () => api.showMain("smartcare") };
    }
    if (v && v.trash > 1e9) {
      return { icon: Trash, title: `The Trash holds ${bytes(v.trash)}`, text: "Emptying it gives that space back.", action: confirmTrash ? `Empty ${bytes(v.trash)} for good?` : "Empty Trash", run: emptyTrash };
    }
    if (care?.due) {
      const since = care.lastRun ? `Last run ${ago(care.lastRun)}.` : `You asked for one every ${care.everyDays === 1 ? "day" : `${care.everyDays} days`}.`;
      return { icon: Sparkles, title: "Smart Care is due", text: since, action: "Run Smart Care", run: () => api.showMain("smartcare") };
    }
    if (v && v.memoryLevel !== "normal") {
      return { icon: MemoryStick, title: "Memory is under pressure", text: "See what's using it and free some up.", action: "Open Memory", run: () => api.showMain("performance") };
    }
    if (!lastClean || now() - lastClean.at > 7 * DAY) {
      return { icon: Sparkles, title: "Time for a quick tidy", text: lastClean ? `Last cleaned ${ago(lastClean.at)}.` : "PenCleaner hasn't cleaned anything yet.", action: "Run Smart Care", run: () => api.showMain("smartcare") };
    }
    if (!lastProtection || now() - lastProtection.at > 14 * DAY) {
      return { icon: ShieldCheck, title: "Check for threats", text: "A Protection scan takes about a minute and stays on this Mac.", action: "Scan Now", run: () => api.showMain("protection") };
    }
    return { icon: Sparkles, title: "All good", text: `Cleaned ${ago(lastClean.at)}. Nothing needs you right now.`, action: "Open PenCleaner", run: () => api.showMain() };
  });

  async function emptyTrash() {
    if (!confirmTrash) {
      confirmTrash = true;
      return;
    }
    emptying = true;
    try {
      await api.emptyTrash();
      await refresh();
    } finally {
      emptying = false;
      confirmTrash = false;
    }
  }

  const rate = (b: number) => (b < 1000 ? `${b} B/s` : `${bytes(b)}/s`);
  const uptime = $derived(v ? (v.uptimeSecs > DAY ? `${Math.floor(v.uptimeSecs / DAY)}d ${Math.floor((v.uptimeSecs % DAY) / 3600)}h` : `${Math.floor(v.uptimeSecs / 3600)}h`) : "");
  const healthWord = { good: "Good", fair: "Fair", attention: "Needs Attention" } as const;
</script>

{#if trashed}
  <div class="pop compact">
    <h1>Uninstall {trashed.name} completely?</h1>
    <p class="lede">You moved it to the Trash, but it left files behind in your Library.</p>
    <div class="card leftover">
      {#if trashedIcon}<img src={trashedIcon} alt="" />{:else}<span class="ph"><Trash size={18} /></span>{/if}
      <span class="rtext">
        <strong>{trashed.name}</strong>
        <span>{plural(trashed.files.length, "leftover")} · {bytes(trashed.size)}</span>
      </span>
    </div>
    {#if trashedState === "done"}
      <p class="lede result">{trashedResult}</p>
      <div class="actions"><button class="btn" onclick={dismiss}>Done</button></div>
    {:else}
      <div class="actions">
        <button class="btn ghost" disabled={trashedState === "removing"} onclick={dismiss}>Keep Them</button>
        <button class="btn primary" disabled={trashedState === "removing"} onclick={removeLeftovers}>
          {trashedState === "removing" ? "Removing…" : "Remove Leftovers"}
        </button>
      </div>
    {/if}
  </div>
{:else}
<div class="pop">
  <header>
    <div>
      <h1>Mac Health: <span class={v?.health ?? "good"}>{v ? healthWord[v.health] : "…"}</span></h1>
      <p>{v?.model ?? "Mac"}</p>
    </div>
    <span class="mac"><Laptop size={30} strokeWidth={1.5} /></span>
  </header>

  <button class="card protect" onclick={() => api.showMain("protection")}>
    <span class="row" class:on={watcher}>
      {#if watcher}<ShieldCheck size={18} />{:else}<ShieldOff size={18} />{/if}
      <strong>Protection</strong>
      <span class="pill" class:on={watcher}>{watcher ? "Watching" : "Watcher off"}</span>
    </span>
    <span class="sub">
      XProtect {xprotect ? `v${xprotect}` : "signatures"} · {lastProtection ? `last scan ${ago(lastProtection.at)}` : "not scanned yet"}
    </span>
  </button>

  {#if v}
    <div class="grid">
      <div class="card tile">
        <span class="t"><HardDrive size={15} /> {v.diskName}</span>
        <span class="big">{bytes(v.diskFree)}</span>
        <span class="sub">free of {bytes(v.diskTotal)}</span>
        <div class="bar"><div style:width="{100 - (v.diskFree / v.diskTotal) * 100}%"></div></div>
        <button class="link" onclick={() => api.showMain("cleanup")}>Free Up</button>
      </div>
      <div class="card tile">
        <span class="t"><MemoryStick size={15} /> Memory</span>
        <span class="big" class:warn={v.memoryLevel !== "normal"}>{v.memoryPressure}%</span>
        <span class="sub">pressure · {bytes(v.swapUsed)} swap</span>
        <div class="bar"><div style:width="{v.memoryPressure}%" class:hot={v.memoryLevel !== "normal"}></div></div>
        <button class="link" onclick={() => api.showMain("performance")}>Free Up</button>
      </div>
      {#if v.battery}
        {@const b = v.battery}
        <div class="card tile">
          <span class="t">
            {#if b.state.startsWith("charg") || b.state.includes("finishing")}<BatteryCharging size={15} />{:else if b.percent < 20}<BatteryLow size={15} />{:else}<BatteryFull size={15} />{/if}
            Battery
          </span>
          <span class="big">{b.percent}%</span>
          <span class="sub">{b.remaining ? `${b.remaining} left` : b.state.replace(/^./, (c) => c.toUpperCase())}</span>
          {#if b.health !== null}<span class="sub" class:warn={b.health < 80}>Health {b.health}%{b.cycles !== null ? ` · ${b.cycles} cycles` : ""}</span>{/if}
        </div>
      {/if}
      <div class="card tile">
        <span class="t"><Cpu size={15} /> CPU</span>
        <span class="big">{v.cpuLoad}%</span>
        <span class="sub">load · up {uptime}</span>
        <div class="bar"><div style:width="{v.cpuLoad}%" class:hot={v.cpuLoad > 85}></div></div>
      </div>
      <div class="card tile">
        <span class="t"><Wifi size={15} /> Network</span>
        <span class="speed"><ArrowUp size={12} /> {rate(v.netUp)}</span>
        <span class="speed"><ArrowDown size={12} /> {rate(v.netDown)}</span>
      </div>
      <div class="card tile">
        <span class="t"><Trash size={15} /> Trash</span>
        <span class="big">{v.trash ? bytes(v.trash) : "Empty"}</span>
        {#if v.trash}
          <button class="link" class:danger={confirmTrash} disabled={emptying} onclick={emptyTrash}>{confirmTrash ? "Empty for good?" : "Empty"}</button>
        {/if}
      </div>
    </div>
  {/if}

  <h2>Today's Recommendation</h2>
  <div class="card rec">
    <span class="ricon"><rec.icon size={18} /></span>
    <span class="rtext">
      <strong>{rec.title}</strong>
      <span>{rec.text}</span>
    </span>
    <button class="btn" class:danger={confirmTrash && rec.action.startsWith("Empty")} disabled={emptying} onclick={rec.run}>{rec.action}</button>
  </div>

  <footer>
    <button class="open" onclick={() => api.showMain()}>Open PenCleaner</button>
    <button class="gear" title="Settings" onclick={() => api.showMain("settings")}><Settings size={15} /></button>
  </footer>
</div>
{/if}

<style>
  .compact {
    gap: 10px;
    padding: 16px;
  }
  .compact h1 {
    font-size: 15px;
  }
  .lede {
    font-size: 12px;
    color: var(--text-2);
    margin-top: -4px;
  }
  .lede.result {
    margin-top: 0;
    color: #fff;
  }
  .leftover {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .leftover img,
  .ph {
    width: 36px;
    height: 36px;
  }
  .ph {
    display: grid;
    place-items: center;
  }
  .actions {
    margin-top: auto;
    display: flex;
    justify-content: space-between;
  }
  .actions:has(> :only-child) {
    justify-content: flex-end;
  }
  :global(html.menubar),
  :global(html.menubar body),
  :global(html.menubar #app) {
    background: transparent !important;
  }
  .pop {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 12px 12px 8px;
    border-radius: 14px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    background:
      radial-gradient(120% 60% at 80% 0%, rgba(166, 58, 179, 0.55), transparent 60%),
      linear-gradient(160deg, #2a1155 0%, #3b1874 55%, #251047 100%);
    overflow: hidden;
    color: #fff;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px 4px 4px;
  }
  h1 {
    font-size: 17px;
    font-weight: 600;
  }
  h1 .good {
    color: #8ff0b0;
  }
  h1 .fair {
    color: #ffd08a;
  }
  h1 .attention {
    color: #ff9c9c;
  }
  header p {
    font-size: 12px;
    color: var(--text-2);
    margin-top: 2px;
  }
  .mac {
    color: rgba(255, 255, 255, 0.75);
  }
  .card {
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.08);
    padding: 10px 12px;
    text-align: left;
  }
  .protect {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .protect:hover,
  .tile:hover {
    background: rgba(255, 255, 255, 0.11);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .row :global(svg) {
    color: var(--text-2);
  }
  .row.on :global(svg) {
    color: #8ff0b0;
  }
  .pill {
    margin-left: auto;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-2);
  }
  .pill.on {
    color: #8ff0b0;
  }
  .sub {
    font-size: 11.5px;
    color: var(--text-2);
  }
  .sub.warn,
  .big.warn {
    color: #ffd08a;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 76px;
  }
  .t {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 600;
    margin-bottom: 2px;
  }
  .big {
    font-size: 19px;
    font-weight: 300;
  }
  .speed {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    height: 3px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.14);
    overflow: hidden;
    margin-top: 4px;
  }
  .bar div {
    height: 100%;
    background: #fff;
    transition: width 400ms ease;
  }
  .bar div.hot {
    background: #ffb36b;
  }
  .link {
    position: absolute;
    right: 10px;
    top: 9px;
    font-size: 11.5px;
    font-weight: 600;
    color: #d9b8ff;
  }
  .link:hover {
    color: #fff;
  }
  .link.danger {
    color: #ff9c9c;
  }
  h2 {
    margin: 4px 4px 0;
    font-size: 13px;
    font-weight: 600;
  }
  .rec {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 10px;
    align-items: start;
  }
  .ricon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: linear-gradient(160deg, #e183ee, #a63ab3);
  }
  .rtext {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: var(--text-2);
  }
  .rtext strong {
    color: #fff;
    font-size: 13px;
  }
  .rec .btn {
    grid-column: 2;
    justify-self: end;
  }
  .btn.danger {
    background: #d9415e;
    border-color: transparent;
  }
  footer {
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    padding-top: 6px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  .open {
    font-weight: 600;
    padding: 6px 10px;
    border-radius: 7px;
  }
  .open:hover,
  .gear:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  .gear {
    position: absolute;
    right: 0;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 7px;
    color: var(--text-2);
  }
</style>
