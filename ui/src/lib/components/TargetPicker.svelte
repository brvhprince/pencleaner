<script lang="ts" generics="T extends { key: string; label: string; sub?: string }">
  import { ChevronDown, Check, FolderOpen, HardDrive } from "@lucide/svelte";
  import { fly } from "svelte/transition";

  interface Props {
    options: T[];
    value: T | null;
    onpick: (option: T) => void;
    /** Adds a "Choose folder…" entry. */
    onchoose?: () => void;
  }
  let { options, value, onpick, onchoose }: Props = $props();
  let open = $state(false);
  let root: HTMLDivElement;

  function pick(o: T) {
    onpick(o);
    open = false;
  }

  function onWindowClick(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false;
  }
</script>

<svelte:window onclick={onWindowClick} />

<div class="picker" bind:this={root}>
  <button class="trigger" onclick={() => (open = !open)} aria-haspopup="listbox" aria-expanded={open}>
    <HardDrive size={15} />
    <span class="label">{value?.label ?? "Choose…"}</span>
    <ChevronDown size={14} />
  </button>
  {#if open}
    <div class="menu" role="listbox" transition:fly={{ y: -4, duration: 140 }}>
      {#each options as o (o.key)}
        <button role="option" aria-selected={value?.key === o.key} onclick={() => pick(o)}>
          <span class="check">{#if value?.key === o.key}<Check size={13} />{/if}</span>
          <span class="text">
            <span>{o.label}</span>
            {#if o.sub}<span class="sub">{o.sub}</span>{/if}
          </span>
        </button>
      {/each}
      {#if onchoose}
        <hr />
        <button onclick={() => { open = false; onchoose?.(); }}>
          <span class="check"><FolderOpen size={13} /></span>
          <span class="text">Choose folder…</span>
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    width: 240px;
  }
  .trigger {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    height: 36px;
    padding: 0 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.14);
    border: 1px solid rgba(255, 255, 255, 0.18);
    font-weight: 600;
  }
  .trigger:hover {
    background: rgba(255, 255, 255, 0.19);
  }
  .label {
    flex: 1;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .menu {
    position: absolute;
    top: 42px;
    left: 0;
    z-index: 10;
    min-width: 100%;
    padding: 5px;
    border-radius: 11px;
    background: rgba(28, 18, 52, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: var(--shadow);
  }
  .menu button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 7px;
    text-align: left;
  }
  .menu button:hover {
    background: var(--accent);
  }
  .check {
    display: grid;
    place-items: center;
    width: 16px;
  }
  .text {
    display: flex;
    flex-direction: column;
  }
  .sub {
    font-size: 11px;
    color: var(--text-2);
  }
  hr {
    border: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    margin: 4px 6px;
  }
</style>
