<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { hotkeyFromEvent, hotkeyLabel } from "$lib/hotkey";
  import Segmented from "$lib/Segmented.svelte";
  import { applyUi, ms } from "$lib/ui.svelte";
  import { fly } from "svelte/transition";

  // Must match the Settings struct in src-tauri/src/settings.rs
  type Settings = {
    replay_seconds: number;
    replay_autostart: boolean;
    sounds: boolean;
    system_audio: boolean;
    system_device: string | null;
    mic_enabled: boolean;
    mic_device: string | null;
    hotkey_screenshot: string;
    hotkey_record: string;
    hotkey_replay: string;
    video_fps: number;
    video_quality: string;
    encoder_preset: string;
    draw_mouse: boolean;
    audio_bitrate: number;
    screenshot_format: string;
    theme: string;
    animations: boolean;
    blur: boolean;
    close_to_tray: boolean;
    save_dir: string | null;
    status_bar: boolean;
    start_with_windows: boolean;
  };
  type HotkeyField = "hotkey_screenshot" | "hotkey_record" | "hotkey_replay";
  // Must match DeviceList in src-tauri/src/audio.rs
  type DeviceList = { default_name: string | null; devices: { id: string; name: string }[] };
  type Kind = "game" | "mic";

  const fpsOptions = [30, 60, 90, 120, 144].map((v) => ({ value: v, label: String(v) }));
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
  const audioOptions = [128, 160, 192, 256].map((v) => ({ value: v, label: `${v} kbps` }));
  const themeOptions = [
    { value: "black", label: "Black" },
    { value: "grey", label: "Dark grey" },
    { value: "light", label: "Light" },
  ];
  const formatOptions = [
    { value: "png", label: "PNG" },
    { value: "jpg", label: "JPG" },
  ];

  // Replay length slider: these steps (seconds), from 15 seconds to 20 minutes.
  const replayLengths = [15, 30, 45, 60, 90, 120, 180, 240, 300, 420, 600, 900, 1200];
  // The slider moves over the list position; the setting stores seconds.
  let replayIndex = $state(5);
  function closestLength(seconds: number) {
    let best = 0;
    replayLengths.forEach((v, i) => {
      if (Math.abs(v - seconds) < Math.abs(replayLengths[best] - seconds)) best = i;
    });
    return best;
  }
  function lengthLabel(seconds: number) {
    if (seconds < 60) return `${seconds} seconds`;
    const m = seconds / 60;
    return m === 1 ? "1 minute" : `${m} minutes`;
  }

  let settings = $state<Settings | null>(null);
  // Settings are split into categories; only the chosen one is shown.
  type Category = "general" | "capture" | "audio" | "replay" | "hotkeys" | "storage";
  let cat = $state<Category>("general");
  const categories: { id: Category; label: string; icon: string }[] = [
    { id: "general", label: "General", icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" },
    { id: "capture", label: "Video & screenshots", icon: "M15 10l5-3v10l-5-3M3 7h12v10H3z" },
    { id: "audio", label: "Audio", icon: "M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3zM5 11a7 7 0 0 0 14 0M12 18v3" },
    { id: "replay", label: "Replay buffer", icon: "M3 12a9 9 0 1 0 3-6.7M3 4v5h5M12 8v4l3 2" },
    { id: "hotkeys", label: "Hotkeys", icon: "M3 7h18v10H3zM7 11h.01M11 11h.01M15 11h.01M8 14h8" },
    { id: "storage", label: "Storage & system", icon: "M4 6h16M4 6v12h16V6M9 10h6" },
  ];

  // Rough disk use of the buffer per minute at each quality (game footage, 60 fps).
  let mbPerMinute = $derived(
    (({ low: 40, medium: 70, high: 110, very_high: 180 }) as Record<string, number>)[
      settings?.video_quality ?? "high"
    ] * ((settings?.video_fps ?? 60) / 60),
  );
  let error = $state("");
  let outputs = $state<DeviceList>({ default_name: null, devices: [] });
  let mics = $state<DeviceList>({ default_name: null, devices: [] });

  invoke<Settings>("get_settings").then((s) => {
    settings = s;
    replayIndex = closestLength(s.replay_seconds);
  });
  // Asked once each time Settings opens (so a newly plugged-in device shows up).
  invoke<DeviceList>("list_audio_devices", { outputs: true }).then((d) => (outputs = d));
  invoke<DeviceList>("list_audio_devices", { outputs: false }).then((d) => (mics = d));

  // Save right away whenever something changes.
  // Theme / animations / blur: show the change right away, then save.
  function saveLook() {
    if (settings) applyUi(settings);
    save();
  }

  // --- Save folder ---
  let folder = $state("");
  let defaultFolder = $state("");
  function loadFolders() {
    invoke<[string, string]>("save_folders").then(([current, def]) => {
      folder = current;
      defaultFolder = def;
    });
  }
  loadFolders();

  async function setFolder(dir: string | null) {
    if (!settings) return;
    const old = settings.save_dir;
    settings.save_dir = dir;
    try {
      await invoke("set_settings", { new: settings });
      error = "";
    } catch (e) {
      settings.save_dir = old;
      error = String(e);
    }
    loadFolders();
  }

  async function chooseFolder() {
    const dir = await invoke<string | null>("pick_folder");
    if (dir) setFolder(dir);
  }

  // "Start with Windows" can fail (e.g. blocked by Windows): then flip the switch back.
  async function saveStartup() {
    try {
      await invoke("set_settings", { new: settings });
      error = "";
    } catch (e) {
      settings!.start_with_windows = !settings!.start_with_windows;
      error = String(e);
    }
  }

  async function save() {
    try {
      await invoke("set_settings", { new: settings });
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  // --- Sound test: a live level bar while you talk / play something ---
  let testing = $state<Kind | null>(null);
  let level = $state(0);
  let testError = $state("");

  function deviceOf(kind: Kind) {
    return kind === "mic" ? settings!.mic_device : settings!.system_device;
  }

  function startTest(kind: Kind) {
    testing = kind;
    level = 0;
    testError = "";
    invoke("start_audio_test", { kind, device: deviceOf(kind) });
  }

  function toggleTest(kind: Kind) {
    if (testing === kind) invoke("stop_audio_test");
    else startTest(kind);
  }

  // Picking another device during a test: switch the test to it.
  function deviceChanged(kind: Kind) {
    save();
    if (testing === kind) startTest(kind);
  }

  $effect(() => {
    const stops = [
      listen<number>("audio-level", (e) => (level = e.payload)),
      listen<string>("audio-level-error", (e) => (testError = e.payload)),
      listen("audio-level-stopped", () => {
        testing = null;
        level = 0;
      }),
    ];
    return () => {
      // Leaving Settings ends the test.
      invoke("stop_audio_test");
      stops.forEach((p) => p.then((stop) => stop()));
    };
  });

  // --- Hotkeys: click one, then press the new key combination ---
  const hotkeyRows: { field: HotkeyField; label: string }[] = [
    { field: "hotkey_screenshot", label: "Take screenshot" },
    { field: "hotkey_record", label: "Start / stop recording" },
    { field: "hotkey_replay", label: "Save replay (last X minutes)" },
  ];
  let capturing = $state<HotkeyField | null>(null);
  let hotkeyError = $state("");

  function startCapture(field: HotkeyField) {
    capturing = field;
    hotkeyError = "";
    // Hotkeys off meanwhile, so pressing e.g. Alt+F12 here doesn't take a screenshot.
    invoke("pause_hotkeys", { paused: true });
  }

  function endCapture() {
    capturing = null;
    invoke("pause_hotkeys", { paused: false });
  }

  async function onKeydown(e: KeyboardEvent) {
    if (!capturing || !settings) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.code === "Escape") {
      endCapture();
      return;
    }
    const result = hotkeyFromEvent(e);
    if (result === null) return; // only Ctrl/Alt/Shift held so far
    if (typeof result !== "string") {
      hotkeyError = result.error;
      return;
    }
    const field = capturing;
    const old = settings[field];
    settings[field] = result;
    capturing = null;
    try {
      await invoke("set_settings", { new: settings });
      hotkeyError = "";
    } catch (err) {
      settings[field] = old;
      hotkeyError = String(err);
    }
    invoke("pause_hotkeys", { paused: false });
  }

  function resetHotkeys() {
    if (!settings) return;
    settings.hotkey_screenshot = "Alt+F12";
    settings.hotkey_record = "Alt+F9";
    settings.hotkey_replay = "Alt+F10";
    hotkeyError = "";
    invoke("set_settings", { new: settings }).catch((err) => (hotkeyError = String(err)));
  }

  // Leaving Settings while choosing a hotkey: turn hotkeys back on.
  $effect(() => () => {
    if (capturing) invoke("pause_hotkeys", { paused: false });
  });

  // Show the level on a decibel scale (-60 dB .. 0 dB), like a mixer.
  let barPercent = $derived(
    level <= 0 ? 0 : Math.max(0, Math.min(100, ((20 * Math.log10(level) + 60) / 60) * 100)),
  );
</script>

{#snippet devicePicker(kind: Kind, list: DeviceList)}
  <div class="sub">
    <div class="device">
      {#if kind === "mic"}
        <select class="select" bind:value={settings!.mic_device} onchange={() => deviceChanged(kind)}>
          <option value={null}>Windows default ({list.default_name ?? "none"})</option>
          {#each list.devices as d}
            <option value={d.id}>{d.name}</option>
          {/each}
        </select>
      {:else}
        <select class="select" bind:value={settings!.system_device} onchange={() => deviceChanged(kind)}>
          <option value={null}>Windows default ({list.default_name ?? "none"})</option>
          {#each list.devices as d}
            <option value={d.id}>{d.name}</option>
          {/each}
        </select>
      {/if}
      <button class="btn" class:testing={testing === kind} onclick={() => toggleTest(kind)}>
        {testing === kind ? "Stop" : "Test"}
      </button>
    </div>
    {#if testing === kind}
      <div class="meter"><div class="fill" style:width="{barPercent}%"></div></div>
      <p class="desc">
        {kind === "mic"
          ? "Talk into your mic: the bar should move."
          : "Play something (a video, a game): the bar should move. If it doesn't, try another device."}
      </p>
    {/if}
  </div>
{/snippet}

<svelte:window onkeydown={onKeydown} />

<h1>Settings</h1>

<div class="layout">
  <!-- Categories -->
  <nav class="cats">
    {#each categories as c}
      <button class="cat" class:active={cat === c.id} onclick={() => (cat = c.id)}>
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <path d={c.icon} fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        {c.label}
      </button>
    {/each}
  </nav>

  <div class="panel">
    {#if settings}
    {#key cat}
    <div in:fly={{ y: 6, duration: ms(180) }}>
      {#if cat === "general"}
        <!-- Appearance -->
        <section class="card">
          <h2>Appearance</h2>

          <div class="row">
            <div class="label"><span>Theme</span></div>
            <Segmented options={themeOptions} bind:value={settings.theme} onchange={saveLook} />
          </div>

          <label class="row">
            <div class="label">
              <span>Animations</span>
              <span class="desc">Short, smooth motion when you click or open things. Off = everything instant.</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.animations} onchange={saveLook} />
          </label>

          <label class="row">
            <div class="label">
              <span>Blur &amp; transparency</span>
              <span class="desc">Frosted glass behind the viewer, notifications and filter bar.</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.blur} onchange={saveLook} />
          </label>

          <label class="row">
            <div class="label">
              <span>Status bar</span>
              <span class="desc">Version, CPU / GPU / RAM and time at the bottom. Measures every 2 s, only while this window is open.</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.status_bar} onchange={save} />
          </label>
        </section>
        <!-- Feedback -->
        <section class="card">
          <h2>Feedback</h2>

          <label class="row">
            <div class="label">
              <span>Sounds</span>
              <span class="desc">A short beep when saving, starting a recording, or on errors.</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.sounds} onchange={save} />
          </label>
        </section>
        <!-- System -->
        <section class="card">
          <h2>System</h2>

          <label class="row">
            <div class="label">
              <span>Keep running in the tray when closed</span>
              <span class="desc">
                Closing the window frees its memory; hotkeys, recording and the replay buffer keep
                working. Open Orbit again from the icon next to the clock.
              </span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.close_to_tray} onchange={save} />
          </label>

          <label class="row">
            <div class="label">
              <span>Start with Windows</span>
              <span class="desc">Starts quietly in the tray, without opening the window.</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.start_with_windows} onchange={saveStartup} />
          </label>
        </section>
      {/if}
      {#if cat === "capture"}
        <!-- Video -->
        <section class="card">
          <h2>Video</h2>

          <div class="row">
            <div class="label">
              <span>Frames per second</span>
              <span class="desc">60 is smooth and light. Higher needs more GPU and disk.</span>
            </div>
            <Segmented options={fpsOptions} bind:value={settings.video_fps} onchange={save} />
          </div>

          <div class="row">
            <div class="label">
              <span>Quality</span>
              <span class="desc">Higher = bigger files. "High" is a good balance.</span>
            </div>
            <Segmented options={qualityOptions} bind:value={settings.video_quality} onchange={save} />
          </div>

          <div class="row">
            <div class="label">
              <span>Encoder speed</span>
              <span class="desc">"Fastest" is lightest on your GPU (best for in-game FPS).</span>
            </div>
            <Segmented options={presetOptions} bind:value={settings.encoder_preset} onchange={save} />
          </div>

          <label class="row">
            <div class="label"><span>Show mouse cursor</span></div>
            <input type="checkbox" class="switch" bind:checked={settings.draw_mouse} onchange={save} />
          </label>
        </section>
        <!-- Screenshots -->
        <section class="card">
          <h2>Screenshots</h2>

          <div class="row">
            <div class="label">
              <span>Format</span>
              <span class="desc">PNG is lossless. JPG is usually much smaller for games.</span>
            </div>
            <Segmented options={formatOptions} bind:value={settings.screenshot_format} onchange={save} />
          </div>
        </section>
        <p class="footnote">
          Recording changes apply to the next recording. A running replay buffer restarts by itself
          (its saved time starts over).
        </p>
      {/if}
      {#if cat === "audio"}
        <!-- Audio -->
        <section class="card">
          <h2>Audio</h2>

          <label class="row">
            <div class="label"><span>Record game / PC sound</span></div>
            <input type="checkbox" class="switch" bind:checked={settings.system_audio} onchange={save} />
          </label>
          {#if settings.system_audio}
            {@render devicePicker("game", outputs)}
          {/if}

          <label class="row">
            <div class="label">
              <span>Record microphone</span>
              <span class="desc">Mixed into the main track, plus its own track for editing.</span>
            </div>
            <input type="checkbox" class="switch" bind:checked={settings.mic_enabled} onchange={save} />
          </label>
          {#if settings.mic_enabled}
            {@render devicePicker("mic", mics)}
          {/if}

          {#if testError}
            <p class="error">Test failed: {testError}</p>
          {/if}

          <div class="row">
            <div class="label"><span>Sound quality</span></div>
            <Segmented options={audioOptions} bind:value={settings.audio_bitrate} onchange={save} />
          </div>
        </section>
        <p class="footnote">
          Recording changes apply to the next recording. A running replay buffer restarts by itself
          (its saved time starts over).
        </p>
      {/if}
      {#if cat === "replay"}
        <!-- Replay buffer -->
        <section class="card">
          <h2>Replay buffer</h2>

          <div class="row">
            <div class="label"><span>Save the last</span></div>
            <strong class="value">{lengthLabel(replayLengths[replayIndex])}</strong>
          </div>
          <input
            class="range"
            type="range"
            min="0"
            max={replayLengths.length - 1}
            step="1"
            bind:value={replayIndex}
            oninput={() => (settings!.replay_seconds = replayLengths[replayIndex])}
            onchange={save}
          />
          <p class="desc">
            Uses roughly {Math.max(1, Math.round((replayLengths[replayIndex] / 60) * mbPerMinute))} MB of
            temporary disk space. Same CPU and GPU at any length.
          </p>

          <label class="row">
            <div class="label"><span>Turn on when Orbit opens</span></div>
            <input type="checkbox" class="switch" bind:checked={settings.replay_autostart} onchange={save} />
          </label>
        </section>
        <p class="footnote">
          Recording changes apply to the next recording. A running replay buffer restarts by itself
          (its saved time starts over).
        </p>
      {/if}
      {#if cat === "hotkeys"}
        <!-- Hotkeys -->
        <section class="card">
          <h2>Hotkeys</h2>

          {#each hotkeyRows as row}
            <div class="row">
              <div class="label"><span>{row.label}</span></div>
              <button
                class="hotkey"
                class:capturing={capturing === row.field}
                onclick={() => (capturing === row.field ? endCapture() : startCapture(row.field))}
              >
                {capturing === row.field ? "Press keys… (Esc cancels)" : hotkeyLabel(settings[row.field])}
              </button>
            </div>
          {/each}

          {#if hotkeyError}
            <p class="error">{hotkeyError}</p>
          {/if}
          <button class="btn ghost small" onclick={resetHotkeys}>Reset to defaults</button>
        </section>
      {/if}
      {#if cat === "storage"}
        <!-- Storage -->
        <section class="card">
          <h2>Storage</h2>

          <div class="row">
            <div class="label">
              <span>Save folder</span>
              <span class="desc path" title={folder}>{folder}</span>
            </div>
            <div class="folder-buttons">
              <button class="btn" onclick={chooseFolder}>Change…</button>
              <button class="btn ghost" onclick={() => invoke("open_media_folder")}>Open</button>
            </div>
          </div>
          {#if settings.save_dir}
            <button class="btn ghost small" onclick={() => setFolder(null)}>
              Reset to default ({defaultFolder})
            </button>
          {/if}
          <p class="desc folder-note">
            Screenshots, Clips and Recordings folders are made inside it. Files already saved stay
            where they are (move them yourself in Explorer if you want them in the new folder).
          </p>
        </section>
      {/if}
    </div>
    {/key}
    {/if}

    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>
</div>

<style>
  /* Two columns: categories on the left, the chosen settings on the right. */
  .layout {
    display: grid;
    grid-template-columns: 200px minmax(0, 680px);
    gap: 28px;
    align-items: start;
  }
  .cats {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .cat {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .cat:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .cat.active {
    background: var(--surface-2);
    color: var(--text);
  }
  /* Narrow window: categories become a row above the settings. */
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
      gap: 14px;
    }
    .cats {
      position: static;
      flex-direction: row;
      flex-wrap: wrap;
    }
  }
  h1 {
    margin: 0 0 20px;
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.3px;
  }
  .card {
    margin-bottom: 16px;
    padding: 18px 20px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }
  h2 {
    margin: 0 0 10px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.8px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px 16px;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  h2 + .row {
    border-top: none;
  }
  label.row {
    cursor: pointer;
  }
  .label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1 1 220px;
  }
  .desc {
    margin: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .card > .desc {
    margin: -4px 0 10px;
  }
  .value {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .small {
    margin: 4px 0 10px;
    padding: 4px 8px;
    font-size: 12px;
  }

  /* Device picker under an audio switch */
  .sub {
    padding: 0 0 12px;
  }
  .device {
    display: flex;
    gap: 8px;
  }
  .device .select {
    flex: 1;
  }
  .btn.testing {
    border-color: var(--accent);
    color: var(--text);
  }
  .meter {
    height: 6px;
    margin: 10px 0 6px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, var(--ok), #a3e635 70%, #facc15);
  }

  /* Hotkey buttons */
  .hotkey {
    min-width: 180px;
    padding: 6px 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    color: var(--text);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    transition:
      border-color var(--fast) var(--ease),
      background var(--fast) var(--ease);
  }
  .hotkey:hover {
    background: var(--surface-hover);
  }
  .hotkey.capturing {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
  .folder-buttons {
    display: flex;
    gap: 6px;
  }
  .folder-note {
    padding-bottom: 10px;
  }
  .footnote {
    max-width: 640px;
    font-size: 12px;
    color: var(--text-faint);
  }
</style>
