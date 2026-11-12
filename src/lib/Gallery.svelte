<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

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

<div class="header">
  <h1>Gallery</h1>
  <button class="link" onclick={() => invoke("open_media_folder")}>Open folder</button>
</div>

<div class="filters">
  {#each filters as f}
    <button class:active={filter === f.value} onclick={() => (filter = f.value)}>{f.label}</button>
  {/each}
  <div class="selects">
    <select bind:value={gameFilter}>
      <option value="all">All games</option>
      {#each games as g}
        <option value={g}>{g}</option>
      {/each}
    </select>
    <select bind:value={sort}>
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
  <p class="muted">Nothing here yet.</p>
{:else}
  <div class="grid">
    {#each shown as item (item.path)}
      {@const preview = previews[item.path]}
      <div class="card" data-path={item.path} use:lazy>
        <button
          class="open"
          onclick={() => invoke("open_media", { path: item.path })}
          title={item.name}
        >
          <div class="thumb">
            {#if preview?.src}
              <img src={preview.src} alt="" decoding="async" />
            {/if}
            <span class="badge">{kindLabel[item.kind]}</span>
            {#if preview?.duration != null}
              <span class="duration">{formatDuration(preview.duration)}</span>
            {/if}
          </div>
          <div class="meta">
            <span class="game">{item.game}</span>
            <span class="muted">{formatDate(item.modified)} · {formatSize(item.size)}</span>
          </div>
        </button>
        <button class="delete" title="Move to Recycle Bin" onclick={() => remove(item)}>
          <!-- trash can icon -->
          <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
            <path
              fill="currentColor"
              d="M9 3h6l1 2h4v2H4V5h4l1-2zm-3 6h12l-1 12H7L6 9zm4 2v8h2v-8h-2zm4 0v8h-2v-8h2z"
            />
          </svg>
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  h1 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
  .muted {
    color: #777;
  }
  .error {
    color: #ff9a9a;
  }
  button {
    font: inherit;
    cursor: pointer;
  }
  .link {
    border: none;
    background: none;
    color: #999;
    padding: 0;
  }
  .link:hover {
    color: #fff;
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 18px;
  }
  .filters button {
    padding: 5px 12px;
    border: 1px solid #222;
    border-radius: 999px;
    background: transparent;
    color: #999;
  }
  .filters button:hover {
    color: #fff;
  }
  .filters button.active {
    background: #1a1a1a;
    border-color: #333;
    color: #fff;
  }
  .selects {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .selects select {
    max-width: 200px;
    padding: 5px 8px;
    border: 1px solid #222;
    border-radius: 6px;
    background: #111;
    color: #e6e6e6;
    font: inherit;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 14px;
  }
  .card {
    position: relative;
    border: 1px solid #1a1a1a;
    border-radius: 8px;
    background: #0a0a0a;
    overflow: hidden;
  }
  .card:hover {
    border-color: #333;
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
  }
  .thumb {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    background: #111;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .badge,
  .duration {
    position: absolute;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.7);
    font-size: 11px;
    color: #ccc;
  }
  .badge {
    left: 6px;
    top: 6px;
  }
  .duration {
    right: 6px;
    bottom: 6px;
    color: #fff;
    font-variant-numeric: tabular-nums;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    font-size: 12px;
  }
  .game {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  /* Delete button: only visible while hovering the card. */
  .delete {
    position: absolute;
    top: 6px;
    right: 6px;
    display: none;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.75);
    color: #ccc;
  }
  .card:hover .delete {
    display: flex;
  }
  .delete:hover {
    background: #b3261e;
    color: #fff;
  }
</style>
