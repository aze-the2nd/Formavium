<script lang="ts">
  // Befehlspalette (Strg+K): alle Befehle, Module, Vorlagen und zuletzt
  // geöffnete Dokumente über die Tastatur.
  import { tick } from 'svelte';
  import { score, type Cmd } from './commands';

  type Props = { open: boolean; commands: Cmd[] };
  let { open = $bindable(), commands }: Props = $props();

  let query = $state('');
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLUListElement>();

  let hits = $derived.by(() => {
    const usable = commands.filter((c) => c.enabled !== false);
    if (!query.trim()) return usable.slice(0, 60);
    return usable
      .map((c) => ({ c, s: score(query, `${c.group}: ${c.title} ${c.detail ?? ''}`) }))
      .filter((x) => x.s >= 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 60)
      .map((x) => x.c);
  });

  $effect(() => {
    if (open) {
      query = '';
      active = 0;
      tick().then(() => input?.focus());
    }
  });

  $effect(() => {
    void hits;
    active = 0;
  });

  function run(c: Cmd | undefined) {
    if (!c) return;
    open = false;
    c.run();
  }

  async function move(delta: number) {
    if (!hits.length) return;
    active = (active + delta + hits.length) % hits.length;
    await tick();
    list?.querySelector('.active')?.scrollIntoView({ block: 'nearest' });
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
    else if (e.key === 'ArrowDown') move(1);
    else if (e.key === 'ArrowUp') move(-1);
    else if (e.key === 'Enter') run(hits[active]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (open = false)}>
    <div class="palette" role="dialog" aria-modal="true" aria-label="Befehlspalette">
      <input
        bind:this={input}
        bind:value={query}
        placeholder="Befehl, Modul oder Dokument suchen …"
        aria-label="Befehl suchen"
        spellcheck="false"
        onkeydown={key}
      />
      <ul bind:this={list} role="listbox">
        {#each hits as c, i (c.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <li
            role="option"
            aria-selected={i === active}
            class:active={i === active}
            onpointermove={() => (active = i)}
            onclick={() => run(c)}
          >
            <span class="group">{c.group}</span>
            <span class="title">{c.title}</span>
            {#if c.detail}<span class="detail">{c.detail}</span>{/if}
            {#if c.keys}<kbd>{c.keys}</kbd>{/if}
          </li>
        {:else}
          <li class="none">Nichts gefunden</li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 30, 0.3);
    z-index: 100;
    display: grid;
    justify-items: center;
    align-items: start;
    padding: 12vh 16px 16px;
  }
  .palette {
    width: min(620px, 100%);
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }
  input {
    font: inherit;
    font-size: 15px;
    color: var(--text);
    background: none;
    border: none;
    border-bottom: 1px solid var(--line);
    padding: 14px 16px;
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    overflow: auto;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 7px 12px;
    border-radius: 7px;
    font-size: 13px;
    cursor: default;
  }
  li.active {
    background: var(--accent-soft);
  }
  .group {
    flex: none;
    width: 78px;
    font-size: 11px;
    color: var(--muted);
  }
  .title {
    white-space: nowrap;
  }
  .detail {
    flex: 1;
    min-width: 0;
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  kbd {
    margin-left: auto;
    font: inherit;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .none {
    color: var(--muted);
    justify-content: center;
    padding: 18px;
  }
</style>
