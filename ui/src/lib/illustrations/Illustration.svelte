<script lang="ts" module>
  // Optional: the vendor pack is gitignored (see README.md), so load whatever exists.
  const files = import.meta.glob("./vendor/*.svg", { query: "?raw", import: "default", eager: true }) as Record<
    string,
    string
  >;
  import.meta.glob("./vendor/empty.css", { eager: true });

  const FALLBACK = `<svg viewBox="0 0 120 100" xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-width="1.2" opacity=".55"><path d="M20 60 60 80l40-20-40-20z"/><path d="M20 60v8l40 20 40-20v-8"/><path d="M45 52V30l15-8 15 8v22" stroke-dasharray="3 3"/></svg>`;

  export function hasIllustration(name: string) {
    return `./vendor/${name}.svg` in files;
  }
</script>

<script lang="ts">
  interface Props {
    name: string;
    /** Height in px. */
    size?: number;
    /** Hero art: heavier strokes, filled faces, and a glow. */
    hero?: boolean;
    /** Animate as if hovered (the pack animates on hover). */
    active?: boolean;
  }
  let { name, size = 100, hero = false, active = false }: Props = $props();
  const svg = $derived(files[`./vendor/${name}.svg`] ?? files["./vendor/default.svg"] ?? FALLBACK);
</script>

<div class="illustration empty empty--filled" class:hero class:active style:height="{size}px" aria-hidden="true">
  {#if hero}<div class="glow"></div>{/if}
  {@html svg}
</div>

<style>
  .illustration {
    position: relative;
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 0;
    color: var(--empty-color);
  }
  .illustration :global(svg) {
    position: relative;
    height: 100%;
    width: auto;
    overflow: visible;
  }
  .hero :global(svg path),
  .hero :global(svg ellipse),
  .hero :global(svg circle),
  .hero :global(svg rect),
  .hero :global(svg line),
  .hero :global(svg polyline) {
    stroke-width: 1.5;
  }
  .glow {
    position: absolute;
    inset: 8% 14%;
    border-radius: 50%;
    background: radial-gradient(closest-side, color-mix(in srgb, var(--accent-strong) 55%, transparent), transparent);
    filter: blur(28px);
    opacity: 0.55;
    animation: breathe 6s ease-in-out infinite;
  }
  .active .glow {
    animation-duration: 2.4s;
    opacity: 0.8;
  }
  @keyframes breathe {
    50% {
      transform: scale(1.08);
      opacity: 0.8;
    }
  }
</style>
