<script lang="ts">
  import Gallery from "$lib/Gallery.svelte";
  import Settings from "$lib/Settings.svelte";
  import Toast from "$lib/Toast.svelte";
  import RecordButton from "$lib/RecordButton.svelte";

  type Tab = "gallery" | "settings";
  let tab = $state<Tab>("gallery");
</script>

<div class="app">
  <nav class="sidebar">
    <div class="logo">Orbit</div>
    <button class:active={tab === "gallery"} onclick={() => (tab = "gallery")}>Gallery</button>
    <button class:active={tab === "settings"} onclick={() => (tab = "settings")}>Settings</button>
    <RecordButton />
  </nav>

  <main class="content">
    {#if tab === "gallery"}
      <Gallery />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

<Toast />

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: #000;
    color: #e6e6e6;
    font-family: "Segoe UI", system-ui, sans-serif;
    font-size: 14px;
    user-select: none;
  }

  .app {
    display: flex;
    height: 100vh;
  }

  .sidebar {
    width: 180px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 10px;
    background: #0a0a0a;
    border-right: 1px solid #1a1a1a;
  }

  .logo {
    font-size: 18px;
    font-weight: 600;
    letter-spacing: 0.5px;
    padding: 0 10px 16px;
    color: #fff;
  }

  .sidebar button {
    text-align: left;
    padding: 8px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #999;
    font: inherit;
    cursor: pointer;
  }

  .sidebar button:hover {
    background: #141414;
    color: #fff;
  }

  .sidebar button.active {
    background: #1a1a1a;
    color: #fff;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px 32px;
  }
</style>
