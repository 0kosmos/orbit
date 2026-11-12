<script lang="ts">
  // The in-game overlay. Lives in its own see-through window over the game; Rust
  // creates it when you press the overlay hotkey and destroys it when it closes.
  import { tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fly, fade } from "svelte/transition";
  import StatusBar from "$lib/StatusBar.svelte";
  import Segmented from "$lib/Segmented.svelte";
  import { hotkeyLabel } from "$lib/hotkey";
  import { applyUi, ms } from "$lib/ui.svelte";

  type S = {
    theme: string;
    animations: boolean;
    blur: boolean;
    status_bar: boolean;
    replay_seconds: number;
    hotkey_screenshot: string;
    hotkey_snip: string;
    hotkey_record: string;
    hotkey_replay: string;
    hotkey_overlay: string;
    video_fps: number;
    video_quality: string;
    encoder_preset: string;
    system_audio: boolean;
    mic_enabled: boolean;
    draw_mouse: boolean;
    sounds: boolean;
    // (all other settings are passed back unchanged when saving)
    [key: string]: unknown;
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
    invoke<S>("get_settings").then(async (v) => {
      s = v;
      replayIndex = closest(v.replay_seconds);
      applyUi(v);
      shown = true;
      // The window is still invisible: show it once the panel is on the page.
      await tick();
      invoke("overlay_ready");
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

  // --- Quick settings (the "Settings" button) ---
  let showSettings = $state(false);
  let saveError = $state("");
  const lengths = [15, 30, 45, 60, 90, 120, 180, 240, 300, 420, 600, 900, 1200];
  let replayIndex = $state(5);
  function closest(sec: number) {
    let best = 0;
    lengths.forEach((v, i) => {
      if (Math.abs(v - sec) < Math.abs(lengths[best] - sec)) best = i;
    });
    return best;
  }
  const lengthLabel = (sec: number) => (sec < 60 ? `${sec} s` : `${sec / 60} min`);
  const fpsOptions = [30, 60, 120].map((v) => ({ value: v, label: String(v) }));
  const qualityOptions = [
    { value: "low", label: "Low" },
    { value: "medium", label: "Medium" },
    { value: "high", label: "High" },
    { value: "very_high", label: "Very high" },
  ];
  const presetOptions = [
    { value: "fastest", label: "Fastest" },
    { value: "balanced", label: "Balanced" },
    { value: "quality", label: "Quality" },
  ];
  async function save() {
    try {
      await invoke("set_settings", { new: s });
      saveError = "";
    } catch (e) {
      saveError = String(e);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- Slightly dimmed game behind; clicking it closes the overlay. -->
<div class="dim" role="presentation" onclick={close} transition:fade={{ duration: ms(150) }}></div>

{#if shown && s}
  <div class="wrap">
    <div class="panel" in:fly={{ y: -18, duration: ms(240) }}>
      <header>
        <img src="/logo.png" alt="" width="22" height="22" />
        <span class="title">Orbit</span>
        <span class="hint">{hotkeyLabel(s.hotkey_overlay)} or Esc to close</span>
        <button class="head-btn" class:on={showSettings} onclick={() => (showSettings = !showSettings)}>
          <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
            <path d="M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M6 14v6" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" />
          </svg>
          Settings
        </button>
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

        <!-- Region screenshot (snip) -->
        <button class="tile" onclick={() => invoke("overlay_snip")}>
          <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
            <path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
            <rect x="8.5" y="8.5" width="7" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.7" stroke-dasharray="2 2" />
          </svg>
          <span class="name">Snip</span>
          <kbd>{hotkeyLabel(s.hotkey_snip)}</kbd>
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

      {#if showSettings}
        <!-- Quick settings: the important ones, saved right away -->
        <div class="quick" transition:fly={{ y: -6, duration: ms(180) }}>
          <div class="q-row wide">
            <span class="q-label">Replay length</span>
            <strong class="q-value">{lengthLabel(lengths[replayIndex])}</strong>
            <input
              class="range"
              type="range"
              min="0"
              max={lengths.length - 1}
              bind:value={replayIndex}
              oninput={() => (s!.replay_seconds = lengths[replayIndex])}
              onchange={save}
            />
          </div>
          <div class="q-row">
            <span class="q-label">FPS</span>
            <Segmented options={fpsOptions} bind:value={s.video_fps} onchange={save} />
          </div>
          <div class="q-row">
            <span class="q-label">Quality</span>
            <Segmented options={qualityOptions} bind:value={s.video_quality} onchange={save} />
          </div>
          <div class="q-row">
            <span class="q-label">Encoder</span>
            <Segmented options={presetOptions} bind:value={s.encoder_preset} onchange={save} />
          </div>
          <div class="switches">
            <label><input type="checkbox" class="switch" bind:checked={s.system_audio} onchange={save} /> Game sound</label>
            <label><input type="checkbox" class="switch" bind:checked={s.mic_enabled} onchange={save} /> Microphone</label>
            <label><input type="checkbox" class="switch" bind:checked={s.draw_mouse} onchange={save} /> Cursor</label>
            <label><input type="checkbox" class="switch" bind:checked={s.sounds} onchange={save} /> Sounds</label>
          </div>
          <div class="q-foot">
            {#if saveError}<span class="error">{saveError}</span>{:else}<span class="faint">Saved automatically. A running replay buffer restarts.</span>{/if}
            <button class="link" onclick={() => invoke("overlay_open_settings")}>All settings →</button>
          </div>
        </div>
      {/if}

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
  /* Solid (no blur): in a see-through window blur would only cost GPU while gaming. */
  .panel {
    background: color-mix(in srgb, var(--surface) 96%, transparent);
    width: min(860px, 94vw);
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
    margin-right: 6px;
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
    grid-template-columns: repeat(5, 1fr);
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

  .head-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: transparent;
    color: var(--text-muted);
    font-size: 12.5px;
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .head-btn:hover,
  .head-btn.on {
    background: var(--surface-hover);
    color: var(--text);
  }

  .quick {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 22px;
    margin: 0 16px 14px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
  }
  .q-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .q-row.wide {
    grid-column: 1 / -1;
    flex-wrap: wrap;
  }
  .q-row.wide .range {
    flex-basis: 100%;
    margin: 2px 0 0;
  }
  .q-label {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .q-value {
    margin-left: auto;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .switches {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: 10px 22px;
  }
  .switches label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    cursor: pointer;
  }
  .q-foot {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 11.5px;
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
