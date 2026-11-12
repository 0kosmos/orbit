<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";

  // Bottom bar: Orbit version · CPU · GPU · RAM · time.
  // Rust sends "stats" every 2 s while the window is open (nothing when it's closed);
  // the clock is updated with each of those, so there's no extra timer.
  type Stats = {
    cpu: number;
    gpu: number | null;
    ram_used: number;
    ram_total: number;
    orbit_ram: number;
  };

  let { compact = false }: { compact?: boolean } = $props();

  let version = $state("");
  let stats = $state<Stats | null>(null);
  let time = $state(clock());

  function clock() {
    return new Date().toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }

  $effect(() => {
    getVersion().then((v) => (version = v));
    const stop = listen<Stats>("stats", (e) => {
      stats = e.payload;
      time = clock();
    });
    return () => stop.then((s) => s());
  });

  const gb = (bytes: number) => (bytes / 1024 ** 3).toFixed(1);
  const mb = (bytes: number) => `${Math.round(bytes / 1024 ** 2)} MB`;
  // Bar color: calm under 70 %, warmer above.
  const level = (pct: number) => (pct >= 90 ? "high" : pct >= 70 ? "mid" : "low");
  let ramPct = $derived(stats && stats.ram_total ? (stats.ram_used / stats.ram_total) * 100 : 0);
</script>

{#snippet meter(label: string, pct: number | null, text: string)}
  <span class="item" title="{label} usage of the whole PC">
    <span class="label">{label}</span>
    <span class="bar">
      <span class="fill {level(pct ?? 0)}" style:width="{pct ?? 0}%"></span>
    </span>
    <span class="value">{text}</span>
  </span>
{/snippet}

<footer class="status" class:compact>
  <span class="item brand">Orbit <span class="faint">v{version}</span></span>
  <span class="spacer"></span>
  {#if stats}
    <!-- Orbit's own memory (app + window + FFmpeg) -->
    <span class="item own" title="Memory used by Orbit itself (app, window and FFmpeg)">
      <span class="label">ORBIT</span>
      <span class="value">{mb(stats.orbit_ram)}</span>
    </span>
    <span class="divider"></span>
    <!-- The whole PC -->
    <span class="label pc" title="Usage of the whole PC (all programs together)">PC</span>
    {@render meter("CPU", stats.cpu, `${Math.round(stats.cpu)}%`)}
    {#if stats.gpu !== null}
      {@render meter("GPU", stats.gpu, `${Math.round(stats.gpu)}%`)}
    {/if}
    {@render meter("RAM", ramPct, `${gb(stats.ram_used)} / ${gb(stats.ram_total)} GB`)}
    <span class="divider"></span>
  {:else}
    <span class="item faint">Measuring…</span>
  {/if}
  <span class="item time">{time}</span>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 18px;
    height: 28px;
    padding: 0 14px;
    border-top: 1px solid var(--border);
    background: var(--bg-sidebar);
    color: var(--text-muted);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
  }
  .compact {
    gap: 12px;
    height: 24px;
    padding: 0 10px;
    border: none;
    background: transparent;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .brand {
    color: var(--text);
    font-weight: 500;
  }
  .spacer {
    flex: 1;
  }
  .label {
    color: var(--text-faint);
    font-weight: 600;
    letter-spacing: 0.4px;
  }
  .bar {
    width: 34px;
    height: 4px;
    border-radius: 999px;
    background: var(--border-strong);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 999px;
    transition: width var(--normal) var(--ease);
  }
  .fill.low {
    background: var(--text-muted);
  }
  .fill.mid {
    background: #f59e0b;
  }
  .fill.high {
    background: var(--rec);
  }
  .value {
    min-width: 30px;
    color: var(--text);
  }
  .time {
    color: var(--text);
  }
  .divider {
    width: 1px;
    height: 12px;
    background: var(--border-strong);
  }
  .pc {
    margin-right: -10px;
    padding: 1px 5px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    font-size: 10px;
  }
</style>
