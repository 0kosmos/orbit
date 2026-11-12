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

  const filters: { value: Kind | "all"; label: string }[] = [
    { value: "all", label: "All" },
    { value: "screenshot", label: "Screenshots" },
    { value: "clip", label: "Clips" },
    { value: "recording", label: "Recordings" },
  ];

  let items = $state<Item[]>([]);
  let filter = $state<Kind | "all">("all");
  let gameFilter = $state("all");
  // path -> thumbnail URL ("" if it couldn't be made)
  let thumbs = $state<Record<string, string>>({});

  // Every game that has something in the gallery, A–Z ("Other" last).
  let games = $derived(
    [...new Set(items.map((i) => i.game))].sort((a, b) =>
      a === "Other" ? 1 : b === "Other" ? -1 : a.localeCompare(b),
    ),
  );
  let shown = $derived(
    items.filter(
      (i) => (filter === "all" || i.kind === filter) && (gameFilter === "all" || i.game === gameFilter),
    ),
  );

  async function refresh() {
    items = await invoke<Item[]>("list_media");
  }

  // --- Lazy thumbnails ---
  // A thumbnail is only requested when its card scrolls into view,
  // and at most 3 are made at the same time.
  const queue: string[] = [];
  let busy = 0;

  function pump() {
    while (busy < 3 && queue.length > 0) {
      const path = queue.shift()!;
      busy++;
      invoke<string>("thumbnail", { path })
        .then((file) => (thumbs[path] = convertFileSrc(file)))
        .catch(() => (thumbs[path] = ""))
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
      if (!(path in thumbs) && !queue.includes(path)) {
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
  <select bind:value={gameFilter}>
    <option value="all">All games</option>
    {#each games as g}
      <option value={g}>{g}</option>
    {/each}
  </select>
</div>

{#if shown.length === 0}
  <p class="muted">Nothing here yet.</p>
{:else}
  <div class="grid">
    {#each shown as item (item.path)}
      <button
        class="card"
        data-path={item.path}
        use:lazy
        onclick={() => invoke("open_media", { path: item.path })}
        title={item.name}
      >
        <div class="thumb">
          {#if thumbs[item.path]}
            <img src={thumbs[item.path]} alt="" decoding="async" />
          {/if}
          <span class="badge">{kindLabel[item.kind]}</span>
        </div>
        <div class="meta">
          <span class="game">{item.game}</span>
          <span class="muted">{formatDate(item.modified)} · {formatSize(item.size)}</span>
        </div>
      </button>
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

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 14px;
  }
  .card {
    display: flex;
    flex-direction: column;
    padding: 0;
    border: 1px solid #1a1a1a;
    border-radius: 8px;
    background: #0a0a0a;
    color: inherit;
    text-align: left;
    overflow: hidden;
  }
  .card:hover {
    border-color: #333;
  }
  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    background: #111;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .badge {
    position: absolute;
    left: 6px;
    top: 6px;
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.7);
    font-size: 11px;
    color: #ccc;
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
  .filters select {
    margin-left: auto;
    max-width: 220px;
    padding: 5px 8px;
    border: 1px solid #222;
    border-radius: 6px;
    background: #111;
    color: #e6e6e6;
    font: inherit;
  }
</style>
