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
</script>

<div class="segmented" role="radiogroup">
  {#each options as o}
    <button role="radio" aria-checked={o.value === value} class:on={o.value === value} onclick={() => pick(o.value)}>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    padding: 2px;
    border: 1px solid #222;
    border-radius: 8px;
    background: #0d0d0d;
  }
  button {
    padding: 4px 12px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #888;
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  button:hover {
    color: #fff;
  }
  button.on {
    background: #222;
    color: #fff;
  }
</style>
