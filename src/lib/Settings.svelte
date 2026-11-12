<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // Must match the Settings struct in src-tauri/src/settings.rs
  type Settings = {
    replay_seconds: number;
    replay_autostart: boolean;
    sounds: boolean;
    system_audio: boolean;
    system_device: string | null;
    mic_enabled: boolean;
    mic_device: string | null;
  };
  // Must match DeviceList in src-tauri/src/audio.rs
  type DeviceList = { default_name: string | null; devices: { id: string; name: string }[] };
  type Kind = "game" | "mic";

  const lengths = [
    { seconds: 30, label: "30 seconds" },
    { seconds: 60, label: "1 minute" },
    { seconds: 120, label: "2 minutes" },
    { seconds: 300, label: "5 minutes" },
    { seconds: 600, label: "10 minutes" },
  ];

  let settings = $state<Settings | null>(null);
  let error = $state("");
  let outputs = $state<DeviceList>({ default_name: null, devices: [] });
  let mics = $state<DeviceList>({ default_name: null, devices: [] });

  invoke<Settings>("get_settings").then((s) => (settings = s));
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

<h1>Settings</h1>

{#if settings}
  <section>
    <h2>Replay buffer</h2>

    <label class="row">
      <span>Save the last</span>
      <select bind:value={settings.replay_seconds} onchange={save}>
        {#each lengths as l}
          <option value={l.seconds}>{l.label}</option>
        {/each}
      </select>
    </label>

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

    <p class="note">
      Applies to the next recording. If the replay buffer is on, turn it off and on again.
    </p>
  </section>

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
</style>
