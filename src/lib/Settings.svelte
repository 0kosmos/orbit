<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // Must match the Settings struct in src-tauri/src/settings.rs
  type Settings = {
    replay_seconds: number;
    replay_autostart: boolean;
    sounds: boolean;
    system_audio: boolean;
  };

  const lengths = [
    { seconds: 30, label: "30 seconds" },
    { seconds: 60, label: "1 minute" },
    { seconds: 120, label: "2 minutes" },
    { seconds: 300, label: "5 minutes" },
    { seconds: 600, label: "10 minutes" },
  ];

  let settings = $state<Settings | null>(null);
  let error = $state("");

  invoke<Settings>("get_settings").then((s) => (settings = s));

  // Save right away whenever something changes.
  async function save() {
    try {
      await invoke("set_settings", { new: settings });
      error = "";
    } catch (e) {
      error = String(e);
    }
  }
</script>

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
      <span>Record game / PC sound</span>
    </label>
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
    max-width: 480px;
    margin-bottom: 28px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
  }
  select {
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
  .note {
    margin: -6px 0 0;
    font-size: 12px;
    color: #666;
  }
  .error {
    color: #ff9a9a;
  }
</style>
