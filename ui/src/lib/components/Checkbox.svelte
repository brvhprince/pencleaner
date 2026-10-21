<script lang="ts">
  interface Props {
    state: "all" | "some" | "none" | boolean;
    onchange: () => void;
    label: string;
    disabled?: boolean;
  }
  let { state, onchange, label, disabled = false }: Props = $props();
  const on = $derived(state === true || state === "all");
  const mixed = $derived(state === "some");
</script>

<button
  class="box"
  class:on
  class:mixed
  role="checkbox"
  aria-checked={mixed ? "mixed" : on}
  aria-label={label}
  {disabled}
  onclick={(e) => {
    e.stopPropagation();
    onchange();
  }}
>
  {#if on}
    <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6.2 5 8.6 9.6 3.6" /></svg>
  {:else if mixed}
    <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 6h6" /></svg>
  {/if}
</button>

<style>
  .box {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    flex: none;
    border-radius: 4px;
    border: 1.5px solid var(--check-border, rgba(255, 255, 255, 0.45));
    transition: background 120ms ease, border-color 120ms ease;
  }
  .on,
  .mixed {
    background: var(--check-on, var(--accent-strong));
    border-color: var(--check-on, var(--accent-strong));
  }
  .box:disabled {
    opacity: 0.45;
  }
  svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: #fff;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
