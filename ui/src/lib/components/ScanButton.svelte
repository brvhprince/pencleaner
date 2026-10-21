<script lang="ts">
  interface Props {
    label: string;
    onclick: () => void;
    /** Shows a ring: a number in 0..1 for determinate, null for a spinner. */
    progress?: number | null;
    busy?: boolean;
    disabled?: boolean;
  }
  let { label, onclick, progress = null, busy = false, disabled = false }: Props = $props();

  const R = 41;
  const C = 2 * Math.PI * R;
</script>

<div class="dock">
  <button class="scan" class:busy {onclick} {disabled}>
    {#if busy}
      <svg class="ring" class:spin={progress === null} viewBox="0 0 90 90" aria-hidden="true">
        <circle cx="45" cy="45" r={R} class="track" />
        <circle
          cx="45"
          cy="45"
          r={R}
          class="value"
          stroke-dasharray={C}
          stroke-dashoffset={progress === null ? C * 0.75 : C * (1 - Math.min(1, Math.max(0, progress)))}
        />
      </svg>
    {/if}
    <span>{label}</span>
  </button>
</div>

<style>
  .dock {
    position: absolute;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    z-index: 5;
  }
  .scan {
    position: relative;
    display: grid;
    place-items: center;
    width: 82px;
    height: 82px;
    border-radius: 50%;
    font-size: 15px;
    font-weight: 500;
    background: radial-gradient(circle at 50% 30%, color-mix(in srgb, var(--accent-strong) 70%, white 10%), var(--accent) 70%);
    box-shadow:
      0 0 0 5px rgba(255, 255, 255, 0.14),
      0 0 40px color-mix(in srgb, var(--accent-strong) 60%, transparent),
      inset 0 1px 0 rgba(255, 255, 255, 0.35);
    transition: transform 200ms var(--ease), box-shadow 200ms ease, filter 200ms ease;
  }
  .scan:hover:not(:disabled) {
    transform: scale(1.04);
    filter: brightness(1.08);
  }
  .scan:active:not(:disabled) {
    transform: scale(0.97);
  }
  .scan:disabled {
    filter: saturate(0.4) brightness(0.8);
  }
  .scan.busy {
    background: radial-gradient(circle at 50% 30%, rgba(255, 255, 255, 0.22), rgba(255, 255, 255, 0.06) 70%);
    box-shadow:
      0 0 0 5px rgba(255, 255, 255, 0.08),
      0 0 30px color-mix(in srgb, var(--accent-strong) 40%, transparent);
  }
  .ring {
    position: absolute;
    inset: -4px;
    width: calc(100% + 8px);
    height: calc(100% + 8px);
    transform: rotate(-90deg);
  }
  .ring.spin {
    animation: spin 1.1s linear infinite;
  }
  circle {
    fill: none;
    stroke-width: 4;
  }
  .track {
    stroke: rgba(255, 255, 255, 0.12);
  }
  .value {
    stroke: #fff;
    stroke-linecap: round;
    transition: stroke-dashoffset 300ms ease;
  }
  @keyframes spin {
    to {
      transform: rotate(270deg);
    }
  }
</style>
