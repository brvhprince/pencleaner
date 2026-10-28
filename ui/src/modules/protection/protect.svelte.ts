// Protection state, at module scope so results survive switching modules.
import type { Component } from "svelte";
import { SvelteSet } from "svelte/reactivity";
import { Bell, Bug, EyeOff, KeyRound } from "@lucide/svelte";
import { api, type Grant, type Progress, type ProtectionReport } from "../../lib/api";
import { bytes, plural } from "../../lib/format";
import { app } from "../../lib/state.svelte";

export type Section = "threats" | "privacy" | "permissions" | "watcher";

export const SECTIONS: { id: Section; title: string; blurb: string; icon: Component }[] = [
  { id: "threats", title: "Threats", blurb: "Malware matched by Apple's XProtect signatures, and background items that look out of place.", icon: Bug },
  { id: "privacy", title: "Privacy", blurb: "Browsing traces and recent-items lists. Saved passwords and bookmarks are never touched.", icon: EyeOff },
  { id: "permissions", title: "App Permissions", blurb: "Apps you've allowed to use the camera, microphone, screen and your files.", icon: KeyRound },
  { id: "watcher", title: "Background Watcher", blurb: "Get told the moment something sets itself up to run in the background.", icon: Bell },
];

class Protection {
  phase = $state<"home" | "scanning" | "results">("home");
  view = $state<"summary" | "review">("summary");
  section = $state<Section>("threats");
  progress = $state<Progress | null>(null);
  report = $state<ProtectionReport | null>(null);
  privacyPicked = new SvelteSet<string>();
  /** Item ids currently being removed or revoked. */
  busy = new SvelteSet<string>();
  cleaningPrivacy = $state(false);

  threats = $derived((this.report?.detections.length ?? 0) + (this.report?.suspicious.length ?? 0));
  privacySize = $derived(this.report?.privacy.reduce((s, i) => s + i.size, 0) ?? 0);
  orphanGrants = $derived(this.report?.permissions.filter((g) => g.orphaned) ?? []);
  pickedSize = $derived(this.report?.privacy.filter((i) => this.privacyPicked.has(i.id)).reduce((s, i) => s + i.size, 0) ?? 0);

  private setPhase(p: "home" | "scanning" | "results") {
    this.phase = p;
    app.phases.protection = p;
  }

  async scan() {
    this.progress = null;
    this.setPhase("scanning");
    try {
      const r = await api.protection.scan((p) => (this.progress = p));
      if (r.cancelled) {
        this.setPhase("home");
        return;
      }
      this.report = r;
      this.privacyPicked.clear();
      for (const i of r.privacy) if (i.recommended && !i.running) this.privacyPicked.add(i.id);
      this.view = "summary";
      this.setPhase("results");
      if (app.module !== "protection") {
        app.toast(this.threats ? `Protection found ${plural(this.threats, "threat")}.` : "Protection scan finished: no threats.", this.threats ? "error" : "success");
      }
    } catch (e) {
      app.toast(`Scan failed: ${e}`, "error");
      this.setPhase("home");
    }
  }

  stop() {
    api.protection.cancel();
  }

  review(s: Section) {
    this.section = s;
    this.view = "review";
  }

  async removeThreat(kind: "detection" | "suspicious", id: string, name: string) {
    this.busy.add(id);
    try {
      await api.protection.remove(kind, id);
      const r = this.report;
      if (r) {
        this.report = {
          ...r,
          detections: kind === "detection" ? r.detections.filter((d) => d.target !== id) : r.detections,
          suspicious: kind === "suspicious" ? r.suspicious.filter((f) => f.item.id !== id) : r.suspicious,
        };
      }
      app.toast(`${name} removed. It's in the Trash.`, "success");
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.busy.delete(id);
    }
  }

  /** Marks a flagged background item as known-good. */
  async trust(id: string, name: string) {
    if (!(await app.ignore("item", `protection:suspicious:${id}`, name, "protection"))) return;
    if (this.report) this.report = { ...this.report, suspicious: this.report.suspicious.filter((f) => f.item.id !== id) };
  }

  async cleanPrivacy() {
    const ids = [...this.privacyPicked];
    if (!ids.length) return;
    this.cleaningPrivacy = true;
    try {
      const res = await api.protection.cleanPrivacy(ids);
      const r = this.report;
      if (r) this.report = { ...r, privacy: r.privacy.filter((i) => !res.cleaned.includes(i.id)) };
      for (const id of res.cleaned) this.privacyPicked.delete(id);
      if (res.cleaned.length) app.toast(`Cleared ${plural(res.cleaned.length, "item")} (${bytes(res.bytes)}).`, "success");
      if (res.failed.length) app.toast(res.failed.map(([, e]) => e).filter((e, i, a) => a.indexOf(e) === i).join(" "), "error");
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.cleaningPrivacy = false;
    }
  }

  async revoke(g: Grant) {
    this.busy.add(g.id);
    try {
      const fresh = await api.protection.revoke(g.id);
      if (this.report) this.report = { ...this.report, permissions: fresh };
      app.toast(`${g.appName} can no longer use ${g.label}. macOS will ask again if it tries.`, "success");
    } catch (e) {
      app.toast(String(e), "error");
    } finally {
      this.busy.delete(g.id);
    }
  }

  async setWatcher(on: boolean) {
    try {
      const s = await api.protection.setWatcher(on);
      if (this.report) this.report = { ...this.report, watcher: s.enabled, alerts: s.alerts };
      app.toast(s.enabled ? "Watching for new background items." : "Background watcher is off.", "success");
    } catch (e) {
      app.toast(`Couldn't change the watcher: ${e}`, "error");
    }
  }

  async clearAlerts() {
    await api.protection.clearAlerts();
    if (this.report) this.report = { ...this.report, alerts: [] };
  }

  startOver() {
    this.report = null;
    this.setPhase("home");
  }
}

export const protection = new Protection();
