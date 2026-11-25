<script lang="ts">
  import { closeMenu, menuState, type MenuItem } from './menus.svelte';

  let el = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });
  let active = $state(-1);

  type Action = Extract<MenuItem, { label: string }>;
  const isAction = (i: MenuItem | undefined): i is Action => !!i && 'label' in i && !i.disabled;

  // Nach dem Öffnen im Fenster halten und den Fokus übernehmen.
  $effect(() => {
    const m = menuState.current;
    if (!m || !el) return;
    const r = el.getBoundingClientRect();
    pos = {
      x: Math.max(4, Math.min(m.x, window.innerWidth - r.width - 4)),
      y: Math.max(4, Math.min(m.y, window.innerHeight - r.height - 4)),
    };
    active = -1;
    el.focus({ preventScroll: true });
  });

  function run(item: MenuItem) {
    if (!isAction(item)) return;
    closeMenu();
    item.action();
  }

  function step(delta: number) {
    const items = menuState.current?.items ?? [];
    let i = active;
    for (let n = 0; n < items.length; n++) {
      i = (i + delta + items.length) % items.length;
      if (isAction(items[i])) {
        active = i;
        return;
      }
    }
  }

  function onKey(e: KeyboardEvent) {
    const items = menuState.current?.items ?? [];
    if (e.key === 'Escape') closeMenu();
    else if (e.key === 'ArrowDown') step(1);
    else if (e.key === 'ArrowUp') step(active < 0 ? 0 : -1);
    else if (e.key === 'Home') (active = -1), step(1);
    else if (e.key === 'End') (active = 0), step(-1);
    else if (e.key === 'Enter' || e.key === ' ') run(items[active]);
    else if (e.key !== 'Tab') return;
    e.preventDefault();
    e.stopPropagation();
  }

  function outside(e: Event) {
    if (menuState.current && !(e.target instanceof Node && el?.contains(e.target))) closeMenu(false);
  }
</script>

<svelte:window
  onpointerdowncapture={outside}
  onwheelcapture={outside}
  onresize={() => closeMenu(false)}
  onblur={() => closeMenu(false)}
/>

{#if menuState.current}
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    bind:this={el}
    style:left="{pos.x}px"
    style:top="{pos.y}px"
    onkeydown={onKey}
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#each menuState.current.items as item, i}
      {#if 'separator' in item}
        <div class="sep" role="separator"></div>
      {:else if 'heading' in item}
        <div class="heading">{item.heading}</div>
      {:else}
        <button
          role="menuitem"
          tabindex="-1"
          class:active={active === i}
          class:danger={item.danger}
          disabled={item.disabled}
          onpointerenter={() => (active = item.disabled ? -1 : i)}
          onclick={() => run(item)}
        >
          <span>{item.label}</span>
          {#if item.hint}<kbd>{item.hint}</kbd>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 90;
    min-width: 220px;
    max-width: 360px;
    max-height: calc(100vh - 8px);
    overflow: auto;
    padding: 4px;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 9px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.18);
    outline: none;
    font-size: 13px;
  }
  button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    width: 100%;
    text-align: left;
    font: inherit;
    color: var(--text);
    background: none;
    border: none;
    border-radius: 6px;
    padding: 6px 10px;
    cursor: default;
  }
  button.active {
    background: var(--accent-soft);
  }
  button.danger {
    color: var(--danger);
  }
  button.danger.active {
    background: var(--danger-soft);
  }
  button:disabled {
    color: var(--muted);
    opacity: 0.55;
  }
  kbd {
    font: inherit;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 4px 6px;
  }
  .heading {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    padding: 6px 10px 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
