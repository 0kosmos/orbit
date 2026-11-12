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
  {on ? labelOn : labelOff}
  <span class="hint">{hint}</span>
</button>

<style>
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid #222;
    border-radius: 6px;
    background: transparent;
    color: #ccc;
    font: inherit;
    cursor: pointer;
  }
  .toggle:hover {
    background: #141414;
    color: #fff;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
  }
  .toggle.on {
    border-color: #6b2222;
    color: #fff;
  }
  .toggle.on .dot {
    background: #e53935;
  }
  .hint {
    margin-left: auto;
    font-size: 11px;
    color: #666;
  }
</style>
