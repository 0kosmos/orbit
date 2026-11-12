<script lang="ts">
  // Region screenshot ("snip"): the frozen screenshot fills the screen; drag the part
  // you want, adjust it, then Enter = save + copy to clipboard, Esc = cancel.
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { fade } from "svelte/transition";
  import { applyUi, ms } from "$lib/ui.svelte";

  let src = $state("");
  let nw = 0; // image size in pixels (= the monitor's real resolution)
  let nh = 0;
  let busy = $state(false);
  let error = $state("");

  // Selection on screen (CSS pixels). w = 0 means nothing selected yet.
  let sel = $state({ x: 0, y: 0, w: 0, h: 0 });
  let dragging = $state(false);

  $effect(() => {
    invoke<{ animations: boolean; blur: boolean; theme: string }>("get_settings").then(applyUi);
    invoke<{ path: string } | null>("snip_info").then((info) => {
      if (info) src = `${convertFileSrc(info.path)}?v=${Date.now()}`;
      else invoke("snip_cancel");
    });
  });

  function onLoad(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    nw = img.naturalWidth;
    nh = img.naturalHeight;
    // The window is still invisible: show it once the picture is fully decoded.
    // (The window is also see-through, so even a frame too early just shows the real
    // screen, which looks the same as the frozen picture: never a black flash.)
    img
      .decode()
      .catch(() => {})
      .then(() => invoke("snip_ready"));
  }

  // --- Drawing / moving / resizing the selection ---
  type Mode = "new" | "move" | "nw" | "ne" | "sw" | "se";
  let drag: { mode: Mode; ax: number; ay: number; sx: number; sy: number; start: typeof sel } | null = null;
  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || busy) return;
    menu = null;
    const mode = ((e.target as HTMLElement).dataset.mode ?? "new") as Mode;
    const { x, y, w, h } = sel;
    const anchors: Record<Mode, [number, number]> = {
      new: [e.clientX, e.clientY],
      move: [x, y],
      nw: [x + w, y + h],
      ne: [x, y + h],
      sw: [x + w, y],
      se: [x, y],
    };
    const [ax, ay] = anchors[mode];
    drag = { mode, ax, ay, sx: e.clientX, sy: e.clientY, start: { ...sel } };
    if (mode === "new") sel = { x: ax, y: ay, w: 0, h: 0 };
    dragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag) return;
    const W = innerWidth;
    const H = innerHeight;
    const px = clamp(e.clientX, 0, W);
    const py = clamp(e.clientY, 0, H);
    if (drag.mode === "move") {
      const { w, h } = drag.start;
      sel = {
        x: clamp(drag.start.x + px - drag.sx, 0, W - w),
        y: clamp(drag.start.y + py - drag.sy, 0, H - h),
        w,
        h,
      };
      return;
    }
    sel = {
      x: Math.min(px, drag.ax),
      y: Math.min(py, drag.ay),
      w: Math.abs(px - drag.ax),
      h: Math.abs(py - drag.ay),
    };
  }

  function onPointerUp() {
    drag = null;
    dragging = false;
    // A tiny click without dragging isn't a selection.
    if (sel.w < 4 || sel.h < 4) sel = { x: 0, y: 0, w: 0, h: 0 };
  }

  // Screen pixels -> image pixels (the image may be scaled by Windows' display scaling).
  let fx = $derived(nw && innerWidth ? nw / innerWidth : 1);
  let fy = $derived(nh && innerHeight ? nh / innerHeight : 1);
  let cropPx = $derived({
    x: Math.round(sel.x * fx),
    y: Math.round(sel.y * fy),
    w: Math.max(1, Math.round(sel.w * fx)),
    h: Math.max(1, Math.round(sel.h * fy)),
  });
  let hasSel = $derived(sel.w >= 4 && sel.h >= 4);

  // keep = also save it as a screenshot file (false = only copy to the clipboard).
  async function save(full: boolean, keep = true) {
    if (busy) return;
    busy = true;
    error = "";
    menu = null;
    try {
      // The window closes itself once done.
      await invoke("snip_save", { crop: full ? null : cropPx, keep });
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (menu) menu = null;
      else invoke("snip_cancel");
    } else if (e.key === "Enter") save(!hasSel);
    else if (e.key === "c" && e.ctrlKey) save(!hasSel, false);
  }

  // Right-click: with a selection, a small menu (Copy / Save & copy / Cancel);
  // without one, cancel.
  let menu = $state<{ x: number; y: number } | null>(null);
  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    if (!hasSel) {
      invoke("snip_cancel");
      return;
    }
    menu = {
      x: Math.min(e.clientX, innerWidth - 200),
      y: Math.min(e.clientY, innerHeight - 130),
    };
  }

  // The toolbar sits under the selection, or above it if there's no room below.
  let toolbarTop = $derived(sel.y + sel.h + 10 + 48 > innerHeight ? Math.max(8, sel.y - 58) : sel.y + sel.h + 10);
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={onContextMenu} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="snip"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
>
  {#if src}
    <img {src} alt="" draggable="false" onload={onLoad} />
  {/if}

  {#if hasSel || dragging}
    <!-- Darkens everything except the selection -->
    <div
      class="frame"
      data-mode="move"
      style:left="{sel.x}px"
      style:top="{sel.y}px"
      style:width="{sel.w}px"
      style:height="{sel.h}px"
    >
      {#if !dragging}
        {#each ["nw", "ne", "sw", "se"] as corner}
          <span class="handle {corner}" data-mode={corner}></span>
        {/each}
      {/if}
      <span class="size">{cropPx.w} × {cropPx.h}</span>
    </div>
  {:else}
    <div class="dim"></div>
    <div class="hint" transition:fade={{ duration: ms(150) }}>
      Drag to select an area · <b>Enter</b> saves the whole screen · <b>Esc</b> cancels
    </div>
  {/if}

  {#if hasSel && !dragging}
    <div
      class="toolbar"
      style:left="{clamp(sel.x + sel.w / 2, 170, innerWidth - 170)}px"
      style:top="{toolbarTop}px"
      onpointerdown={(e) => e.stopPropagation()}
      transition:fade={{ duration: ms(120) }}
    >
      <button class="primary" disabled={busy} onclick={() => save(false)}>
        {busy ? "Saving…" : "Save & copy"} <kbd>Enter</kbd>
      </button>
      <button disabled={busy} onclick={() => save(false, false)}>Copy <kbd>Ctrl+C</kbd></button>
      <button disabled={busy} onclick={() => save(true)}>Full screen</button>
      <button disabled={busy} onclick={() => invoke("snip_cancel")}>Cancel <kbd>Esc</kbd></button>
    </div>
  {/if}

  {#if menu}
    <!-- Right-click menu -->
    <div
      class="menu"
      style:left="{menu.x}px"
      style:top="{menu.y}px"
      onpointerdown={(e) => e.stopPropagation()}
      transition:fade={{ duration: ms(100) }}
    >
      <button onclick={() => save(false, false)}>Copy<kbd>Ctrl+C</kbd></button>
      <button onclick={() => save(false)}>Save & copy<kbd>Enter</kbd></button>
      <div class="sep"></div>
      <button onclick={() => invoke("snip_cancel")}>Cancel<kbd>Esc</kbd></button>
    </div>
  {/if}

  {#if error}
    <div class="error-box">{error}</div>
  {/if}
</div>

<style>
  .snip {
    position: fixed;
    inset: 0;
    overflow: hidden;
    cursor: crosshair;
    user-select: none;
    touch-action: none;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100vw;
    height: 100vh;
    display: block;
    pointer-events: none;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    pointer-events: none;
  }
  .frame {
    position: absolute;
    outline: 1.5px solid #fff;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.45);
    cursor: move;
  }
  .handle {
    position: absolute;
    width: 12px;
    height: 12px;
    border: 2px solid #000;
    border-radius: 50%;
    background: #fff;
  }
  .handle.nw {
    left: -6px;
    top: -6px;
    cursor: nwse-resize;
  }
  .handle.se {
    right: -6px;
    bottom: -6px;
    cursor: nwse-resize;
  }
  .handle.ne {
    right: -6px;
    top: -6px;
    cursor: nesw-resize;
  }
  .handle.sw {
    left: -6px;
    bottom: -6px;
    cursor: nesw-resize;
  }
  .size {
    position: absolute;
    left: 0;
    top: -26px;
    padding: 2px 7px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.75);
    color: #fff;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    pointer-events: none;
  }
  .hint {
    position: absolute;
    left: 50%;
    top: 28px;
    transform: translateX(-50%);
    padding: 9px 16px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    background: rgba(12, 12, 14, 0.8);
    color: rgba(255, 255, 255, 0.85);
    font-size: 13px;
    pointer-events: none;
  }
  .hint b {
    color: #fff;
    font-weight: 600;
  }

  .toolbar {
    position: absolute;
    display: flex;
    gap: 6px;
    padding: 6px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    background: rgba(12, 12, 14, 0.92);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    transform: translateX(-50%);
    cursor: default;
  }
  .toolbar button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    cursor: pointer;
  }
  .toolbar button:hover {
    background: rgba(255, 255, 255, 0.14);
  }
  .toolbar .primary {
    border-color: #fff;
    background: #fff;
    color: #000;
  }
  .toolbar .primary:hover {
    background: #e6e6e6;
  }
  .toolbar button:disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  kbd {
    padding: 0 5px;
    border: 1px solid color-mix(in srgb, currentColor 35%, transparent);
    border-radius: 4px;
    font-family: inherit;
    font-size: 10.5px;
    opacity: 0.7;
  }
  .menu {
    position: absolute;
    display: flex;
    flex-direction: column;
    min-width: 190px;
    padding: 5px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 10px;
    background: rgba(16, 16, 18, 0.96);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    cursor: default;
  }
  .menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 7px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #fff;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  .menu .sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255, 255, 255, 0.1);
  }
  .error-box {
    position: absolute;
    left: 50%;
    bottom: 30px;
    transform: translateX(-50%);
    padding: 10px 16px;
    border-radius: 10px;
    background: rgba(40, 10, 10, 0.92);
    color: #fca5a5;
    font-size: 13px;
  }
</style>
