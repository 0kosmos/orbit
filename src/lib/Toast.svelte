<script lang="ts">
  import { listen } from "@tauri-apps/api/event";

  // A small message in the corner that hides itself after a few seconds.
  // It only reacts to events sent from Rust, so there's no polling.
  let message = $state("");
  let isError = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(text: string, error = false) {
    message = text;
    isError = error;
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
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });
</script>

{#if message}
  <div class="toast" class:error={isError}>{message}</div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 16px;
    bottom: 16px;
    max-width: 420px;
    padding: 10px 14px;
    background: #141414;
    border: 1px solid #2a2a2a;
    border-radius: 8px;
    color: #e6e6e6;
    font-size: 13px;
    user-select: text;
  }
  .toast.error {
    border-color: #6b2222;
    color: #ff9a9a;
  }
</style>
