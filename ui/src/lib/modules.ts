import type { Component } from "svelte";
import {
  AppWindow,
  Bot,
  ChartLine,
  Clock,
  Copy,
  Download,
  FileClock,
  Folders,
  Hammer,
  HardDrive,
  Images,
  Layers,
  Mail,
  Monitor,
  Orbit,
  Package,
  Power,
  Rocket,
  ScanSearch,
  Sparkles,
  SprayCan,
  Trash,
  Zap,
} from "@lucide/svelte";

export type ModuleId =
  | "smartcare"
  | "cleanup"
  | "performance"
  | "applications"
  | "clutter"
  | "spacelens"
  | "tools"
  | "activity";

export interface Theme {
  /** Window background. */
  bg: string;
  accent: string;
  accentStrong: string;
  /** Mid tone the illustrations' fills blend into. */
  surface: string;
}

export interface Feature {
  icon: Component;
  label: string;
}

export interface ModuleDef {
  id: ModuleId;
  title: string;
  tagline: string;
  icon: Component;
  illustration: string;
  features: Feature[];
  theme: Theme;
  /** Which build phase delivers the scan; undefined once it works. */
  comingIn?: string;
  /** Rendered below the sidebar's main group. */
  secondary?: boolean;
}

const theme = (from: string, mid: string, to: string, glow: string, accent: string, accentStrong: string): Theme => ({
  bg: `radial-gradient(90% 80% at 72% 18%, ${glow} 0%, transparent 60%), linear-gradient(135deg, ${from} 0%, ${mid} 48%, ${to} 100%)`,
  accent,
  accentStrong,
  surface: mid,
});

export const MODULES: ModuleDef[] = [
  {
    id: "smartcare",
    title: "Smart Care",
    tagline: "Quick maintenance that takes care of the essentials.",
    icon: Monitor,
    illustration: "sessions",
    features: [
      { icon: Sparkles, label: "Quick Junk Sweep" },
      { icon: Zap, label: "System Tune-up" },
      { icon: HardDrive, label: "Storage Check" },
    ],
    theme: theme("#240d45", "#4a1a78", "#86308f", "#a63ab3", "#c85bd8", "#e183ee"),
    comingIn: "Phase 7",
  },
  {
    id: "cleanup",
    title: "Cleanup",
    tagline: "Clear out junk to reclaim space and keep your Mac running smoothly.",
    icon: SprayCan,
    illustration: "housekeeping",
    features: [
      { icon: Sparkles, label: "System Junk" },
      { icon: Hammer, label: "Developer Junk" },
      { icon: Bot, label: "AI Clutter" },
      { icon: Mail, label: "Mail Attachments" },
      { icon: Trash, label: "Trash Bins" },
    ],
    theme: theme("#0c2614", "#1d5228", "#3d8a39", "#58a849", "#6fcf5a", "#9be686"),
    comingIn: "Phase 3",
  },
  {
    id: "performance",
    title: "Performance",
    tagline: "Keep your Mac in top shape with recommended maintenance.",
    icon: Zap,
    illustration: "metrics",
    features: [
      { icon: Hammer, label: "Maintenance Tasks" },
      { icon: Power, label: "Login Items" },
      { icon: Rocket, label: "Background Items" },
      { icon: Clock, label: "Time Machine Snapshots" },
    ],
    theme: theme("#3f1205", "#86330f", "#c4561d", "#e47a35", "#ff9a4d", "#ffbb85"),
    comingIn: "Phase 6",
  },
  {
    id: "applications",
    title: "Applications",
    tagline: "Take control of your apps. Uninstall them completely and sweep up what they leave behind.",
    icon: AppWindow,
    illustration: "applications",
    features: [
      { icon: AppWindow, label: "App Uninstaller" },
      { icon: Package, label: "App Leftovers" },
      { icon: Download, label: "Installation Files" },
    ],
    theme: theme("#0b1856", "#1c2d96", "#3d2aa6", "#3d6fe2", "#6d9bff", "#a3c0ff"),
    comingIn: "Phase 5",
  },
  {
    id: "clutter",
    title: "My Clutter",
    tagline: "Sort through your files and reduce the mess in just a few clicks.",
    icon: Folders,
    illustration: "folders",
    features: [
      { icon: FileClock, label: "Large & Old Files" },
      { icon: Copy, label: "Duplicates" },
      { icon: Images, label: "Similar Images" },
      { icon: Download, label: "Downloads" },
    ],
    theme: theme("#0c2b2a", "#1b5652", "#3b8a7e", "#4fa898", "#6fe0c8", "#a5f0e0"),
    comingIn: "Phase 4",
  },
  {
    id: "spacelens",
    title: "Space Lens",
    tagline: "See what's taking up the most disk space and clean up your storage quickly.",
    icon: Orbit,
    illustration: "data-sources",
    features: [
      { icon: Orbit, label: "Visual Storage Map" },
      { icon: ScanSearch, label: "Hidden Files Uncovered" },
      { icon: Folders, label: "Large Folders Overview" },
    ],
    theme: theme("#170a45", "#36168c", "#5a2bbf", "#7a48e6", "#a07bff", "#c7b0ff"),
  },
  {
    id: "tools",
    title: "My Tools",
    tagline: "Focused tools for one job at a time.",
    icon: Layers,
    illustration: "dashboards",
    features: [],
    theme: theme("#1e1230", "#38214f", "#55306a", "#6a3c84", "#c78bff", "#dcb5ff"),
    secondary: true,
  },
  {
    id: "activity",
    title: "My Activity",
    tagline: "Everything PenCleaner has done for your Mac.",
    icon: ChartLine,
    illustration: "insights",
    features: [],
    theme: theme("#14112a", "#241e3f", "#3a2c5c", "#3c3470", "#8fa8ff", "#bccbff"),
    comingIn: "Phase 7",
    secondary: true,
  },
];

export const MODULE = Object.fromEntries(MODULES.map((m) => [m.id, m])) as Record<ModuleId, ModuleDef>;
