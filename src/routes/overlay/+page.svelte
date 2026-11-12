<script lang="ts">
  // The in-game overlay. Lives in its own see-through window over the game; Rust
  // creates it when you press the overlay hotkey and destroys it when it closes.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fly, fade } from "svelte/transition";
  import StatusBar from "$lib/StatusBar.svelte";
  import { hotkeyLabel } from "$lib/hotkey";
  import { applyUi, ms } from "$lib/ui.svelte";

  type S = {
    theme: string;
    animations: boolean;
    blur: boolean;
    status_bar: boolean;
    replay_seconds: number;
    hotkey_screenshot: string;
    hotkey_record: string;
    hotkey_replay: string;
    hotkey_overlay: string;
  };

  let s = $state<S | null>(null);
  let replayOn = $state(false);
  let recordingSince = $state<number | null>(null);
  let now = $state(Date.now());
  let shown = $state(false); // for the slide-in

  async function loadRecording() {
    recordingSince = await invoke<number | null>("recording_since");
  }

  $effect(() => {
    invoke<S>("get_settings").then((v) => {
      s = v;
      applyUi(v);
      shown = true;
    });
    invoke<boolean>("is_replay_running").then((v) => (replayOn = v));
    loadRecording();
    const stops = [
      listen("replay-started", () => (replayOn = true)),
      listen("replay-stopped", () => (replayOn = false)),
      listen("replay-failed", () => (replayOn = false)),
      listen("recording-started", loadRecording),
      listen("recording-saved", () => (recordingSince = null)),
      listen("recording-failed", () => (recordingSince = null)),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });

  // Recording timer: ticks only while recording.
  $effect(() => {
    if (recordingSince === null) return;
    now = Date.now();
    const id = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(id);
  });

  function elapsed(msTotal: number) {
    const t = Math.max(0, Math.floor(msTotal / 1000));
    const m = String(Math.floor(t / 60)).padStart(2, "0");
    return `${m}:${String(t % 60).padStart(2, "0")}`;
  }

  const close = () => invoke("close_overlay");

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- Slightly dimmed game behind; clicking it closes the overlay. -->
<div class="dim" role="presentation" onclick={close} transition:fade={{ duration: ms(150) }}></div>

{#if shown && s}
  <div class="wrap">
    <div class="panel glass" in:fly={{ y: -18, duration: ms(240) }}>
      <header>
        <img src="/logo.png" alt="" width="22" height="22" />
        <span class="title">Orbit</span>
        <span class="hint">{hotkeyLabel(s.hotkey_overlay)} or Esc to close</span>
        <button class="close" aria-label="Close" onclick={close}>
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
          </svg>
        </button>
      </header>

      <div class="tiles">
        <!-- Screenshot (the overlay hides itself first) -->
        <button class="tile" onclick={() => invoke("overlay_screenshot")}>
          <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
            <path d="M4 8h3l2-3h6l2 3h3v11H4z" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" />
            <circle cx="12" cy="13" r="3.5" fill="none" stroke="currentColor" stroke-width="1.7" />
          </svg>
          <span class="name">Screenshot</span>
          <kbd>{hotkeyLabel(s.hotkey_screenshot)}</kbd>
        </button>

        <!-- Record -->
        <button class="tile" class:live={recordingSince !== null} onclick={() => invoke("toggle_recording")}>
          {#if recordingSince !== null}
            <span class="rec-dot"></span>
            <span class="name">Stop · {elapsed(now - recordingSince)}</span>
          {:else}
            <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
              <circle cx="12" cy="12" r="8" fill="none" stroke="currentColor" stroke-width="1.7" />
              <circle cx="12" cy="12" r="3.5" fill="currentColor" />
            </svg>
            <span class="name">Record</span>
          {/if}
          <kbd>{hotkeyLabel(s.hotkey_record)}</kbd>
        </button>

        <!-- Replay buffer on/off -->
        <button class="tile" class:on={replayOn} onclick={() => invoke("toggle_replay")}>
          <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
            <path d="M3 12a9 9 0 1 0 3-6.7M3 4v5h5M12 8v4l3 2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span class="name">Replay buffer</span>
          <span class="state">{replayOn ? "On" : "Off"}</span>
        </button>

        <!-- Save replay -->
        <button class="tile" disabled={!replayOn} onclick={() => invoke("save_replay")}>
          <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
            <path d="M12 4v11M7 10l5 5 5-5M5 20h14" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span class="name">Save last {s.replay_seconds < 60 ? `${s.replay_seconds}s` : `${s.replay_seconds / 60} min`}</span>
          <kbd>{hotkeyLabel(s.hotkey_replay)}</kbd>
        </button>
      </div>

      <footer>
        <button class="link" onclick={() => invoke("overlay_open_main")}>Open Orbit →</button>
      </footer>

      {#if s.status_bar}
        <div class="stats"><StatusBar compact /></div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .dim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
  }
  .wrap {
    position: fixed;
    left: 0;
    right: 0;
    top: 6vh;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }
  .panel {
    width: min(760px, 92vw);
    border: 1px solid var(--border-strong);
    border-radius: 16px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.55);
    pointer-events: auto;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px 10px 18px;
  }
  header img {
    border-radius: 6px;
  }
  .title {
    font-size: 15px;
    font-weight: 600;
  }
  .hint {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-faint);
  }
  .close {
    display: flex;
    padding: 6px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .close:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    padding: 6px 16px 14px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 118px;
    padding: 14px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
    transition:
      border-color var(--fast) var(--ease),
      background var(--fast) var(--ease),
      transform var(--fast) var(--ease);
  }
  .tile:hover {
    border-color: var(--hover-border);
  }
  .tile:active {
    transform: scale(0.97);
  }
  .tile:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  .tile.on,
  .tile.live {
    border-color: color-mix(in srgb, var(--rec) 55%, transparent);
    background: var(--rec-soft);
  }
  .name {
    font-size: 13px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .state {
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .rec-dot {
    width: 16px;
    height: 16px;
    margin: 5px;
    border-radius: 50%;
    background: var(--rec);
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--rec) 25%, transparent);
  }
  kbd {
    padding: 1px 6px;
    border: 1px solid var(--border-strong);
    border-radius: 5px;
    color: var(--text-muted);
    font-family: inherit;
    font-size: 11px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    padding: 0 18px 12px;
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 12.5px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }
  .stats {
    border-top: 1px solid var(--border);
    padding: 2px 6px;
  }
</style>
