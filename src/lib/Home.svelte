<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { hotkeyLabel } from "$lib/hotkey";

  // Overview: what's running, the main actions, hotkeys, storage and latest captures.
  // Everything updates from Rust events (no polling). Only the recording timer ticks,
  // once a second, and only while recording with this page open.

  let { onopen }: { onopen: (tab: "gallery" | "settings") => void } = $props();

  type Item = { path: string; name: string; kind: string; game: string; modified: number; size: number };
  type S = {
    replay_seconds: number;
    hotkey_screenshot: string;
    hotkey_snip: string;
    hotkey_record: string;
    hotkey_replay: string;
    video_fps: number;
    video_quality: string;
    system_audio: boolean;
    mic_enabled: boolean;
  };

  let s = $state<S | null>(null);
  let replayOn = $state(false);
  let recordingSince = $state<number | null>(null);
  let now = $state(Date.now());
  let recent = $state<Item[]>([]);
  // path -> thumbnail URL + video length
  let thumbs = $state<Record<string, { src: string; duration: number | null }>>({});
  let storage = $state<[number, number]>([0, 0]);

  async function loadRecent() {
    const items = await invoke<Item[]>("list_media");
    recent = items.slice(0, 15);
    for (const it of recent) {
      if (thumbs[it.path]) continue;
      invoke<{ thumb: string; duration: number | null }>("preview", { path: it.path })
        .then((p) => (thumbs[it.path] = { src: convertFileSrc(p.thumb), duration: p.duration }))
        .catch(() => {});
    }
    storage = await invoke<[number, number]>("storage_summary");
  }

  async function loadRecording() {
    recordingSince = await invoke<number | null>("recording_since");
  }

  $effect(() => {
    invoke<S>("get_settings").then((v) => (s = v));
    invoke<boolean>("is_replay_running").then((v) => (replayOn = v));
    loadRecording();
    loadRecent();
    const stops = [
      listen<S>("settings-changed", (e) => (s = e.payload)),
      listen("replay-started", () => (replayOn = true)),
      listen("replay-stopped", () => (replayOn = false)),
      listen("replay-failed", () => (replayOn = false)),
      listen("recording-started", loadRecording),
      listen("recording-saved", () => ((recordingSince = null), loadRecent())),
      listen("recording-failed", () => (recordingSince = null)),
      listen("screenshot-saved", loadRecent),
      listen("replay-saved", loadRecent),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });

  // Recording timer: ticks only while recording.
  $effect(() => {
    if (recordingSince === null) return;
    now = Date.now();
    const id = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(id);
  });

  function elapsed(ms: number) {
    const t = Math.max(0, Math.floor(ms / 1000));
    const h = Math.floor(t / 3600);
    const m = String(Math.floor((t % 3600) / 60)).padStart(2, "0");
    const sec = String(t % 60).padStart(2, "0");
    return h ? `${h}:${m}:${sec}` : `${m}:${sec}`;
  }
  function lengthLabel(seconds: number) {
    return seconds < 60 ? `${seconds} s` : `${seconds / 60} min`;
  }
  function size(bytes: number) {
    return bytes >= 1024 ** 3 ? `${(bytes / 1024 ** 3).toFixed(1)} GB` : `${Math.round(bytes / 1024 ** 2)} MB`;
  }
  function when(ms: number) {
    return new Date(ms).toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  }
  function length(s: number) {
    const t = Math.round(s);
    return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
  }
  const kinds: Record<string, string> = { screenshot: "Screenshot", clip: "Clip", recording: "Recording" };
  const quality: Record<string, string> = { low: "Low", medium: "Medium", high: "High", very_high: "Very high" };
</script>

<div class="head">
  <div>
    <h1>Home</h1>
    <p class="muted">
      {#if recordingSince !== null}
        Recording right now.
      {:else if replayOn}
        Replay buffer is running: press {s ? hotkeyLabel(s.hotkey_replay) : ""} to save a clip.
      {:else}
        Ready. Everything works in the background, even with this window closed.
      {/if}
    </p>
  </div>
</div>

<div class="cards">
  <!-- Replay buffer -->
  <section class="card" class:live={replayOn}>
    <div class="card-head">
      <span class="dot" class:on={replayOn}></span>
      <h2>Replay buffer</h2>
      <span class="pill">{replayOn ? "On" : "Off"}</span>
    </div>
    <p class="muted small">
      Keeps the last {s ? lengthLabel(s.replay_seconds) : "…"} so you can save what just happened.
    </p>
    <div class="actions">
      <button class="btn" class:primary={!replayOn} onclick={() => invoke("toggle_replay")}>
        {replayOn ? "Turn off" : "Turn on"}
      </button>
      <button class="btn" disabled={!replayOn} onclick={() => invoke("save_replay")}>
        Save replay
        {#if s}<kbd>{hotkeyLabel(s.hotkey_replay)}</kbd>{/if}
      </button>
    </div>
  </section>

  <!-- Recording -->
  <section class="card" class:live={recordingSince !== null}>
    <div class="card-head">
      <span class="dot" class:on={recordingSince !== null}></span>
      <h2>Recording</h2>
      {#if recordingSince !== null}
        <span class="pill rec">{elapsed(now - recordingSince)}</span>
      {:else}
        <span class="pill">Idle</span>
      {/if}
    </div>
    <p class="muted small">
      {s ? `${s.video_fps} fps · ${quality[s.video_quality] ?? s.video_quality} quality` : "…"}
      {s?.system_audio ? " · game sound" : ""}{s?.mic_enabled ? " · mic" : ""}
    </p>
    <div class="actions">
      <button class="btn" class:danger-solid={recordingSince !== null} class:primary={recordingSince === null} onclick={() => invoke("toggle_recording")}>
        {recordingSince !== null ? "Stop recording" : "Start recording"}
        {#if s}<kbd>{hotkeyLabel(s.hotkey_record)}</kbd>{/if}
      </button>
    </div>
  </section>

  <!-- Hotkeys -->
  <section class="card">
    <div class="card-head">
      <h2>Hotkeys</h2>
      <button class="link" onclick={() => onopen("settings")}>Change</button>
    </div>
    {#if s}
      <ul class="keys">
        <li><span>Screenshot</span><kbd>{hotkeyLabel(s.hotkey_screenshot)}</kbd></li>
        <li><span>Region screenshot (snip)</span><kbd>{hotkeyLabel(s.hotkey_snip)}</kbd></li>
        <li><span>Start / stop recording</span><kbd>{hotkeyLabel(s.hotkey_record)}</kbd></li>
        <li><span>Save replay</span><kbd>{hotkeyLabel(s.hotkey_replay)}</kbd></li>
      </ul>
    {/if}
  </section>

  <!-- Storage -->
  <section class="card">
    <div class="card-head">
      <h2>Storage</h2>
      <button class="link" onclick={() => invoke("open_media_folder")}>Open folder</button>
    </div>
    <div class="big">{size(storage[0])}</div>
    <p class="muted small">{storage[1]} {storage[1] === 1 ? "file" : "files"} saved</p>
  </section>
</div>

<div class="section-head">
  <h2>Latest captures</h2>
  <button class="link" onclick={() => onopen("gallery")}>Open gallery →</button>
</div>
{#if recent.length === 0}
  <p class="faint">Nothing yet. Take a screenshot or save a replay and it shows up here.</p>
{:else}
  <div class="recent">
    {#each recent as it (it.path)}
      <button class="tile" title={it.name} onclick={() => onopen("gallery")}>
        <div class="tile-img">
          {#if thumbs[it.path]}<img src={thumbs[it.path].src} alt="" decoding="async" />{/if}
          <span class="badge">{kinds[it.kind] ?? it.kind}</span>
          {#if thumbs[it.path]?.duration != null}
            <span class="badge len">{length(thumbs[it.path].duration!)}</span>
          {/if}
        </div>
        <span class="tile-game">{it.game}</span>
        <span class="tile-date">{when(it.modified)}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .head {
    margin-bottom: 22px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.4px;
  }
  h1 + p {
    margin: 0;
  }
  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .small {
    margin: 6px 0 14px;
    font-size: 12.5px;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 14px;
    margin-bottom: 28px;
  }
  .card {
    display: flex;
    flex-direction: column;
    padding: 16px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    transition: border-color var(--normal) var(--ease);
  }
  .card.live {
    border-color: color-mix(in srgb, var(--rec) 45%, var(--border));
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .card-head .link,
  .card-head .pill {
    margin-left: auto;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-faint);
  }
  .dot.on {
    background: var(--rec);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--rec) 25%, transparent);
  }
  .pill {
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
  }
  .pill.rec {
    background: var(--rec-soft);
    color: var(--rec);
    font-weight: 600;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: auto;
  }
  .btn.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
  }
  .btn.primary:hover {
    opacity: 0.9;
  }
  .btn.danger-solid {
    border-color: var(--danger);
    background: var(--danger);
    color: #fff;
  }
  .btn:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  kbd {
    padding: 1px 6px;
    border: 1px solid color-mix(in srgb, currentColor 30%, transparent);
    border-radius: 5px;
    font-family: inherit;
    font-size: 11px;
    opacity: 0.75;
  }
  .keys {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
  }
  .keys li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 7px 0;
    border-top: 1px solid var(--border);
    font-size: 13px;
  }
  .keys li:first-child {
    border-top: none;
  }
  .keys kbd {
    opacity: 1;
    color: var(--text);
  }
  .big {
    margin-top: 12px;
    font-size: 28px;
    font-weight: 600;
    letter-spacing: -0.5px;
    font-variant-numeric: tabular-nums;
  }
  .big + .small {
    margin-top: 2px;
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--text-muted);
    font-size: 12.5px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
  }

  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  /* 5 per row; fewer in a narrow window. */
  .recent {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 16px 14px;
  }
  @media (max-width: 1150px) {
    .recent {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }
  @media (max-width: 950px) {
    .recent {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  .badge {
    position: absolute;
    left: 6px;
    top: 6px;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 10.5px;
  }
  .badge.len {
    left: auto;
    top: auto;
    right: 6px;
    bottom: 6px;
    font-variant-numeric: tabular-nums;
  }
  .tile-date {
    margin-top: -4px;
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .tile-img {
    position: relative;
    aspect-ratio: 16 / 9;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    overflow: hidden;
    transition: border-color var(--fast) var(--ease);
  }
  .tile:hover .tile-img {
    border-color: var(--hover-border);
  }
  .tile-img img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .tile-game {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
