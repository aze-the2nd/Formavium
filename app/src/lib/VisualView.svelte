<script lang="ts">
  // Visueller Editor: Abschnitte sehen aus wie im Dokument und werden
  // direkt bearbeitet. Verschieben am Griff oder mit Strg+↑/↓, alle
  // weiteren Aktionen über Rechtsklick.
  import { tick } from 'svelte';
  import EditableText from './EditableText.svelte';
  import ElementView from './ElementView.svelte';
  import { openMenu, openMenuBelow, type MenuItem } from './menus.svelte';
  import type { Command, ContentKind, Doc, PartView } from './api';

  type Props = {
    doc: Doc;
    errorParts: string[];
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
    onImage: (part: string, field: string, file: File) => void;
    onJump: (offset: number) => void;
  };
  let { doc, errorParts, onCommand, onImage, onJump }: Props = $props();

  let root = $state<HTMLDivElement>();
  let selected = $state<string | null>(null);

  const kindLabel: Record<ContentKind, string> = { text: 'Text', table: 'Tabelle', image: 'Bild', latex: 'LaTeX' };
  const partEl = (id: string) => root?.querySelector<HTMLElement>(`li.part[data-part="${CSS.escape(id)}"]`);
  const notices = (p: PartView) => doc.notices.filter((n) => n.part === p.id);

  // --- Abschnitte einfügen, verschieben, entfernen ---

  async function insert(index: number, content: unknown) {
    await onCommand({ op: 'insert_part', index, content });
    await tick();
    const id = doc.parts[index]?.id;
    if (!id) return;
    selected = id;
    const el = partEl(id);
    // Erstes Feld markieren: Tippen ersetzt den Vorgabetext.
    const field = el?.querySelector<HTMLInputElement | HTMLTextAreaElement>('input, textarea');
    (field ?? el)?.focus();
    field?.select();
    el?.scrollIntoView({ block: 'nearest' });
  }

  const insertLatex = (index: number) => insert(index, { kind: 'latex', source: 'Neuer Abschnitt.\n' });

  const insertModule = (m: Doc['modules'][number], index: number) =>
    insert(index, {
      kind: 'module',
      module: { id: m.id, version: m.version },
      values: {},
      column_order: {},
      required: false,
    });

  function insertItems(index: number): MenuItem[] {
    return [
      { heading: 'Hier einfügen' },
      { label: 'LaTeX-Abschnitt', action: () => insertLatex(index) },
      ...doc.modules.map((m) => ({
        label: `Modul „${m.name}“`,
        hint: m.kinds.map((k) => kindLabel[k]).join(', '),
        action: () => insertModule(m, index),
      })),
    ];
  }

  async function movePart(id: string, delta: number) {
    const from = doc.parts.findIndex((p) => p.id === id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= doc.parts.length) return;
    const active = document.activeElement;
    await onCommand({ op: 'move_part', part: id, to });
    await tick();
    // Beim Umsortieren kann der Fokus verloren gehen.
    if (active instanceof HTMLElement && active.isConnected) active.focus({ preventScroll: true });
    partEl(id)?.scrollIntoView({ block: 'nearest' });
  }

  async function removePart(id: string) {
    const i = doc.parts.findIndex((p) => p.id === id);
    const next = (doc.parts[i + 1] ?? doc.parts[i - 1])?.id;
    await onCommand({ op: 'remove_part', part: id });
    await tick();
    selected = next ?? null;
    if (next) partEl(next)?.focus();
  }

  function partItems(p: PartView, i: number): MenuItem[] {
    return [
      { heading: p.kind === 'module' ? `Modul „${p.name}“` : 'Freier LaTeX-Abschnitt' },
      { label: 'Im Quelltext zeigen', action: () => onJump(p.offset) },
      { label: 'Nach oben', hint: 'Strg+↑', disabled: i === 0, action: () => movePart(p.id, -1) },
      {
        label: 'Nach unten',
        hint: 'Strg+↓',
        disabled: i === doc.parts.length - 1,
        action: () => movePart(p.id, 1),
      },
      { separator: true },
      { label: 'LaTeX-Abschnitt darüber', action: () => insertLatex(i) },
      { label: 'LaTeX-Abschnitt darunter', action: () => insertLatex(i + 1) },
      ...doc.modules.map((m) => ({ label: `Modul „${m.name}“ darunter`, action: () => insertModule(m, i + 1) })),
      { separator: true },
      { label: 'Abschnitt entfernen', hint: 'Entf', danger: true, action: () => removePart(p.id) },
    ];
  }

  function partContext(e: MouseEvent, p: PartView, i: number) {
    // In Textfeldern bleibt das Menü des Systems (Kopieren, Einfügen).
    if ((e.target as HTMLElement).closest('input, textarea')) return;
    e.preventDefault();
    selected = p.id;
    openMenu({ x: e.clientX, y: e.clientY }, partItems(p, i));
  }

  // --- Tastatur ---

  function onKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    const li = target.closest<HTMLElement>('li.part');
    const id = li?.dataset.part ?? selected;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.altKey && !e.shiftKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      if (!id) return;
      e.preventDefault();
      movePart(id, e.key === 'ArrowUp' ? -1 : 1);
      return;
    }
    if (!li || !id) return;
    if (target !== li) {
      // Escape verlässt das Feld und fokussiert den Abschnitt.
      if (e.key === 'Escape') {
        e.preventDefault();
        li.focus();
      }
      return;
    }
    const i = doc.parts.findIndex((p) => p.id === id);
    let handled = true;
    if (e.key === 'ArrowUp' && i > 0) partEl(doc.parts[i - 1].id)?.focus();
    else if (e.key === 'ArrowDown' && i < doc.parts.length - 1) partEl(doc.parts[i + 1].id)?.focus();
    else if (e.key === 'Enter') li.querySelector<HTMLElement>('input, textarea, [tabindex="0"]')?.focus();
    else if (e.key === 'Delete' || e.key === 'Backspace') removePart(id);
    else handled = false;
    if (handled) e.preventDefault();
  }

  // --- Abschnitte am Griff ziehen ---

  let drag = $state<{ id: string; from: number; over: number | null; active: boolean } | null>(null);
  let dragStartY = 0;

  function handleDown(e: PointerEvent, id: string, from: number) {
    if (e.button !== 0) return;
    drag = { id, from, over: null, active: false };
    dragStartY = e.clientY;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function handleMove(e: PointerEvent) {
    if (!drag || !root) return;
    if (!drag.active && Math.abs(e.clientY - dragStartY) < 5) return;
    drag.active = true;
    const items = Array.from(root.querySelectorAll<HTMLElement>('li.part'));
    let over = items.length;
    for (let k = 0; k < items.length; k++) {
      const r = items[k].getBoundingClientRect();
      if (e.clientY < r.top + r.height / 2) {
        over = k;
        break;
      }
    }
    drag.over = over;
    const r = root.getBoundingClientRect();
    if (e.clientY < r.top + 48) root.scrollBy(0, -16);
    else if (e.clientY > r.bottom - 48) root.scrollBy(0, 16);
  }

  function handleUp(e: PointerEvent, p: PartView, i: number) {
    const d = drag;
    drag = null;
    if (!d) return;
    if (!d.active) {
      // Klick ohne Ziehen: Aktionen des Abschnitts.
      selected = p.id;
      openMenuBelow(e.currentTarget as HTMLElement, partItems(p, i));
      return;
    }
    if (d.over === null) return;
    const to = d.over > d.from ? d.over - 1 : d.over;
    if (to !== d.from) onCommand({ op: 'move_part', part: d.id, to });
  }

  /** Einfügemarke beim Ziehen: vor Abschnitt `i` oder nach dem letzten. */
  const dropBefore = (i: number) => !!drag?.active && drag.over === i && i !== drag.from && i !== drag.from + 1;
  const dropAfter = (i: number) =>
    !!drag?.active && drag.over === doc.parts.length && i === doc.parts.length - 1 && i !== drag.from;

  const stripNewline = (s: string) => s.replace(/\r?\n$/, '');
</script>

{#snippet gap(index: number, end: boolean)}
  <li class="gap" class:end role="presentation">
    <button
      class="add"
      tabindex="-1"
      title="Abschnitt hier einfügen"
      aria-label="Abschnitt hier einfügen"
      onclick={(e) => openMenuBelow(e.currentTarget, insertItems(index))}>+</button
    >
  </li>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="visual" bind:this={root} onkeydown={onKey}>
  {#if doc.parts.length === 0}
    <div class="blank">
      <p>Dieses Dokument hat noch keine Abschnitte.</p>
      <button class="ghost" onclick={(e) => openMenuBelow(e.currentTarget, insertItems(0))}>Abschnitt einfügen</button>
    </div>
  {:else}
    <ol class="parts">
      {@render gap(0, false)}
      {#each doc.parts as p, i (p.id)}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <li
          class="part"
          class:selected={selected === p.id}
          class:has-error={errorParts.includes(p.id)}
          class:dragging={drag?.active && drag.id === p.id}
          class:drop-before={dropBefore(i)}
          class:drop-after={dropAfter(i)}
          data-part={p.id}
          tabindex="-1"
          aria-label={p.kind === 'module' ? `Modul ${p.name}` : 'Freier LaTeX-Abschnitt'}
          onpointerdown={() => (selected = p.id)}
          onfocusin={() => (selected = p.id)}
          oncontextmenu={(e) => partContext(e, p, i)}
        >
          <button
            class="handle"
            tabindex="-1"
            title="Ziehen verschiebt den Abschnitt (Strg+↑/↓). Klick zeigt die Aktionen."
            aria-label="Abschnitt verschieben oder Aktionen öffnen"
            onpointerdown={(e) => handleDown(e, p.id, i)}
            onpointermove={handleMove}
            onpointerup={(e) => handleUp(e, p, i)}
            onpointercancel={() => (drag = null)}
          >
            <svg viewBox="0 0 8 14" width="8" height="14" aria-hidden="true"
              ><g fill="currentColor"
                ><circle cx="2" cy="2" r="1.2" /><circle cx="6" cy="2" r="1.2" /><circle cx="2" cy="7" r="1.2" /><circle
                  cx="6"
                  cy="7"
                  r="1.2"
                /><circle cx="2" cy="12" r="1.2" /><circle cx="6" cy="12" r="1.2" /></g
              ></svg
            >
          </button>
          <div class="frame">
            <header>
              {#if p.kind === 'module'}
                <span class="badge">Modul</span>
                <span class="name">{p.name}</span>
                {#if p.required}
                  <span class="req" title="Von der Vorlage als erforderlich gekennzeichnet">erforderlich</span>
                {/if}
              {:else}
                <span class="badge latex">LaTeX</span>
                <span class="name">Freier Abschnitt</span>
              {/if}
            </header>
            {#each notices(p) as n}
              <div class="notice">{n.message}</div>
            {/each}
            {#if p.kind === 'module'}
              <div class="body">
                {#each p.body as el}
                  <ElementView {el} part={p.id} {onCommand} {onImage} />
                {/each}
              </div>
            {:else}
              <div class="source">
                <EditableText
                  multiline
                  value={stripNewline(p.source)}
                  label="LaTeX-Quelltext des Abschnitts"
                  placeholder="LaTeX"
                  onCommit={(v: string) => onCommand({ op: 'set_latex', part: p.id, source: `${v}\n` })}
                />
              </div>
            {/if}
          </div>
        </li>
        {@render gap(i + 1, i === doc.parts.length - 1)}
      {/each}
    </ol>
  {/if}
</div>

<style>
  .visual {
    height: 100%;
    overflow: auto;
    padding: 6px 18px 40px 6px;
    background: var(--panel-2);
  }
  .parts {
    list-style: none;
    margin: 0 auto;
    padding: 0;
    max-width: 860px;
  }
  .part {
    position: relative;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    outline: none;
  }
  .part.dragging {
    opacity: 0.45;
  }
  .part.drop-before::before,
  .part.drop-after::after {
    content: '';
    position: absolute;
    left: 22px;
    right: 0;
    height: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .part.drop-before::before {
    top: -8px;
  }
  .part.drop-after::after {
    bottom: -8px;
  }

  /* Griff: an jedem Abschnitt dauerhaft sichtbar. */
  .handle {
    align-self: start;
    margin-top: 8px;
    width: 18px;
    height: 26px;
    display: grid;
    place-items: center;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--line-strong);
    cursor: grab;
    touch-action: none;
  }
  .handle:hover,
  .part.selected .handle {
    color: var(--muted);
  }
  .handle:hover {
    background: var(--line);
  }
  .handle:active {
    cursor: grabbing;
  }

  .frame {
    min-width: 0;
    container: part / inline-size;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 8px 30px 12px;
    transition:
      border-color 0.1s,
      box-shadow 0.1s;
  }
  .part.selected .frame {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .part.has-error .frame {
    border-color: var(--danger);
  }
  header {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 0 -18px 6px;
    font-size: 11px;
    color: var(--muted);
    user-select: none;
  }
  .badge {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    background: var(--accent-soft);
    color: var(--accent-strong);
    padding: 1px 6px;
    border-radius: 4px;
    font-weight: 600;
  }
  .badge.latex {
    background: var(--panel-2);
    color: var(--muted);
  }
  .req {
    color: var(--warn);
  }
  .notice {
    font-size: 12px;
    color: var(--warn);
    background: var(--warn-soft);
    border-radius: 6px;
    padding: 4px 8px;
    margin-bottom: 8px;
  }
  /* Modulinhalt in Dokumentschrift, Quelltext in Festbreitenschrift. */
  .body {
    font-family: var(--serif);
    font-size: 15px;
  }
  .source {
    --et-font: var(--mono);
    --et-size: 12.5px;
    --et-line: 1.55;
    margin: 0 -6px;
    color: var(--text-2);
  }

  /* Einfügestelle zwischen Abschnitten: erscheint beim Überfahren. */
  .gap {
    position: relative;
    height: 14px;
    margin-left: 22px;
  }
  .gap::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 6px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
    opacity: 0;
    transition: opacity 0.1s 0.05s;
  }
  .add {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    padding: 0 0 1px;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    color: white;
    font: 600 14px/1 var(--ui);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s 0.05s;
  }
  .gap:hover::before,
  .gap:hover .add,
  .add:focus-visible {
    opacity: 1;
  }
  .gap.end {
    height: 30px;
  }
  .gap.end::before {
    top: 14px;
  }
  .gap.end .add {
    opacity: 0.4;
  }
  .gap.end:hover .add {
    opacity: 1;
  }
  .blank {
    max-width: 420px;
    margin: 60px auto;
    text-align: center;
    color: var(--muted);
  }
</style>
