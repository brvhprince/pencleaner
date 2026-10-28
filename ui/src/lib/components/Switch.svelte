<script lang="ts">
  let { on, onchange, label, disabled = false, busy = false }: { on: boolean; onchange: (v: boolean) => void; label: string; disabled?: boolean; busy?: boolean } = $props();
</script>

<button class="switch" class:on class:busy role="switch" aria-checked={on} aria-label={label} disabled={disabled || busy} onclick={() => onchange(!on)}>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    width: 34px;
    height: 20px;
    flex: none;
    border-radius: 10px;
    background: var(--switch-off, #d4d0de);
    transition: background 180ms ease;
  }
  .switch.on {
    background: var(--switch-on, var(--m-accent, var(--accent)));
  }
  .switch:disabled {
    opacity: 0.45;
  }
  .switch.busy {
    opacity: 0.7;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    transition: transform 200ms var(--ease);
  }
  .on .knob {
    transform: translateX(14px);
  }
</style>
