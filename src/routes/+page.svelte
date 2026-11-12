<script lang="ts">
  import Home from "$lib/Home.svelte";
  import Gallery from "$lib/Gallery.svelte";
  import Settings from "$lib/Settings.svelte";
  import Toast from "$lib/Toast.svelte";
  import ToggleButton from "$lib/ToggleButton.svelte";
  import StatusBar from "$lib/StatusBar.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fly } from "svelte/transition";
  import { hotkeyLabel } from "$lib/hotkey";
  import { applyUi, ms } from "$lib/ui.svelte";

  type Tab = "home" | "gallery" | "settings";
  let tab = $state<Tab>("home");

  // The settings the shell needs: hotkey hints, the look, and the status bar switch.
  // Loaded once, then updated only when Settings changes something (no polling).
  type ShellSettings = {
    hotkey_record: string;
    hotkey_replay: string;
    theme: string;
    animations: boolean;
    blur: boolean;
    status_bar: boolean;
  };
  let keys = $state({ hotkey_record: "Alt+F9", hotkey_replay: "Alt+F10" });
  let statusBar = $state(false);
  function use(s: ShellSettings) {
    keys = s;
    statusBar = s.status_bar;
    applyUi(s);
  }
  $effect(() => {
    invoke<ShellSettings>("get_settings").then(use);
    // Opened from the overlay's "All settings": start on that page.
    invoke<string | null>("take_pending_tab").then((t) => t && (tab = t as Tab));
    const stops = [
      listen<ShellSettings>("settings-changed", (e) => use(e.payload)),
      listen<string>("open-tab", (e) => {
        tab = e.payload as Tab;
        invoke("take_pending_tab");
      }),
    ];
    return () => stops.forEach((p) => p.then((s) => s()));
  });

  const tabs: { id: Tab; label: string; icon: string; fill?: boolean }[] = [
    // house
    { id: "home", label: "Home", icon: "M4 11l8-7 8 7v8a1 1 0 0 1-1 1h-4v-6h-6v6H5a1 1 0 0 1-1-1z" },
    // 4 squares
    { id: "gallery", label: "Gallery", icon: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z", fill: true },
    // sliders
    { id: "settings", label: "Settings", icon: "M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M6 14v6" },
  ];
</script>

<div class="app" class:with-status={statusBar}>
  <nav class="sidebar">
    <div class="logo">
      <img src="/logo.png" alt="" width="26" height="26" />
      <span>Orbit</span>
    </div>

    <div class="nav-label">Menu</div>
    {#each tabs as t}
      <button class="nav" class:active={tab === t.id} onclick={() => (tab = t.id)}>
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path
            d={t.icon}
            fill={t.fill ? "currentColor" : "none"}
            stroke="currentColor"
            stroke-width={t.fill ? 0 : 1.9}
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        {t.label}
      </button>
    {/each}

    <div class="controls">
      <div class="nav-label">Capture</div>
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
        {#if tab === "home"}
          <Home onopen={(t) => (tab = t)} />
        {:else if tab === "gallery"}
          <Gallery />
        {:else}
          <Settings />
        {/if}
      </div>
    {/key}
  </main>

  {#if statusBar}
    <div class="status-area"><StatusBar /></div>
  {/if}
</div>

<Toast />

<style>
  .app {
    display: grid;
    grid-template-columns: 210px 1fr;
    grid-template-rows: 1fr;
    height: 100vh;
  }
  .app.with-status {
    grid-template-rows: 1fr auto;
  }
  .status-area {
    grid-column: 1 / -1;
  }

  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 0;
    padding: 18px 12px 14px;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 22px;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.2px;
  }
  .logo img {
    border-radius: 7px;
  }

  .nav-label {
    padding: 0 10px 6px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.9px;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 11px;
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
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
  }
  .page {
    max-width: 1280px;
    padding: 28px 36px 40px;
  }
</style>
