<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { fly } from "svelte/transition";
  import { ms } from "$lib/ui.svelte";

  // A small message in the corner that hides itself after a few seconds.
  // It only reacts to events sent from Rust, so there's no polling.
  let message = $state("");
  let isError = $state(false);
  // Changes with every message, so a new message slides in again.
  let id = $state(0);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(text: string, error = false) {
    message = text;
    isError = error;
    id++;
    clearTimeout(timer);
    timer = setTimeout(() => (message = ""), error ? 6000 : 2500);
  }

  $effect(() => {
    const stops = [
      listen<string>("screenshot-saved", (e) => {
        const file = e.payload.split("\\").pop();
        show(`Screenshot saved: ${file}`);
      }),
      listen<string>("screenshot-failed", (e) => show(`Screenshot failed: ${e.payload}`, true)),
      listen<string>("recording-saved", (e) => {
        const file = e.payload.split("\\").pop();
        show(`Recording saved: ${file}`);
      }),
      listen<string>("recording-failed", (e) => show(`Recording failed: ${e.payload}`, true)),
      listen<string>("replay-saved", (e) => {
        const file = e.payload.split("\\").pop();
        show(`Clip saved: ${file}`);
      }),
      listen<string>("replay-save-failed", (e) => show(`Saving clip failed: ${e.payload}`, true)),
      listen<string>("replay-failed", (e) => show(`Replay buffer stopped: ${e.payload}`, true)),
      listen("snip-copied", () => show("Copied to clipboard")),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });
</script>

{#if message}
  {#key id}
    <div
      class="toast"
      class:error={isError}
      role="status"
      in:fly={{ y: 16, duration: ms(260) }}
      out:fly={{ y: 16, duration: ms(180) }}
    >
      <span class="dot"></span>
      <span class="text">{message}</span>
    </div>
  {/key}
{/if}

<style>
  .toast {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 100;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 440px;
    padding: 11px 16px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    /* Solid on purpose: a blurred, moving notification can make WebView2 stop redrawing. */
    background: var(--surface-2);
    box-shadow: var(--shadow);
    color: var(--text);
    font-size: 13px;
    user-select: text;
  }
  .dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
  }
  .toast.error .dot {
    background: var(--rec);
  }
  .toast.error .text {
    color: #f87171;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
