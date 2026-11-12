<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { fade, scale } from "svelte/transition";
  import { ms } from "$lib/ui.svelte";
  import Player from "$lib/Player.svelte";

  // Same shape as the gallery's items.
  type Item = {
    path: string;
    name: string;
    kind: "screenshot" | "clip" | "recording";
    game: string;
    modified: number;
    size: number;
  };

  let {
    items,
    index = $bindable(),
    onclose,
    ondelete,
  }: {
    items: Item[];
    index: number;
    onclose: () => void;
    ondelete: (item: Item) => void;
  } = $props();

  let item = $derived(items[index]);
  let isVideo = $derived(item?.kind !== "screenshot");
  let src = $derived(item ? convertFileSrc(item.path) : "");
  let failed = $state(false);

  // A new file: forget the "can't play" message from the previous one.
  $effect(() => {
    void src;
    failed = false;
  });

  // Remember the volume between videos (only in this browser view; ok if unavailable).
  // Playback speed (0.5x / 1x / 2x), kept while flipping through videos.
  let speed = $state(1);
  let volume = $state(readVolume());
  function readVolume() {
    try {
      return Number(localStorage.getItem("viewer-volume") ?? "1");
    } catch {
      return 1;
    }
  }
  $effect(() => {
    try {
      localStorage.setItem("viewer-volume", String(volume));
    } catch {}
  });

  function prev() {
    if (index > 0) index--;
  }
  function next() {
    if (index < items.length - 1) index++;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    else if (e.key === "ArrowLeft") prev();
    else if (e.key === "ArrowRight") next();
    else return;
    e.preventDefault();
  }

  function formatDate(ms: number) {
    return new Date(ms).toLocaleString(undefined, {
      day: "numeric",
      month: "short",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if item}
  <!-- Clicking the dark area around the media closes the viewer. -->
  <div
    class="backdrop"
    role="presentation"
    onclick={onclose}
    transition:fade={{ duration: ms(180) }}
  >
    <div
      class="viewer"
      role="dialog"
      aria-label={item.name}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={() => {}}
      transition:scale={{ start: 0.97, duration: ms(220) }}
    >
      <header>
        <div class="title">
          <span class="game">{item.game}</span>
          <span class="muted">{formatDate(item.modified)} · {index + 1} / {items.length}</span>
        </div>
        <div class="actions">
          <button class="btn" onclick={() => invoke("show_in_folder", { path: item.path })}>
            Show in folder
          </button>
          <button class="btn" onclick={() => invoke("open_media", { path: item.path })}>
            Open in Windows
          </button>
          <button class="btn danger" onclick={() => ondelete(item)}>Delete</button>
          <button class="btn ghost close" title="Close (Esc)" aria-label="Close" onclick={onclose}>
            <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
              <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      </header>

      <div class="stage">
        {#if failed}
          <div class="fallback">
            <p>This file can't be played inside Orbit (older recordings are MKV files).</p>
            <button class="btn" onclick={() => invoke("open_media", { path: item.path })}>
              Open in Windows
            </button>
          </div>
        {:else if isVideo}
          <!-- {#key} makes a fresh player for each file, so the old one is fully freed. -->
          {#key src}
            <div class="media" in:fade={{ duration: ms(160) }}>
              <Player {src} bind:volume bind:speed onerror={() => (failed = true)} />
            </div>
          {/key}
        {:else}
          {#key src}
            <img {src} alt={item.name} decoding="async" in:fade={{ duration: ms(160) }} />
          {/key}
        {/if}

        {#if index > 0}
          <button class="nav left" title="Previous (←)" aria-label="Previous" onclick={prev}>
            <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
              <path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
            </svg>
          </button>
        {/if}
        {#if index < items.length - 1}
          <button class="nav right" title="Next (→)" aria-label="Next" onclick={next}>
            <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
              <path d="M9 5l7 7-7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
            </svg>
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    padding: 24px 28px;
    background: var(--overlay);
    backdrop-filter: blur(18px) saturate(120%);
  }
  :global(:root[data-blur="off"]) .backdrop {
    background: var(--bg);
    backdrop-filter: none;
  }
  .viewer {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    gap: 14px;
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .game {
    font-size: 16px;
    font-weight: 600;
    color: #fff;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  /* Over the dark backdrop the text stays light, also in the light theme. */
  .title .muted {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
  }
  :global(:root[data-blur="off"][data-theme="light"]) .game {
    color: var(--text);
  }
  :global(:root[data-blur="off"][data-theme="light"]) .title .muted {
    color: var(--text-muted);
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .close {
    padding: 6px 8px;
  }

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  /* The player: at most 1100px wide, and small enough that it (with its controls)
     always fits the space above and below. */
  .stage {
    container-type: size;
  }
  .media {
    width: min(1100px, 100cqw, calc((100cqh - 84px) * 16 / 9));
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    border-radius: var(--radius);
  }
  img {
    max-width: min(1100px, 100%);
    max-height: 100%;
    border-radius: var(--radius);
    background: #000;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  }
  .fallback {
    text-align: center;
    color: rgba(255, 255, 255, 0.75);
  }

  .nav {
    position: absolute;
    top: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    margin-top: -22px;
    padding: 0;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
    cursor: pointer;
    opacity: 0.7;
    transition:
      opacity var(--fast) var(--ease),
      transform var(--fast) var(--ease),
      background var(--fast) var(--ease);
  }
  .nav:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.7);
    transform: scale(1.06);
  }
  .nav.left {
    left: 4px;
  }
  .nav.right {
    right: 4px;
  }
</style>
