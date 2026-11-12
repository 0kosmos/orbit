<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // A sidebar button for something that is on or off (recording, replay buffer).
  // Its state only changes when Rust sends an event, so there's no polling.
  let {
    labelOff,
    labelOn,
    hint,
    statusCommand,
    toggleCommand,
    onEvents,
    offEvents,
  }: {
    labelOff: string;
    labelOn: string;
    hint: string;
    statusCommand: string;
    toggleCommand: string;
    onEvents: string[];
    offEvents: string[];
  } = $props();

  let on = $state(false);

  $effect(() => {
    invoke<boolean>(statusCommand).then((r) => (on = r));
    const stops = [
      ...onEvents.map((name) => listen(name, () => (on = true))),
      ...offEvents.map((name) => listen(name, () => (on = false))),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });
</script>

<button class="toggle" class:on onclick={() => invoke(toggleCommand)}>
  <span class="dot"></span>
  <span class="text">
    <span class="label">{on ? labelOn : labelOff}</span>
    <span class="hint">{hint}</span>
  </span>
</button>

<style>
  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      border-color var(--normal) var(--ease),
      box-shadow var(--normal) var(--ease),
      transform var(--fast) var(--ease);
  }
  .toggle:hover {
    background: var(--surface-hover);
  }
  .toggle:active {
    transform: scale(0.98);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .label {
    font-size: 13px;
    font-weight: 500;
  }
  .hint {
    font-size: 11px;
    color: var(--text-faint);
  }
  /* No blinking/pulsing on purpose: a constant animation would keep the GPU busy. */
  .dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-faint);
    transition:
      background var(--normal) var(--ease),
      box-shadow var(--normal) var(--ease);
  }
  .toggle.on {
    border-color: color-mix(in srgb, var(--rec) 55%, transparent);
    background: var(--rec-soft);
  }
  .toggle.on .dot {
    background: var(--rec);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--rec) 25%, transparent);
  }
</style>
