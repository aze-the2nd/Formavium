<script lang="ts">
  // Auswahl aus der Bibliothek: Module einfügen oder ein Dokument aus einer
  // Vorlage anlegen. Suche, UND-verknüpfte Inhaltsfilter, Kacheln oder Liste.
  import { tick } from 'svelte';
  import Miniature from './Miniature.svelte';
  import { score } from './commands';
  import { kindLabel, type LibModule, type LibTemplate, type LibraryView } from './library';
  import { openMenu, type MenuItem } from './menus.svelte';
  import type { ContentKind } from './api';

  export type PickerMode = { kind: 'module'; index: number } | { kind: 'template' } | { kind: 'manage' };

  type Props = {
    mode: PickerMode | null;
    library: LibraryView | null;
    onModule: (m: LibModule, index: number) => void;
    onTemplate: (t: LibTemplate) => void;
    onNewModule: (index: number) => void;
    onDelete: (item: { module: LibModule } | { template: LibTemplate }) => void;
    onRevealDir?: () => void;
  };
  let { mode = $bindable(), library, onModule, onTemplate, onNewModule, onDelete, onRevealDir }: Props = $props();

  let query = $state('');
  let filters = $state<ContentKind[]>([]);
  let tab = $state<'module' | 'template'>('module');
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let grid = $state<HTMLElement>();

  function loadListMode() {
    try {
      return localStorage.getItem('formavium.picker') === 'list';
    } catch {
      return false;
    }
  }
  let asList = $state(loadListMode());
  function setList(v: boolean) {
    asList = v;
    try {
      localStorage.setItem('formavium.picker', v ? 'list' : 'grid');
    } catch {
      /* Darstellung gilt dann nur für diese Sitzung */
    }
  }

  $effect(() => {
    if (!mode) return;
    tab = mode.kind === 'template' ? 'template' : 'module';
    query = '';
    filters = [];
    active = 0;
    tick().then(() => input?.focus());
  });

  // Je Kennung nur die neueste Version anbieten.
  let modules = $derived((library?.modules ?? []).filter((m) => m.latest));

  function ranked<T extends { name: string; description: string }>(items: T[]): T[] {
    if (!query.trim()) return items;
    return items
      .map((item) => ({ item, s: score(query, `${item.name} ${item.description}`) }))
      .filter((x) => x.s >= 0)
      .sort((a, b) => b.s - a.s)
      .map((x) => x.item);
  }

  let moduleHits = $derived(ranked(modules.filter((m) => filters.every((f) => m.kinds.includes(f)))));
  let templateHits = $derived(ranked(library?.templates ?? []));
  let count = $derived(tab === 'module' ? moduleHits.length : templateHits.length);

  $effect(() => {
    void [query, filters.length, tab];
    active = 0;
  });

  function toggleFilter(k: ContentKind) {
    filters = filters.includes(k) ? filters.filter((f) => f !== k) : [...filters, k];
    input?.focus();
  }

  function choose(i: number) {
    if (!mode) return;
    if (tab === 'module') {
      const m = moduleHits[i];
      if (!m || mode.kind === 'manage') return;
      const index = mode.kind === 'module' ? mode.index : 0;
      mode = null;
      onModule(m, index);
    } else {
      const t = templateHits[i];
      if (!t || mode.kind === 'manage') return;
      mode = null;
      onTemplate(t);
    }
  }

  function columns(): number {
    if (asList || !grid) return 1;
    const first = grid.querySelector<HTMLElement>('.item');
    return first ? Math.max(1, Math.round(grid.clientWidth / first.offsetWidth)) : 1;
  }

  async function move(delta: number) {
    if (!count) return;
    active = Math.max(0, Math.min(count - 1, active + delta));
    await tick();
    grid?.querySelector('.item.active')?.scrollIntoView({ block: 'nearest' });
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') mode = null;
    else if (e.key === 'ArrowDown') move(columns());
    else if (e.key === 'ArrowUp') move(-columns());
    else if (e.key === 'ArrowRight' && !asList && input?.selectionStart === query.length) move(1);
    else if (e.key === 'ArrowLeft' && !asList && input?.selectionStart === 0) move(-1);
    else if (e.key === 'Enter') choose(active);
    else if (e.key === 'Tab' && mode?.kind !== 'module') tab = tab === 'module' ? 'template' : 'module';
    else {
      e.stopPropagation();
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  }

  function context(e: MouseEvent, item: { module: LibModule } | { template: LibTemplate }) {
    e.preventDefault();
    const entry = 'module' in item ? item.module : item.template;
    const items: MenuItem[] = [
      { heading: entry.name },
      {
        label: 'Aus der Bibliothek löschen',
        danger: true,
        disabled: entry.source === 'builtin',
        action: () => onDelete(item),
      },
    ];
    openMenu({ x: e.clientX, y: e.clientY }, items);
  }

  const title = $derived(
    mode?.kind === 'module' ? 'Modul einfügen' : mode?.kind === 'template' ? 'Neues Dokument' : 'Bibliothek',
  );
</script>

{#if mode}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (mode = null)}>
    <div class="picker" role="dialog" aria-modal="true" aria-label={title}>
      <header>
        <input
          bind:this={input}
          bind:value={query}
          placeholder={tab === 'module' ? 'Modul suchen …' : 'Vorlage suchen …'}
          aria-label="Suchen"
          spellcheck="false"
          onkeydown={key}
        />
        <div class="toggle" role="group" aria-label="Darstellung">
          <button class:on={!asList} aria-pressed={!asList} title="Kacheln" onclick={() => setList(false)}>▦</button>
          <button class:on={asList} aria-pressed={asList} title="Liste" onclick={() => setList(true)}>☰</button>
        </div>
      </header>

      <div class="bar">
        {#if mode.kind !== 'module'}
          <div class="tabs" role="tablist">
            <button role="tab" aria-selected={tab === 'template'} class:on={tab === 'template'} onclick={() => (tab = 'template')}
              >Vorlagen</button
            >
            <button role="tab" aria-selected={tab === 'module'} class:on={tab === 'module'} onclick={() => (tab = 'module')}
              >Module</button
            >
          </div>
        {/if}
        {#if tab === 'module'}
          <div class="chips" role="group" aria-label="Inhalt enthält">
            {#each ['text', 'table', 'image', 'latex'] as const as k}
              <button class="chip" class:on={filters.includes(k)} aria-pressed={filters.includes(k)} onclick={() => toggleFilter(k)}
                >{kindLabel[k]}</button
              >
            {/each}
          </div>
        {/if}
        <span class="spacer"></span>
        {#if mode.kind === 'module'}
          {@const index = mode.index}
          <button
            class="ghost small"
            onclick={() => {
              mode = null;
              onNewModule(index);
            }}>Neues Modul erstellen …</button
          >
        {/if}
      </div>

      <div class="items" class:list={asList} bind:this={grid}>
        {#if tab === 'module'}
          {#each moduleHits as m, i (m.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div
              class="item"
              class:active={i === active}
              onpointermove={() => (active = i)}
              onclick={() => choose(i)}
              oncontextmenu={(e) => context(e, { module: m })}
            >
              {#if !asList}<Miniature outline={m.outline} />{/if}
              <div class="meta">
                <span class="name">{m.name}</span>
                {#if m.source === 'user'}<span class="own" title="Aus der eigenen Bibliothek">eigen · v{m.version}</span>{/if}
                <span class="desc">{m.description || m.kinds.map((k) => kindLabel[k]).join(', ')}</span>
              </div>
            </div>
          {:else}
            <p class="none">Kein Modul gefunden.</p>
          {/each}
        {:else}
          {#each templateHits as t, i (t.id)}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
            <div
              class="item"
              class:active={i === active}
              onpointermove={() => (active = i)}
              onclick={() => choose(i)}
              oncontextmenu={(e) => context(e, { template: t })}
            >
              {#if !asList}<Miniature outline={t.outline} />{/if}
              <div class="meta">
                <span class="name">{t.name}</span>
                {#if t.source === 'user'}<span class="own" title="Aus der eigenen Bibliothek">eigen</span>{/if}
                <span class="desc">{t.description}</span>
              </div>
            </div>
          {:else}
            <p class="none">Keine Vorlage gefunden.</p>
          {/each}
        {/if}
      </div>

      <footer>
        <span>
          {#if mode.kind === 'manage'}
            Rechtsklick löscht eigene Einträge.
          {:else}
            Eingabe {tab === 'module' ? 'fügt ein' : 'legt das Dokument an'} · Pfeiltasten wählen
          {/if}
        </span>
        <span class="spacer"></span>
        {#if library?.dir}
          <button class="link" title={library.dir} disabled={!onRevealDir} onclick={() => onRevealDir?.()}
            >Bibliotheksordner: {library.dir}</button
          >
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 30, 0.35);
    z-index: 95;
    display: grid;
    justify-items: center;
    align-items: start;
    padding: 8vh 16px 16px;
  }
  .picker {
    width: min(880px, 100%);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--line);
  }
  header input {
    flex: 1;
    font: inherit;
    font-size: 15px;
    color: var(--text);
    background: none;
    border: none;
    padding: 14px 16px;
    outline: none;
  }
  .toggle {
    display: flex;
    margin-right: 10px;
    border: 1px solid var(--line);
    border-radius: 7px;
    overflow: hidden;
  }
  .toggle button {
    font: inherit;
    width: 30px;
    height: 26px;
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .toggle button.on {
    background: var(--panel-2);
    color: var(--text);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    min-height: 44px;
  }
  .tabs {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 2px;
  }
  .tabs button {
    font: inherit;
    font-size: 13px;
    padding: 3px 12px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--panel);
    color: var(--text);
    border-color: var(--line);
  }
  .chips {
    display: flex;
    gap: 6px;
  }
  .chip {
    font: inherit;
    font-size: 12px;
    padding: 2px 10px;
    border-radius: 12px;
    border: 1px solid var(--line-strong);
    background: none;
    color: var(--text-2);
    cursor: pointer;
  }
  .chip.on {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  .spacer {
    flex: 1;
  }
  .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 8px;
    padding: 12px;
    overflow: auto;
    align-content: start;
  }
  .items.list {
    grid-template-columns: 1fr;
    gap: 0;
    padding: 4px;
  }
  .item {
    display: grid;
    gap: 7px;
    padding: 8px;
    border-radius: 9px;
    border: 1px solid transparent;
    cursor: default;
    min-width: 0;
  }
  .item.active {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 8px;
    min-width: 0;
  }
  .list .meta {
    flex-wrap: nowrap;
  }
  .name {
    font-weight: 600;
    font-size: 13px;
    white-space: nowrap;
  }
  .own {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent-strong);
    white-space: nowrap;
  }
  .desc {
    flex-basis: 100%;
    font-size: 12px;
    color: var(--muted);
    line-height: 1.35;
  }
  .list .desc {
    flex-basis: auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    grid-column: 1 / -1;
    text-align: center;
    color: var(--muted);
    padding: 24px;
    margin: 0;
  }
  footer {
    display: flex;
    gap: 12px;
    padding: 7px 14px;
    border-top: 1px solid var(--line);
    font-size: 11px;
    color: var(--muted);
  }
  .link {
    font: inherit;
    color: inherit;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link:hover:not(:disabled) {
    color: var(--text);
    text-decoration: underline;
  }
</style>
