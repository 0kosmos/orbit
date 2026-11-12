<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Segmented from "$lib/Segmented.svelte";

  // Crop + resize for screenshots. The crop frame is kept in image pixels; on screen
  // it's drawn with percentages, so it fits whatever size the image is shown at.
  let {
    src,
    path,
    onclose,
    onsaved,
  }: {
    src: string;
    path: string;
    onclose: () => void;
    onsaved: (newPath: string) => void;
  } = $props();

  let nw = $state(0); // image width in pixels
  let nh = $state(0); // image height in pixels
  let crop = $state({ x: 0, y: 0, w: 0, h: 0 });
  let canvas: HTMLDivElement;

  let aspect = $state("free");
  let scale = $state(100);
  // The image's own format (the editor starts fresh for every image).
  const originalFormat = () => (path.toLowerCase().endsWith(".jpg") ? "jpg" : "png");
  let format = $state(originalFormat());
  let busy = $state(false);
  let error = $state("");

  const ratios: Record<string, number | null> = { free: null, "16:9": 16 / 9, "4:3": 4 / 3, "1:1": 1 };
  const aspectOptions = Object.keys(ratios).map((k) => ({ value: k, label: k === "free" ? "Free" : k }));
  const scaleOptions = [100, 75, 50, 25].map((v) => ({ value: v, label: `${v}%` }));
  const formatOptions = [
    { value: "png", label: "PNG" },
    { value: "jpg", label: "JPG" },
  ];

  function onLoad(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    nw = img.naturalWidth;
    nh = img.naturalHeight;
    crop = { x: 0, y: 0, w: nw, h: nh };
  }

  // Picking a shape: the biggest frame of that shape, centered.
  function applyAspect() {
    const r = ratios[aspect];
    if (!r) return;
    let w = Math.min(nw, nh * r);
    let h = w / r;
    crop = { x: Math.round((nw - w) / 2), y: Math.round((nh - h) / 2), w: Math.round(w), h: Math.round(h) };
  }

  function reset() {
    aspect = "free";
    scale = 100;
    crop = { x: 0, y: 0, w: nw, h: nh };
  }

  // --- Dragging ---
  type Mode = "move" | "nw" | "ne" | "sw" | "se" | "new";
  let drag: { mode: Mode; ax: number; ay: number; sx: number; sy: number; start: typeof crop } | null = null;
  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

  // Mouse position in image pixels.
  function toImage(e: PointerEvent) {
    const r = canvas.getBoundingClientRect();
    return {
      px: clamp(((e.clientX - r.left) / r.width) * nw, 0, nw),
      py: clamp(((e.clientY - r.top) / r.height) * nh, 0, nh),
    };
  }

  function onPointerDown(e: PointerEvent) {
    if (!nw) return;
    const mode = ((e.target as HTMLElement).dataset.mode ?? "new") as Mode;
    const { px, py } = toImage(e);
    const { x, y, w, h } = crop;
    // The corner that stays put while resizing (the opposite one).
    const anchors: Record<string, [number, number]> = {
      nw: [x + w, y + h],
      ne: [x, y + h],
      sw: [x + w, y],
      se: [x, y],
      new: [px, py],
      move: [x, y],
    };
    const [ax, ay] = anchors[mode];
    drag = { mode, ax, ay, sx: px, sy: py, start: { ...crop } };
    canvas.setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag) return;
    const { px, py } = toImage(e);
    if (drag.mode === "move") {
      const { w, h } = drag.start;
      crop = {
        x: Math.round(clamp(drag.start.x + px - drag.sx, 0, nw - w)),
        y: Math.round(clamp(drag.start.y + py - drag.sy, 0, nh - h)),
        w,
        h,
      };
      return;
    }
    // Resizing / drawing: from the fixed corner to the mouse, keeping the shape if one is chosen.
    let w = Math.abs(px - drag.ax);
    let h = Math.abs(py - drag.ay);
    const r = ratios[aspect];
    if (r) {
      if (w / Math.max(h, 1) > r) w = h * r;
      else h = w / r;
    }
    w = Math.max(w, 16);
    h = Math.max(h, 16);
    const x = clamp(px < drag.ax ? drag.ax - w : drag.ax, 0, nw - w);
    const y = clamp(py < drag.ay ? drag.ay - h : drag.ay, 0, nh - h);
    crop = { x: Math.round(x), y: Math.round(y), w: Math.round(w), h: Math.round(h) };
  }

  function onPointerUp() {
    drag = null;
  }

  // Final size after cropping and resizing.
  let outW = $derived(Math.max(1, Math.round((crop.w * scale) / 100)));
  let outH = $derived(Math.max(1, Math.round((crop.h * scale) / 100)));
  let isFull = $derived(crop.x === 0 && crop.y === 0 && crop.w === nw && crop.h === nh);
  let unchanged = $derived(isFull && scale === 100 && format === originalFormat());

  async function save(replace: boolean) {
    busy = true;
    error = "";
    try {
      const saved = await invoke<string>("edit_image", {
        path,
        crop: isFull ? null : crop,
        width: scale === 100 ? null : outW,
        format,
        replace,
      });
      onsaved(saved);
    } catch (e) {
      error = String(e);
    }
    busy = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="editor">
  <div class="area">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="canvas"
      bind:this={canvas}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
    >
      <img {src} alt="" draggable="false" onload={onLoad} />
      {#if nw}
        <!-- Darkens everything outside the frame (clipped to the image). -->
        <div class="shade">
          <div
            class="hole"
            style:left="{(crop.x / nw) * 100}%"
            style:top="{(crop.y / nh) * 100}%"
            style:width="{(crop.w / nw) * 100}%"
            style:height="{(crop.h / nh) * 100}%"
          ></div>
        </div>
        <div
          class="frame"
          data-mode="move"
          style:left="{(crop.x / nw) * 100}%"
          style:top="{(crop.y / nh) * 100}%"
          style:width="{(crop.w / nw) * 100}%"
          style:height="{(crop.h / nh) * 100}%"
        >
          <span class="grid-lines" data-mode="move"></span>
          {#each ["nw", "ne", "sw", "se"] as corner}
            <span class="handle {corner}" data-mode={corner}></span>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="toolbar">
    <div class="group">
      <span class="label">Crop</span>
      <Segmented options={aspectOptions} bind:value={aspect} onchange={applyAspect} />
    </div>
    <div class="group">
      <span class="label">Size</span>
      <Segmented options={scaleOptions} bind:value={scale} />
    </div>
    <div class="group">
      <span class="label">Format</span>
      <Segmented options={formatOptions} bind:value={format} />
    </div>
    <div class="info">
      <span>{nw}×{nh} → <strong>{outW}×{outH}</strong></span>
      {#if error}<span class="error">{error}</span>{/if}
    </div>
    <div class="buttons">
      <button class="btn ghost" onclick={reset} disabled={busy}>Reset</button>
      <button class="btn ghost" onclick={onclose} disabled={busy}>Cancel</button>
      <button class="btn" onclick={() => save(true)} disabled={busy || unchanged}>Replace original</button>
      <button class="btn primary" onclick={() => save(false)} disabled={busy || unchanged}>
        {busy ? "Saving…" : "Save as copy"}
      </button>
    </div>
  </div>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  .area {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    container-type: size;
  }
  .canvas {
    position: relative;
    line-height: 0;
    touch-action: none;
    cursor: crosshair;
    user-select: none;
  }
  img {
    display: block;
    max-width: min(1100px, 100cqw);
    max-height: 100cqh;
    border-radius: 4px;
  }

  /* Everything outside the kept area is darkened by a huge shadow around a "hole". */
  .shade {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: 4px;
    pointer-events: none;
  }
  .hole {
    position: absolute;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.6);
  }
  /* The frame itself (with its corner handles) may stick out over the image edge. */
  .frame {
    position: absolute;
    outline: 1.5px solid #fff;
    cursor: move;
  }
  /* Rule-of-thirds lines inside the frame. */
  .grid-lines {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(to right, transparent calc(33.33% - 0.5px), rgba(255, 255, 255, 0.35) 0 calc(33.33% + 0.5px), transparent 0 calc(66.66% - 0.5px), rgba(255, 255, 255, 0.35) 0 calc(66.66% + 0.5px), transparent 0),
      linear-gradient(to bottom, transparent calc(33.33% - 0.5px), rgba(255, 255, 255, 0.35) 0 calc(33.33% + 0.5px), transparent 0 calc(66.66% - 0.5px), rgba(255, 255, 255, 0.35) 0 calc(66.66% + 0.5px), transparent 0);
  }
  .handle {
    position: absolute;
    width: 14px;
    height: 14px;
    border: 2px solid #000;
    border-radius: 50%;
    background: #fff;
  }
  .handle.nw {
    left: -7px;
    top: -7px;
    cursor: nwse-resize;
  }
  .handle.se {
    right: -7px;
    bottom: -7px;
    cursor: nwse-resize;
  }
  .handle.ne {
    right: -7px;
    top: -7px;
    cursor: nesw-resize;
  }
  .handle.sw {
    left: -7px;
    bottom: -7px;
    cursor: nesw-resize;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 18px;
    padding: 12px 14px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .label {
    font-size: 12px;
    color: var(--text-faint);
  }
  .info {
    display: flex;
    flex-direction: column;
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .info strong {
    color: var(--text);
  }
  .buttons {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  .primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
  }
  .primary:hover {
    background: var(--accent);
    opacity: 0.9;
  }
  .btn:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
</style>
