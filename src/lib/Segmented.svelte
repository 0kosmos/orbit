<script lang="ts" generics="T extends string | number">
  // A row of buttons where exactly one is selected (like a radio group).
  let {
    options,
    value = $bindable(),
    onchange,
  }: {
    options: { value: T; label: string }[];
    value: T;
    onchange?: () => void;
  } = $props();

  function pick(v: T) {
    if (v === value) return;
    value = v;
    onchange?.();
  }

  // The highlight slides to the chosen button (by index, all buttons equal width).
  let index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));
</script>

<div class="segmented" role="radiogroup" style:--count={options.length} style:--index={index}>
  <span class="highlight" aria-hidden="true"></span>
  {#each options as o}
    <button
      role="radio"
      aria-checked={o.value === value}
      class:on={o.value === value}
      onclick={() => pick(o.value)}
    >
      {o.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    position: relative;
    display: inline-grid;
    grid-template-columns: repeat(var(--count), 1fr);
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .highlight {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc((100% - 6px) / var(--count));
    border-radius: 5px;
    background: var(--surface-hover);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transform: translateX(calc(100% * var(--index)));
    transition: transform var(--normal) var(--ease);
  }
  button {
    position: relative;
    padding: 5px 12px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    white-space: nowrap;
    cursor: pointer;
    transition: color var(--fast) var(--ease);
  }
  button:hover,
  button.on {
    color: var(--text);
  }
</style>
