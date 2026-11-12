<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { hotkeyFromEvent, hotkeyLabel } from "$lib/hotkey";
  import Segmented from "$lib/Segmented.svelte";

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
  <div class="row indent">
    {#if kind === "mic"}
      <select bind:value={settings!.mic_device} onchange={() => deviceChanged(kind)}>
        <option value={null}>Windows default ({list.default_name ?? "none"})</option>
        {#each list.devices as d}
          <option value={d.id}>{d.name}</option>
        {/each}
      </select>
    {:else}
      <select bind:value={settings!.system_device} onchange={() => deviceChanged(kind)}>
        <option value={null}>Windows default ({list.default_name ?? "none"})</option>
        {#each list.devices as d}
          <option value={d.id}>{d.name}</option>
        {/each}
      </select>
    {/if}
    <button class="test" class:on={testing === kind} onclick={() => toggleTest(kind)}>
      {testing === kind ? "Stop" : "Test"}
    </button>
  </div>
  {#if testing === kind}
    <div class="meter indent"><div class="fill" style:width="{barPercent}%"></div></div>
    <p class="note indent">
      {kind === "mic"
        ? "Talk into your mic: the bar should move."
        : "Play something (a video, a game): the bar should move. If it doesn't, try another device."}
    </p>
  {/if}
{/snippet}

<svelte:window onkeydown={onKeydown} />

<h1>Settings</h1>

{#if settings}
  <section>
    <h2>Hotkeys</h2>

    {#each hotkeyRows as row}
      <div class="row hotkey-row">
        <span>{row.label}</span>
        <button
          class="hotkey"
          class:capturing={capturing === row.field}
          onclick={() => (capturing === row.field ? endCapture() : startCapture(row.field))}
        >
          {capturing === row.field ? "Press keys… (Esc to cancel)" : hotkeyLabel(settings[row.field])}
        </button>
      </div>
    {/each}

    {#if hotkeyError}
      <p class="error">{hotkeyError}</p>
    {/if}
    <button class="link" onclick={resetHotkeys}>Reset to defaults</button>
  </section>

  <section>
    <h2>Video</h2>

    <div class="row setting">
      <span>Frames per second</span>
      <Segmented options={fpsOptions} bind:value={settings.video_fps} onchange={save} />
    </div>
    <p class="note">
      60 is smooth and light. Higher needs more GPU and disk, and only helps on a
      high-refresh monitor.
    </p>

    <div class="row setting">
      <span>Quality</span>
      <Segmented options={qualityOptions} bind:value={settings.video_quality} onchange={save} />
    </div>
    <p class="note">Higher quality = bigger files. "High" is a good balance.</p>

    <div class="row setting">
      <span>Encoder speed</span>
      <Segmented options={presetOptions} bind:value={settings.encoder_preset} onchange={save} />
    </div>
    <p class="note">
      "Fastest" puts the least load on your GPU (best for FPS in games), "Quality" makes
      slightly better video for the same file size.
    </p>

    <label class="row">
      <input type="checkbox" bind:checked={settings.draw_mouse} onchange={save} />
      <span>Show the mouse cursor in recordings and screenshots</span>
    </label>
  </section>

  <section>
    <h2>Replay buffer</h2>

    <div class="row setting">
      <span>Save the last</span>
      <strong>{lengthLabel(replayLengths[replayIndex])}</strong>
    </div>
    <input
      class="slider"
      type="range"
      min="0"
      max={replayLengths.length - 1}
      step="1"
      bind:value={replayIndex}
      oninput={() => (settings!.replay_seconds = replayLengths[replayIndex])}
      onchange={save}
    />
    <p class="note">
      Longer = more temporary disk space (roughly {Math.max(
        1,
        Math.round((replayLengths[replayIndex] / 60) * mbPerMinute),
      )} MB), same CPU and GPU.
    </p>

    <label class="row">
      <input type="checkbox" bind:checked={settings.replay_autostart} onchange={save} />
      <span>Turn on the replay buffer when Orbit opens</span>
    </label>
  </section>

  <section>
    <h2>Audio</h2>

    <label class="row">
      <input type="checkbox" bind:checked={settings.system_audio} onchange={save} />
      <span>Record game / PC sound from</span>
    </label>
    {#if settings.system_audio}
      {@render devicePicker("game", outputs)}
    {/if}

    <label class="row">
      <input type="checkbox" bind:checked={settings.mic_enabled} onchange={save} />
      <span>Record microphone (as a separate track)</span>
    </label>
    {#if settings.mic_enabled}
      {@render devicePicker("mic", mics)}
    {/if}

    {#if testError}
      <p class="error indent">Test failed: {testError}</p>
    {/if}

    <div class="row setting">
      <span>Sound quality</span>
      <Segmented options={audioOptions} bind:value={settings.audio_bitrate} onchange={save} />
    </div>
  </section>

  <section>
    <h2>Screenshots</h2>

    <div class="row setting">
      <span>Format</span>
      <Segmented options={formatOptions} bind:value={settings.screenshot_format} onchange={save} />
    </div>
    <p class="note">
      PNG is lossless. JPG is usually several times smaller for games (but not for plain
      desktop screenshots).
    </p>
  </section>

  <p class="note">
    Changes apply to the next recording. A running replay buffer restarts by itself (its
    saved time starts over).
  </p>

  <section>
    <h2>Feedback</h2>

    <label class="row">
      <input type="checkbox" bind:checked={settings.sounds} onchange={save} />
      <span>Play a short sound when saving, starting a recording, or on errors</span>
    </label>
  </section>
{/if}

{#if error}
  <p class="error">{error}</p>
{/if}

<style>
  h1 {
    margin: 0 0 20px;
    font-size: 20px;
    font-weight: 600;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 13px;
    font-weight: 600;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  section {
    max-width: 520px;
    margin-bottom: 28px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
  }
  select {
    min-width: 0;
    flex: 1;
    max-width: 360px;
    padding: 5px 8px;
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    background: #111;
    color: #e6e6e6;
    font: inherit;
  }
  input[type="checkbox"] {
    accent-color: #e53935;
  }
  .indent {
    margin-left: 24px;
  }
  .test {
    padding: 5px 12px;
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    background: transparent;
    color: #ccc;
    font: inherit;
    cursor: pointer;
  }
  .test:hover {
    color: #fff;
    background: #141414;
  }
  .test.on {
    border-color: #6b2222;
    color: #fff;
  }
  .meter {
    height: 6px;
    max-width: 360px;
    margin-bottom: 8px;
    border-radius: 3px;
    background: #1a1a1a;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: #3fbf5f;
  }
  .note {
    margin: -6px 0 12px;
    font-size: 12px;
    color: #666;
  }
  .meter + .note {
    margin-top: 0;
  }
  .error {
    color: #ff9a9a;
  }
  .setting {
    justify-content: space-between;
    flex-wrap: wrap;
  }
  .slider {
    width: 100%;
    margin: 0 0 12px;
    accent-color: #e53935;
  }
  .hotkey-row {
    justify-content: space-between;
  }
  .hotkey {
    min-width: 170px;
    padding: 5px 12px;
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    background: #111;
    color: #e6e6e6;
    font: inherit;
    cursor: pointer;
  }
  .hotkey:hover {
    border-color: #444;
  }
  .hotkey.capturing {
    border-color: #e53935;
    color: #fff;
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: #888;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .link:hover {
    color: #fff;
  }
</style>
