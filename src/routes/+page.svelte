<script lang="ts">
  import Gallery from "$lib/Gallery.svelte";
  import Settings from "$lib/Settings.svelte";
  import Toast from "$lib/Toast.svelte";
  import ToggleButton from "$lib/ToggleButton.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fly } from "svelte/transition";
  import { hotkeyLabel } from "$lib/hotkey";
  import { applyUi, ms } from "$lib/ui.svelte";

  type Tab = "gallery" | "settings";
  let tab = $state<Tab>("gallery");

  // The settings the shell needs: hotkey hints on the sidebar buttons and the look.
  // Loaded once, then updated only when Settings changes something (no polling).
  type ShellSettings = {
    hotkey_record: string;
    hotkey_replay: string;
    theme: string;
    animations: boolean;
    blur: boolean;
  };
  let keys = $state({ hotkey_record: "Alt+F9", hotkey_replay: "Alt+F10" });
  function use(s: ShellSettings) {
    keys = s;
    applyUi(s);
  }
  $effect(() => {
    invoke<ShellSettings>("get_settings").then(use);
    const stop = listen<ShellSettings>("settings-changed", (e) => use(e.payload));
    return () => stop.then((s) => s());
  });

  const tabs: { id: Tab; label: string; icon: string }[] = [
    {
      id: "gallery",
      label: "Gallery",
      // 4 squares
      icon: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
    },
    {
      id: "settings",
      label: "Settings",
      // sliders
      icon: "M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M6 14v6",
    },
  ];
</script>

<div class="app">
  <nav class="sidebar">
    <div class="logo">
      <!-- Orbit logo: a planet with a ring -->
      <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
        <circle cx="12" cy="12" r="5" fill="var(--accent)" />
        <ellipse
          cx="12"
          cy="12"
          rx="10"
          ry="4"
          fill="none"
          stroke="var(--text-muted)"
          stroke-width="1.5"
          transform="rotate(-20 12 12)"
        />
      </svg>
      <span>Orbit</span>
    </div>

    {#each tabs as t}
      <button class="nav" class:active={tab === t.id} onclick={() => (tab = t.id)}>
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path
            d={t.icon}
            fill={t.id === "gallery" ? "currentColor" : "none"}
            stroke="currentColor"
            stroke-width={t.id === "gallery" ? 0 : 2}
            stroke-linecap="round"
          />
        </svg>
        {t.label}
      </button>
    {/each}

    <div class="controls">
      <ToggleButton
        labelOff="Replay buffer"
        labelOn="Replay buffer on"
        hint="{hotkeyLabel(keys.hotkey_replay)} saves"
        statusCommand="is_replay_running"
        toggleCommand="toggle_replay"
        onEvents={["replay-started"]}
        offEvents={["replay-stopped", "replay-failed"]}
      />
      <ToggleButton
        labelOff="Record"
        labelOn="Stop recording"
        hint={hotkeyLabel(keys.hotkey_record)}
        statusCommand="is_recording"
        toggleCommand="toggle_recording"
        onEvents={["recording-started"]}
        offEvents={["recording-saved", "recording-failed"]}
      />
    </div>
  </nav>

  <main class="content">
    <!-- {#key} remakes the page on tab change, so it can slide in. -->
    {#key tab}
      <div class="page" in:fly={{ y: 8, duration: ms(220) }}>
        {#if tab === "gallery"}
          <Gallery />
        {:else}
          <Settings />
        {/if}
      </div>
    {/key}
  </main>
</div>

<Toast />

<style>
  .app {
    display: flex;
    height: 100vh;
  }

  .sidebar {
    width: 200px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 18px 12px;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 20px;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.3px;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    text-align: left;
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .nav:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .nav.active {
    background: var(--surface-2);
    color: var(--text);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .controls {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
  }
  .page {
    padding: 28px 36px 40px;
  }
</style>
