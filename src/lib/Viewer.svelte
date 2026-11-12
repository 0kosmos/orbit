<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";

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
  <div class="backdrop" role="presentation" onclick={onclose}>
    <div class="viewer" role="dialog" aria-label={item.name} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <header>
        <div class="title">
          <span class="game">{item.game}</span>
          <span class="muted">{formatDate(item.modified)} · {index + 1} / {items.length}</span>
        </div>
        <div class="actions">
          <button onclick={() => invoke("show_in_folder", { path: item.path })}>Show in folder</button>
          <button onclick={() => invoke("open_media", { path: item.path })}>Open in Windows</button>
          <button class="danger" onclick={() => ondelete(item)}>Delete</button>
          <button class="close" title="Close (Esc)" onclick={onclose}>✕</button>
        </div>
      </header>

      <div class="stage">
        {#if failed}
          <div class="fallback">
            <p>This file can't be played inside Orbit (older recordings are MKV files).</p>
            <button onclick={() => invoke("open_media", { path: item.path })}>Open in Windows</button>
          </div>
        {:else if isVideo}
          <!-- {#key} makes a fresh player for each file, so the old one is fully freed. -->
          {#key src}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video {src} controls autoplay preload="metadata" bind:volume onerror={() => (failed = true)}></video>
          {/key}
        {:else}
          <img {src} alt={item.name} decoding="async" />
        {/if}

        {#if index > 0}
          <button class="nav left" title="Previous (←)" onclick={prev}>‹</button>
        {/if}
        {#if index < items.length - 1}
          <button class="nav right" title="Next (→)" onclick={next}>›</button>
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
    padding: 24px;
    background: rgba(0, 0, 0, 0.85);
  }
  .viewer {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    gap: 12px;
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
    min-width: 0;
  }
  .game {
    font-size: 15px;
    font-weight: 600;
    color: #fff;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .muted {
    font-size: 12px;
    color: #888;
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  button {
    padding: 6px 12px;
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    background: #111;
    color: #ddd;
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  button:hover {
    border-color: #444;
    color: #fff;
  }
  .danger:hover {
    background: #b3261e;
    border-color: #b3261e;
  }
  .close {
    width: 32px;
    padding: 6px 0;
  }

  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  video,
  img {
    max-width: 100%;
    max-height: 100%;
    border-radius: 6px;
    background: #000;
  }
  .fallback {
    text-align: center;
    color: #aaa;
  }

  .nav {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 40px;
    height: 64px;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.5);
    font-size: 28px;
    line-height: 1;
  }
  .nav.left {
    left: 0;
  }
  .nav.right {
    right: 0;
  }
</style>
