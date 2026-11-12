<script lang="ts">
  // Orbit's own clean video player (instead of the browser's built-in controls).
  // Video decoding is still done by the GPU through the built-in <video> element.

  let {
    src,
    volume = $bindable(1),
    speed = $bindable(1),
    trim = $bindable(null),
    onerror,
  }: {
    src: string;
    volume?: number;
    speed?: number;
    // Trim mode: the part to keep (seconds). null = normal player.
    trim?: { start: number; end: number } | null;
    onerror?: () => void;
  } = $props();

  let video: HTMLVideoElement;
  let box: HTMLDivElement;
  let paused = $state(true);
  let duration = $state(0);
  let time = $state(0);
  let muted = $state(false);

  // Smooth timeline while playing: read the position every screen frame, but only
  // while the video plays (a paused player does no work at all).
  $effect(() => {
    if (paused) return;
    let frame = 0;
    const tick = () => {
      time = video.currentTime;
      // Trimming: play only the kept part, on repeat.
      if (trim && (time >= trim.end || time < trim.start - 0.05)) {
        video.currentTime = trim.start;
        time = trim.start;
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(frame);
  });

  // Trim mode starts with the whole video selected.
  $effect(() => {
    if (trim && duration > 0 && trim.end > duration) trim.end = duration;
  });

  function togglePlay() {
    if (video.ended) video.currentTime = trim?.start ?? 0;
    if (video.paused) video.play();
    else video.pause();
  }

  function skip(seconds: number) {
    const lo = trim?.start ?? 0;
    const hi = trim?.end ?? (duration || 0);
    video.currentTime = Math.min(Math.max(lo, video.currentTime + seconds), hi);
    time = video.currentTime;
  }

  // --- Timeline: click or drag to jump (or, when trimming, drag the start/end handles) ---
  let dragging = $state(false);
  let handle: "start" | "end" | null = null;
  const MIN_TRIM = 0.5; // seconds

  function timeAt(e: PointerEvent, bar: HTMLElement) {
    const rect = bar.getBoundingClientRect();
    return Math.min(Math.max(0, (e.clientX - rect.left) / rect.width), 1) * duration;
  }
  function seekTo(e: PointerEvent, bar: HTMLElement) {
    let t = timeAt(e, bar);
    if (handle && trim) {
      // Move a trim handle and show that exact frame.
      if (handle === "start") trim.start = t = Math.min(t, trim.end - MIN_TRIM);
      else trim.end = t = Math.max(t, trim.start + MIN_TRIM);
    } else if (trim) {
      t = Math.min(Math.max(t, trim.start), trim.end);
    }
    video.currentTime = t;
    time = t;
  }
  function onPointerDown(e: PointerEvent) {
    const bar = e.currentTarget as HTMLElement;
    bar.setPointerCapture(e.pointerId);
    dragging = true;
    handle = ((e.target as HTMLElement).dataset.handle as "start" | "end") ?? null;
    if (handle) video.pause();
    seekTo(e, bar);
  }
  function onPointerMove(e: PointerEvent) {
    if (dragging) seekTo(e, e.currentTarget as HTMLElement);
  }
  function onPointerUp() {
    dragging = false;
    handle = null;
  }

  function fullscreen() {
    if (document.fullscreenElement) document.exitFullscreen();
    else box.requestFullscreen();
  }

  // Space = play / pause (arrow keys stay "previous / next file" in the viewer).
  function onKeydown(e: KeyboardEvent) {
    if (e.code === "Space") {
      e.preventDefault();
      togglePlay();
    }
  }

  function clock(s: number) {
    if (!isFinite(s)) s = 0;
    const t = Math.floor(s);
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const ss = String(t % 60).padStart(2, "0");
    return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
  }

  let progress = $derived(duration > 0 ? (time / duration) * 100 : 0);
  const speeds = [0.5, 1, 2];
</script>

<svelte:window onkeydown={onKeydown} />

<div class="player" bind:this={box}>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={video}
    {src}
    autoplay
    preload="metadata"
    bind:paused
    bind:duration
    bind:volume
    bind:muted
    bind:playbackRate={speed}
    onclick={togglePlay}
    onseeked={() => (time = video.currentTime)}
    ontimeupdate={() => paused && (time = video.currentTime)}
    {onerror}
  ></video>

  <div class="controls">
    <!-- Timeline -->
    <div
      class="timeline"
      class:dragging
      class:trimming={trim !== null}
      role="slider"
      tabindex="-1"
      aria-label="Position"
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(time)}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
    >
      <div class="track">
        <div class="fill" style:width="{progress}%"></div>
      </div>
      {#if trim && duration > 0}
        <!-- Greyed-out parts that get cut, and the two handles. -->
        <div class="cut" style:left="0" style:width="{(trim.start / duration) * 100}%"></div>
        <div class="cut" style:left="{(trim.end / duration) * 100}%" style:right="0"></div>
        <div
          class="keep"
          style:left="{(trim.start / duration) * 100}%"
          style:width="{((trim.end - trim.start) / duration) * 100}%"
        ></div>
        <div class="trim-handle" data-handle="start" style:left="{(trim.start / duration) * 100}%"></div>
        <div class="trim-handle" data-handle="end" style:left="{(trim.end / duration) * 100}%"></div>
      {/if}
      <div class="knob" style:left="{progress}%"></div>
    </div>

    <div class="bar">
      <div class="left">
        <button class="icon" title="Back 10 seconds" aria-label="Back 10 seconds" onclick={() => skip(-10)}>
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
            <path d="M12 5V2L7.5 5.5 12 9V6.5a6 6 0 1 1-6 6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
            <text x="12.3" y="15.6" text-anchor="middle" font-size="6.2" font-weight="600" fill="currentColor">10</text>
          </svg>
        </button>

        <button class="icon play" title={paused ? "Play (Space)" : "Pause (Space)"} aria-label={paused ? "Play" : "Pause"} onclick={togglePlay}>
          {#if paused}
            <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M8 5.5v13l10.5-6.5z" fill="currentColor" /></svg>
          {:else}
            <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" fill="currentColor" /></svg>
          {/if}
        </button>

        <button class="icon" title="Forward 10 seconds" aria-label="Forward 10 seconds" onclick={() => skip(10)}>
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
            <path d="M12 5V2l4.5 3.5L12 9V6.5a6 6 0 1 0 6 6" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" />
            <text x="11.7" y="15.6" text-anchor="middle" font-size="6.2" font-weight="600" fill="currentColor">10</text>
          </svg>
        </button>

        <span class="time">{clock(time)} <span class="faint">/ {clock(duration)}</span></span>
      </div>

      <div class="right">
        <div class="speeds" role="radiogroup" aria-label="Speed">
          {#each speeds as s}
            <button role="radio" aria-checked={speed === s} class:on={speed === s} onclick={() => (speed = s)}>
              {s}x
            </button>
          {/each}
        </div>

        <div class="volume">
          <button class="icon" title={muted ? "Unmute" : "Mute"} aria-label={muted ? "Unmute" : "Mute"} onclick={() => (muted = !muted)}>
            <svg viewBox="0 0 24 24" width="19" height="19" aria-hidden="true">
              <path d="M4 9.5h3.5L12 6v12l-4.5-3.5H4z" fill="currentColor" />
              {#if muted || volume === 0}
                <path d="M16 9.5l5 5M21 9.5l-5 5" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
              {:else}
                <path d="M15.5 9.5a3.5 3.5 0 0 1 0 5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
                {#if volume > 0.5}
                  <path d="M18 7.5a6.5 6.5 0 0 1 0 9" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
                {/if}
              {/if}
            </svg>
          </button>
          <input
            class="vol"
            type="range"
            min="0"
            max="1"
            step="0.01"
            aria-label="Volume"
            bind:value={volume}
            oninput={() => (muted = false)}
            style:--level="{(muted ? 0 : volume) * 100}%"
          />
        </div>

        <button class="icon" title="Fullscreen" aria-label="Fullscreen" onclick={fullscreen}>
          <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
            <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .player {
    display: flex;
    flex-direction: column;
    width: 100%;
    max-height: 100%;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: var(--radius);
    background: #000;
    overflow: hidden;
  }
  video {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: contain;
    background: #000;
    cursor: pointer;
  }
  /* Fullscreen: video fills the screen, controls stay at the bottom. */
  .player:fullscreen {
    border: none;
    border-radius: 0;
  }
  .player:fullscreen video {
    flex: 1;
    min-height: 0;
    aspect-ratio: auto;
  }

  /* The control panel is always dark, also in the light theme (it sits on the video). */
  .controls {
    padding: 6px 14px 10px;
    background: #0b0b0c;
    color: #f2f2f2;
  }

  .timeline {
    position: relative;
    height: 16px;
    cursor: pointer;
    touch-action: none;
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 3px;
    margin-top: -1.5px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.16);
    overflow: hidden;
    transition: height var(--fast) var(--ease), margin var(--fast) var(--ease);
  }
  .timeline:hover .track,
  .timeline.dragging .track {
    height: 5px;
    margin-top: -2.5px;
  }
  .fill {
    height: 100%;
    background: #fff;
  }
  .knob {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border-radius: 50%;
    background: #fff;
    opacity: 0;
    transition: opacity var(--fast) var(--ease);
  }
  .timeline:hover .knob,
  .timeline.dragging .knob {
    opacity: 1;
  }

  /* --- Trim mode --- */
  .timeline.trimming {
    height: 30px;
    margin-bottom: 2px;
  }
  .timeline.trimming .track {
    height: 6px;
    margin-top: -3px;
  }
  .cut {
    position: absolute;
    top: 50%;
    height: 6px;
    margin-top: -3px;
    background: rgba(0, 0, 0, 0.7);
    pointer-events: none;
  }
  .keep {
    position: absolute;
    top: 50%;
    height: 20px;
    margin-top: -10px;
    border-top: 2px solid #fff;
    border-bottom: 2px solid #fff;
    pointer-events: none;
  }
  .trim-handle {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 26px;
    margin: -13px 0 0 -5px;
    border-radius: 4px;
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6);
    cursor: ew-resize;
    z-index: 1;
  }
  .trim-handle::after {
    content: "";
    position: absolute;
    left: 4px;
    top: 8px;
    width: 2px;
    height: 10px;
    border-radius: 1px;
    background: #000;
    opacity: 0.4;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .right {
    gap: 10px;
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: rgba(255, 255, 255, 0.82);
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .icon:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .play {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: #fff;
    color: #000;
  }
  .play:hover {
    background: #e6e6e6;
    color: #000;
  }

  .time {
    margin-left: 8px;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .faint {
    color: rgba(255, 255, 255, 0.45);
  }

  .speeds {
    display: flex;
    padding: 2px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.07);
  }
  .speeds button {
    padding: 4px 9px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .speeds button:hover {
    color: #fff;
  }
  .speeds button.on {
    background: #fff;
    color: #000;
  }

  .volume {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  /* Thin volume slider: white up to the current level, grey after. */
  .vol {
    appearance: none;
    width: 84px;
    height: 3px;
    margin: 0;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      #fff 0 var(--level),
      rgba(255, 255, 255, 0.18) var(--level) 100%
    );
    cursor: pointer;
  }
  .vol::-webkit-slider-thumb {
    appearance: none;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #fff;
    transition: transform var(--fast) var(--ease);
  }
  .vol:hover::-webkit-slider-thumb {
    transform: scale(1.15);
  }
</style>
