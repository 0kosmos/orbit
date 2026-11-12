<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // Shows whether we're recording. Updated only by events from Rust (no polling).
  let recording = $state(false);

  $effect(() => {
    invoke<boolean>("is_recording").then((r) => (recording = r));
    const stops = [
      listen("recording-started", () => (recording = true)),
      listen("recording-saved", () => (recording = false)),
      listen("recording-failed", () => (recording = false)),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });
</script>

<button class="rec" class:on={recording} onclick={() => invoke("toggle_recording")}>
  <span class="dot"></span>
  {recording ? "Stop recording" : "Record"}
  <span class="key">Alt+F9</span>
</button>

<style>
  .rec {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: auto;
    padding: 8px 10px;
    border: 1px solid #222;
    border-radius: 6px;
    background: transparent;
    color: #ccc;
    font: inherit;
    cursor: pointer;
  }
  .rec:hover {
    background: #141414;
    color: #fff;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
  }
  .rec.on {
    border-color: #6b2222;
    color: #fff;
  }
  .rec.on .dot {
    background: #e53935;
  }
  .key {
    margin-left: auto;
    font-size: 11px;
    color: #666;
  }
</style>
