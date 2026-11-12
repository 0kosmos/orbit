<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Viewer from "$lib/Viewer.svelte";
  import { fly } from "svelte/transition";
  import { ms } from "$lib/ui.svelte";

  // Must match MediaItem in src-tauri/src/media.rs
  type Kind = "screenshot" | "clip" | "recording";
  type Item = {
    path: string;
    name: string;
    kind: Kind;
    game: string;
    modified: number;
    size: number;
  };
  // Must match Preview in src-tauri/src/media.rs
  type Preview = { thumb: string; duration: number | null };
  type Sort = "newest" | "oldest" | "largest" | "smallest";

  const filters: { value: Kind | "all"; label: string }[] = [
    { value: "all", label: "All" },
    { value: "screenshot", label: "Screenshots" },
    { value: "clip", label: "Clips" },
    { value: "recording", label: "Recordings" },
  ];

  const sorts: { value: Sort; label: string }[] = [
    { value: "newest", label: "Newest first" },
    { value: "oldest", label: "Oldest first" },
    { value: "largest", label: "Largest first" },
    { value: "smallest", label: "Smallest first" },
  ];

  let items = $state<Item[]>([]);
  let filter = $state<Kind | "all">("all");
  let gameFilter = $state("all");
  let sort = $state<Sort>("newest");
  let error = $state("");
  // path -> preview (thumbnail URL + video length); thumb "" if it couldn't be made
  let previews = $state<Record<string, { src: string; duration: number | null }>>({});

  // Every game that has something in the gallery, A–Z ("Other" last).
  let games = $derived(
    [...new Set(items.map((i) => i.game))].sort((a, b) =>
      a === "Other" ? 1 : b === "Other" ? -1 : a.localeCompare(b),
    ),
  );

  const compare: Record<Sort, (a: Item, b: Item) => number> = {
    newest: (a, b) => b.modified - a.modified,
    oldest: (a, b) => a.modified - b.modified,
    largest: (a, b) => b.size - a.size,
    smallest: (a, b) => a.size - b.size,
  };

  let shown = $derived(
    items
      .filter(
        (i) =>
          (filter === "all" || i.kind === filter) && (gameFilter === "all" || i.game === gameFilter),
      )
      .sort(compare[sort]),
  );

  async function refresh() {
    items = await invoke<Item[]>("list_media");
  }

  // --- Lazy previews ---
  // A preview is only requested when its card scrolls into view,
  // and at most 3 are made at the same time.
  const queue: string[] = [];
  let busy = 0;

  function pump() {
    while (busy < 3 && queue.length > 0) {
      const path = queue.shift()!;
      busy++;
      invoke<Preview>("preview", { path })
        .then((p) => (previews[path] = { src: convertFileSrc(p.thumb), duration: p.duration }))
        .catch(() => (previews[path] = { src: "", duration: null }))
        .finally(() => {
          busy--;
          pump();
        });
    }
  }

  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      observer.unobserve(entry.target);
      const path = (entry.target as HTMLElement).dataset.path!;
      if (!(path in previews) && !queue.includes(path)) {
        queue.push(path);
        pump();
      }
    }
  });

  function lazy(node: HTMLElement) {
    observer.observe(node);
    return { destroy: () => observer.unobserve(node) };
  }

  // Load the list now, and again only when Orbit saves something new (no polling).
  $effect(() => {
    refresh();
    const stops = ["screenshot-saved", "recording-saved", "replay-saved"].map((name) =>
      listen(name, refresh),
    );
    return () => {
      stops.forEach((p) => p.then((stop) => stop()));
      observer.disconnect();
    };
  });

  // Moves the file to the Recycle Bin, then removes it from the list.
  async function remove(item: Item) {
    try {
      await invoke("delete_media", { path: item.path });
      items = items.filter((i) => i.path !== item.path);
      delete previews[item.path];
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  // --- Bulk select ---
  let selecting = $state(false);
  // Selected file paths. A plain object is used as a set (Svelte tracks its keys).
  let selected = $state<Record<string, true>>({});
  let lastClicked: string | null = null;
  let confirming = $state(false);
  let deleting = $state(false);

  // Only files that are visible with the current filters count (and get deleted).
  let selectedItems = $derived(shown.filter((i) => selected[i.path]));
  let selectedCount = $derived(selectedItems.length);
  let selectedSize = $derived(selectedItems.reduce((sum, i) => sum + i.size, 0));

  function startSelecting() {
    selecting = true;
    confirming = false;
  }
  function stopSelecting() {
    selecting = false;
    selected = {};
    confirming = false;
    lastClicked = null;
  }

  function toggle(item: Item) {
    if (selected[item.path]) delete selected[item.path];
    else selected[item.path] = true;
    confirming = false;
  }

  // Shift+click: select everything between the last clicked card and this one.
  function selectRange(item: Item) {
    const a = shown.findIndex((i) => i.path === lastClicked);
    const b = shown.findIndex((i) => i.path === item.path);
    if (a < 0 || b < 0) return toggle(item);
    for (const i of shown.slice(Math.min(a, b), Math.max(a, b) + 1)) selected[i.path] = true;
    confirming = false;
  }

  function onCardClick(e: MouseEvent, item: Item) {
    // Ctrl/Shift+click starts selecting right away.
    if (!selecting && (e.ctrlKey || e.shiftKey)) startSelecting();
    if (!selecting) {
      viewing = item.path;
      return;
    }
    if (e.shiftKey && lastClicked) selectRange(item);
    else toggle(item);
    lastClicked = item.path;
  }

  function selectAll() {
    for (const i of shown) selected[i.path] = true;
    confirming = false;
  }

  // First click asks "Delete N files?", the second click deletes.
  async function deleteSelected() {
    if (!confirming) {
      confirming = true;
      return;
    }
    deleting = true;
    const paths = selectedItems.map((i) => i.path);
    try {
      const done = await invoke<string[]>("delete_media_many", { paths });
      const gone = new Set(done);
      items = items.filter((i) => !gone.has(i.path));
      for (const p of done) {
        delete previews[p];
        delete selected[p];
      }
      error =
        done.length < paths.length
          ? `${paths.length - done.length} file(s) couldn't be deleted (open somewhere?).`
          : "";
      if (done.length === paths.length) stopSelecting();
    } catch (e) {
      error = String(e);
    }
    deleting = false;
    confirming = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (viewing) return; // the viewer handles its own keys
    if (e.key === "Escape" && selecting) stopSelecting();
    else if (e.key === "a" && e.ctrlKey && selecting) {
      e.preventDefault();
      selectAll();
    }
  }

  // --- In-app viewer ---
  // Path of the file open in the viewer (null = closed).
  let viewing = $state<string | null>(null);

  // Deleting from the viewer: show the next file (or the previous one at the end).
  async function removeFromViewer(item: Item) {
    const i = shown.findIndex((x) => x.path === item.path);
    await remove(item);
    if (items.some((x) => x.path === item.path)) return; // delete failed
    viewing = (shown[i] ?? shown[i - 1])?.path ?? null;
  }

  function formatDate(ms: number) {
    return new Date(ms).toLocaleString(undefined, {
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function formatSize(bytes: number) {
    return bytes < 1024 * 1024
      ? `${Math.max(1, Math.round(bytes / 1024))} KB`
      : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  // 62.3 -> "1:02", 3725 -> "1:02:05"
  function formatDuration(seconds: number) {
    const s = Math.round(seconds);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const ss = String(s % 60).padStart(2, "0");
    return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
  }

  const kindLabel: Record<Kind, string> = {
    screenshot: "Screenshot",
    clip: "Clip",
    recording: "Recording",
  };
</script>

<svelte:window onkeydown={onKeydown} />

<div class="header">
  <h1>Gallery</h1>
  <div class="header-actions">
  <button
    class="btn ghost"
    class:on={selecting}
    title="Select several files (or Ctrl+click a card)"
    onclick={() => (selecting ? stopSelecting() : startSelecting())}
  >
    <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
      <rect x="4" y="4" width="16" height="16" rx="4" fill="none" stroke="currentColor" stroke-width="1.8" />
      <path d="M8.5 12.2l2.4 2.4 4.6-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    {selecting ? "Done" : "Select"}
  </button>
  <button class="btn ghost" onclick={() => invoke("open_media_folder")}>
    <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
      <path
        d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
      />
    </svg>
    Open folder
  </button>
  </div>
</div>

<!-- Stays at the top while scrolling (frosted glass over the cards). -->
<div class="filters glass">
  <div class="chips">
    {#each filters as f}
      <button class="chip" class:active={filter === f.value} onclick={() => (filter = f.value)}>
        {f.label}
      </button>
    {/each}
  </div>
  <div class="selects">
    <select class="select" bind:value={gameFilter}>
      <option value="all">All games</option>
      {#each games as g}
        <option value={g}>{g}</option>
      {/each}
    </select>
    <select class="select" bind:value={sort}>
      {#each sorts as s}
        <option value={s.value}>{s.label}</option>
      {/each}
    </select>
  </div>
</div>

{#if error}
  <p class="error">{error}</p>
{/if}

{#if shown.length === 0}
  <div class="empty">
    <p>Nothing here yet.</p>
    <p class="faint">Take a screenshot, record, or save a replay and it shows up here.</p>
  </div>
{:else}
  <div class="grid" class:room={selecting}>
    {#each shown as item (item.path)}
      {@const preview = previews[item.path]}
      <div
        class="card"
        class:selecting
        class:selected={selected[item.path]}
        data-path={item.path}
        use:lazy
      >
        <button class="open" onclick={(e) => onCardClick(e, item)} title={item.name}>
          <div class="thumb">
            {#if preview?.src}
              <!-- Fades in once loaded (instant with animations off). -->
              <img
                src={preview.src}
                alt=""
                decoding="async"
                onload={(e) => (e.currentTarget as HTMLImageElement).classList.add("loaded")}
              />
            {/if}
            <span class="badge">{kindLabel[item.kind]}</span>
            {#if preview?.duration != null}
              <span class="duration">{formatDuration(preview.duration)}</span>
            {/if}
            {#if selecting}
              <!-- Round tick box (top-left) while selecting. -->
              <span class="check" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="14" height="14">
                  <path d="M6 12.5l4 4 8-9" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </span>
            {/if}
            {#if item.kind !== "screenshot" && !selecting}
              <span class="play" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="18" height="18"><path d="M8 5v14l11-7z" fill="currentColor" /></svg>
              </span>
            {/if}
          </div>
          <div class="meta">
            <span class="game">{item.game}</span>
            <span class="faint">{formatDate(item.modified)} · {formatSize(item.size)}</span>
          </div>
        </button>
        {#if !selecting}
          <button
            class="delete"
            title="Move to Recycle Bin"
            aria-label="Move to Recycle Bin"
            onclick={() => remove(item)}
          >
            <!-- Outline trash can; the lid lifts a little on hover. -->
            <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true">
              <g class="lid">
                <path d="M4 7h16" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
                <path d="M9.5 7V5.2c0-.7.5-1.2 1.2-1.2h2.6c.7 0 1.2.5 1.2 1.2V7" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
              </g>
              <path d="M6.5 7l.8 11.3c.1 1 .9 1.7 1.9 1.7h5.6c1 0 1.8-.7 1.9-1.7L17.5 7" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
              <path d="M10.2 11v5M13.8 11v5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<!-- Bulk actions bar, at the bottom while selecting. -->
{#if selecting}
  <div class="bulk" transition:fly={{ y: 20, duration: ms(200) }}>
    <span class="count">
      {selectedCount} selected
      {#if selectedCount > 0}<span class="faint">· {formatSize(selectedSize)}</span>{/if}
    </span>
    <button class="btn ghost" onclick={selectAll}>Select all</button>
    <button class="btn ghost" onclick={() => ((selected = {}), (confirming = false))} disabled={selectedCount === 0}>
      Clear
    </button>
    <button
      class="btn danger-solid"
      class:confirm={confirming}
      disabled={selectedCount === 0 || deleting}
      onclick={deleteSelected}
    >
      {#if deleting}
        Deleting…
      {:else if confirming}
        Delete {selectedCount} {selectedCount === 1 ? "file" : "files"}?
      {:else}
        Delete
      {/if}
    </button>
    <button class="btn ghost" onclick={stopSelecting}>Cancel</button>
  </div>
{/if}

{#if viewing}
  <!-- The viewer follows the file by its path, so new saves arriving
       while it's open don't make it jump to another file. -->
  <Viewer
    items={shown}
    bind:index={
      () => shown.findIndex((i) => i.path === viewing),
      (v) => (viewing = shown[v]?.path ?? null)
    }
    onclose={() => (viewing = null)}
    ondelete={removeFromViewer}
  />
{/if}

<style>
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }
  h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.3px;
  }

  .filters {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 0 -12px 18px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .chips {
    display: flex;
    gap: 4px;
  }
  .chip {
    padding: 5px 13px;
    border: 1px solid transparent;
    border-radius: 999px;
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease);
  }
  .chip:hover {
    color: var(--text);
    background: var(--surface-hover);
  }
  .chip.active {
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
    color: var(--text);
  }
  .selects {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .selects .select {
    max-width: 190px;
  }

  .empty {
    padding: 60px 0;
    text-align: center;
  }
  .empty p {
    margin: 4px 0;
  }

  .grid.room {
    padding-bottom: 70px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 16px;
  }
  /* Hover: just a clean white (black in light mode) border, nothing moves. */
  .card {
    position: relative;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    overflow: hidden;
    transition: border-color var(--fast) var(--ease);
  }
  .card:hover {
    border-color: var(--hover-border);
  }
  .open {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .thumb {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    background: var(--surface-2);
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transition: opacity var(--normal) var(--ease);
  }
  .thumb :global(img.loaded) {
    opacity: 1;
  }
  .badge,
  .duration {
    position: absolute;
    padding: 2px 7px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 11px;
  }
  .badge {
    left: 8px;
    top: 8px;
  }
  .duration {
    right: 8px;
    bottom: 8px;
    font-variant-numeric: tabular-nums;
  }
  /* Play icon, shown on hover over videos. */
  .play {
    position: absolute;
    left: 50%;
    top: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    margin: -20px 0 0 -20px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    opacity: 0;
    transition: opacity var(--fast) var(--ease);
  }
  .card:hover .play {
    opacity: 1;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px 12px;
    font-size: 12px;
  }
  .game {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  /* Trash button: a round glass button that fades + scales in while hovering the
     card, turns red when pointed at, and its lid lifts a little. */
  .delete {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 50%;
    background: rgba(10, 10, 12, 0.6);
    backdrop-filter: blur(8px);
    color: rgba(255, 255, 255, 0.9);
    cursor: pointer;
    opacity: 0;
    transform: scale(0.85);
    transition:
      opacity var(--normal) var(--ease),
      transform var(--normal) var(--ease),
      background var(--fast) var(--ease),
      border-color var(--fast) var(--ease);
  }
  :global(:root[data-blur="off"]) .delete {
    backdrop-filter: none;
    background: rgba(10, 10, 12, 0.85);
  }
  .card:hover .delete,
  .delete:focus-visible {
    opacity: 1;
    transform: scale(1);
  }
  .delete:hover {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .delete:active {
    transform: scale(0.9);
  }
  .delete .lid {
    transform-origin: 6px 7px;
    transition: transform var(--normal) var(--ease);
  }
  .delete:hover .lid {
    transform: translateY(-1.2px) rotate(-12deg);
  }

  /* --- Selecting --- */
  .header-actions {
    display: flex;
    gap: 4px;
  }
  .btn.on {
    background: var(--accent-soft);
    color: var(--text);
  }
  .card.selected {
    border-color: var(--hover-border);
    box-shadow: 0 0 0 1px var(--hover-border);
  }
  .card.selected .thumb img {
    opacity: 0.75;
  }
  .check {
    position: absolute;
    left: 8px;
    bottom: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: 1.5px solid rgba(255, 255, 255, 0.85);
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.35);
    color: transparent;
    transition:
      background var(--fast) var(--ease),
      color var(--fast) var(--ease),
      transform var(--fast) var(--ease);
  }
  .card.selected .check {
    border-color: #fff;
    background: #fff;
    color: #000;
    transform: scale(1.06);
  }

  .bulk {
    position: fixed;
    left: calc(50% + 100px);
    bottom: 22px;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 8px 8px 16px;
    border: 1px solid var(--border-strong);
    border-radius: 14px;
    background: var(--surface-2);
    box-shadow: var(--shadow);
    transform: translateX(-50%);
    white-space: nowrap;
  }
  .count {
    margin-right: 8px;
    font-size: 13px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .btn:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  .danger-solid {
    border-color: var(--danger);
    background: var(--danger);
    color: #fff;
  }
  .danger-solid:hover {
    background: #b91c1c;
  }
  .danger-solid.confirm {
    background: #b91c1c;
    box-shadow: 0 0 0 3px rgba(220, 38, 38, 0.3);
  }
</style>
