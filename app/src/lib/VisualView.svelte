<script lang="ts">
  // Visueller Editor: Abschnitte sehen aus wie im Dokument und werden
  // direkt bearbeitet. Verschieben am Griff oder mit Strg+↑/↓, alle
  // weiteren Aktionen über Rechtsklick.
  import { tick } from 'svelte';
  import EditableText from './EditableText.svelte';
  import ElementView from './ElementView.svelte';
  import Inspector from './Inspector.svelte';
  import { editor } from './editor.svelte';
  import { openMenu, openMenuBelow, type MenuItem } from './menus.svelte';
  import { call, type Command, type Doc, type El, type ModulePart, type PartView } from './api';

  type Props = {
    doc: Doc;
    errorParts: string[];
    /** Modul, das gerade in der Werkstatt bearbeitet wird (`kennung@version`). */
    editing: string | null;
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
    onImage: (part: string, field: string, file: File) => void;
    onJump: (offset: number) => void;
    /** Modul aus der Bibliothek an dieser Stelle einfügen. */
    onPick: (index: number) => void;
    onEditModule: (p: ModulePart) => void;
    onPublish: (p: ModulePart) => void;
    onUpgrade: (p: ModulePart) => void;
    onToModule: (p: PartView) => void;
    /** Seitenbereich je Teil im angezeigten PDF. */
    pages: Record<string, { range: [number, number]; breaks: [number, number][] }> | null;
    /** Vom Anwender ausgelöstes Scrollen (für die Synchronisierung). */
    onScroll?: () => void;
  };
  let {
    doc,
    errorParts,
    editing,
    onCommand,
    onImage,
    onJump,
    onPick,
    onEditModule,
    onPublish,
    onUpgrade,
    onToModule,
    pages,
    onScroll,
  }: Props = $props();

  let root = $state<HTMLDivElement>();
  let selected = $state<string | null>(null);

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

  /** Neue Verwendung desselben Moduls wie Teil `p`, ob in der Bibliothek oder nicht. */
  async function insertSame(p: ModulePart, index: number) {
    const def = await call('module_def', { module: { id: p.module_id, version: p.version } });
    await onCommand({ op: 'insert_module', index, def, requires: [] });
    await focusPart(index);
  }

  async function duplicate(p: PartView, i: number) {
    await onCommand({ op: 'duplicate_part', part: p.id });
    await focusPart(i + 1);
  }

  function insertItems(index: number): MenuItem[] {
    const above = doc.parts[index - 1];
    return [
      { heading: 'Hier einfügen' },
      ...(above?.kind === 'module'
        ? [{ label: `Gleiches Modul: ${above.name}`, hint: 'leer', action: () => insertSame(above, index) }]
        : []),
      { label: 'Modul aus der Bibliothek …', hint: 'Strg+M', action: () => onPick(index) },
      { label: 'LaTeX-Abschnitt', action: () => insertLatex(index) },
    ];
  }

  /** Nach dem Einfügen von außen: neuen Abschnitt auswählen und sein erstes Feld markieren. */
  export async function focusPart(index: number) {
    await tick();
    const id = doc.parts[index]?.id;
    if (!id) return;
    selected = id;
    const el = partEl(id);
    const field = el?.querySelector<HTMLInputElement | HTMLTextAreaElement>('input, textarea');
    (field ?? el)?.focus();
    field?.select();
    el?.scrollIntoView({ block: 'nearest' });
  }

  /** Stelle für „Einfügen“ aus Menü und Tastatur: hinter dem gewählten Abschnitt, sonst am Ende. */
  export function insertIndex(): number {
    const i = doc.parts.findIndex((p) => p.id === selected);
    return i < 0 ? doc.parts.length : i + 1;
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
      ...(p.kind === 'module'
        ? [
            { label: 'Modul bearbeiten …', hint: 'Doppelklick', action: () => onEditModule(p) },
            { label: 'In Bibliothek speichern', disabled: !p.status.local, action: () => onPublish(p) },
            ...(p.status.newer
              ? [{ label: `Auf Version ${p.status.newer} aktualisieren`, action: () => onUpgrade(p) }]
              : []),
          ]
        : [{ label: 'In Modul umwandeln …', action: () => onToModule(p) }]),
      { separator: true },
      { label: 'Duplizieren', hint: 'mit Inhalt', action: () => duplicate(p, i) },
      ...(p.kind === 'module' ? [{ label: 'Gleiches Modul darunter', hint: 'leer', action: () => insertSame(p, i + 1) }] : []),
      { label: 'Modul darüber einfügen …', action: () => onPick(i) },
      { label: 'Modul darunter einfügen …', action: () => onPick(i + 1) },
      { label: 'LaTeX-Abschnitt darunter', action: () => insertLatex(i + 1) },
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

  // --- Einklappen: Module als einzeilige Zusammenfassung ---

  const collapseKey = () => `formavium.collapsed:${doc.path}`;
  function loadCollapsed(): string[] {
    try {
      return JSON.parse(localStorage.getItem(collapseKey()) ?? '[]');
    } catch {
      return [];
    }
  }
  let collapsed = $state<string[]>(loadCollapsed());
  function toggleCollapse(id: string) {
    collapsed = collapsed.includes(id) ? collapsed.filter((c) => c !== id) : [...collapsed, id];
    try {
      localStorage.setItem(collapseKey(), JSON.stringify(collapsed));
    } catch {
      /* nur für diese Sitzung */
    }
  }
  function summary(body: El[]): string {
    const out: string[] = [];
    const walk = (els: El[]) => {
      for (const el of els) {
        if (out.join(' · ').length > 160) return;
        if ((el.kind === 'heading' || el.kind === 'paragraph') && el.text) out.push(el.text);
        else if (el.kind === 'latex') out.push(...el.fields.filter((f) => f.kind !== 'image' && f.value).map((f) => f.value));
        else if (el.kind === 'table') out.push(`Tabelle ${el.rows.length} × ${el.columns.length}`);
        else if (el.kind === 'row' || el.kind === 'module') walk(el.children);
      }
    };
    walk(body);
    return out.join(' · ');
  }

  // --- Fokus: gewählte Stelle melden, Sprünge aus dem PDF ausführen ---

  function focusIn(e: FocusEvent) {
    const target = e.target as HTMLElement;
    // Tabellen melden ihre Zellen selbst.
    if (target.closest('.tv')) return;
    const li = target.closest<HTMLElement>('li.part');
    if (!li?.dataset.part) return;
    const field = target.closest<HTMLElement>('[data-field]')?.dataset.field;
    const item = target.closest<HTMLElement>('[data-item]')?.dataset.item;
    const list = target.closest<HTMLElement>('[data-list]')?.dataset.list;
    editor.focus = [
      item && list
        ? { part: li.dataset.part, path: list, row: item, column: 'text' }
        : field
          ? { part: li.dataset.part, path: field }
          : { part: li.dataset.part },
    ];
  }

  $effect(() => {
    const req = editor.request;
    if (!req || !root) return;
    const t = req.target;
    tick().then(() => {
      const li = partEl(t.part);
      if (!li) return;
      selected = t.part;
      if (collapsed.includes(t.part)) toggleCollapse(t.part);
      let el: HTMLElement | null = null;
      if (t.row && t.path) {
        // Listenpunkt; Tabellenzellen übernimmt die Tabelle selbst.
        el = li.querySelector<HTMLElement>(`[data-list="${CSS.escape(t.path)}"] [data-item="${CSS.escape(t.row)}"]`);
        if (!el) return;
      } else if (t.path) {
        const host = li.querySelector<HTMLElement>(`[data-field="${CSS.escape(t.path)}"]`);
        el = host?.querySelector<HTMLElement>('input, textarea, [tabindex="0"]') ?? host ?? null;
      }
      const target = el ?? li;
      target.focus({ preventScroll: true });
      target.scrollIntoView({ block: 'center', behavior: 'smooth' });
      target.closest<HTMLElement>('.frame')?.animate(
        [{ boxShadow: '0 0 0 4px var(--accent-soft)' }, { boxShadow: '0 0 0 0 transparent' }],
        { duration: 700 },
      );
    });
  });

  // --- Lage für das synchrone Scrollen ---

  /** Oberster sichtbarer Teil und wie weit er schon gescrollt ist (0 bis 1). */
  export function topPosition(): { part: string; fraction: number } | null {
    if (!root) return null;
    const top = root.getBoundingClientRect().top + 8;
    for (const li of root.querySelectorAll<HTMLElement>('li.part')) {
      const r = li.getBoundingClientRect();
      if (r.bottom > top) {
        return { part: li.dataset.part!, fraction: Math.min(1, Math.max(0, (top - r.top) / Math.max(1, r.height))) };
      }
    }
    return null;
  }

  export function scrollToPart(id: string, fraction: number) {
    const li = partEl(id);
    if (!li || !root) return;
    const offset = li.getBoundingClientRect().top - root.getBoundingClientRect().top + root.scrollTop;
    root.scrollTop = offset + fraction * li.offsetHeight - 8;
  }

  /** Seitenwechsel vor Teil `i` im angezeigten PDF. */
  function pageBreak(i: number): number | null {
    if (!pages || i === 0) return null;
    const here = pages[doc.parts[i].id]?.range;
    const before = pages[doc.parts[i - 1].id]?.range;
    return here && before && here[0] > before[1] ? here[0] : null;
  }
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

<div class="vwrap">
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="visual" bind:this={root} onkeydown={onKey} onfocusin={focusIn} onscroll={() => onScroll?.()}>
  {#if doc.parts.length === 0}
    <div class="blank">
      <p>Dieses Dokument hat noch keine Abschnitte.</p>
      <button class="ghost" onclick={(e) => openMenuBelow(e.currentTarget, insertItems(0))}>Abschnitt einfügen</button>
    </div>
  {:else}
    <ol class="parts">
      {@render gap(0, false)}
      {#each doc.parts as p, i (p.id)}
        {@const brk = pageBreak(i)}
        {#if brk}
          <li class="pagebreak" role="presentation"><span>Seite {brk}</span></li>
        {/if}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <li
          class="part"
          class:selected={selected === p.id}
          class:has-error={errorParts.includes(p.id)}
          class:editing={p.kind === 'module' && p.module === editing}
          class:collapsed={collapsed.includes(p.id)}
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
            {#if !collapsed.includes(p.id)}
              {#each pages?.[p.id]?.breaks ?? [] as [pg, at]}
                <div class="inbreak" style:top="{8 + at * 88}%" aria-hidden="true"><span>Seite {pg}</span></div>
              {/each}
            {/if}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <header ondblclick={() => p.kind === 'module' && onEditModule(p)}>
              <button
                class="chev"
                tabindex="-1"
                title={collapsed.includes(p.id) ? 'Aufklappen' : 'Einklappen'}
                aria-label={collapsed.includes(p.id) ? 'Abschnitt aufklappen' : 'Abschnitt einklappen'}
                aria-expanded={!collapsed.includes(p.id)}
                onclick={() => toggleCollapse(p.id)}>›</button
              >
              {#if p.kind === 'module'}
                <span class="badge">Modul</span>
                <span class="name" title={p.description || undefined}>{p.name}</span>
                {#if p.status.newer}
                  <button class="state update" title="In der Bibliothek gibt es eine neuere Version" onclick={() => onUpgrade(p)}
                    >Version {p.status.newer} verfügbar</button
                  >
                {:else if p.status.local}
                  <button
                    class="state"
                    title="Dieser Stand steht nur im Dokument. Klick öffnet die Modul-Werkstatt."
                    onclick={() => onEditModule(p)}>nur im Dokument</button
                  >
                {/if}
                {#if p.required}
                  <span class="req" title="Von der Vorlage als erforderlich gekennzeichnet">erforderlich</span>
                {/if}
              {:else}
                <span class="badge latex">LaTeX</span>
                <span class="name">Freier Abschnitt</span>
              {/if}
              {#if collapsed.includes(p.id)}
                <span class="summary">{p.kind === 'module' ? summary(p.body) : stripNewline(p.source).split('\n')[0]}</span>
              {/if}
              <span class="spacer"></span>
              {#if pages?.[p.id] && pages[p.id].range[1] > pages[p.id].range[0]}
                <span class="pages" title="Dieser Abschnitt geht im PDF über mehrere Seiten">S. {pages[p.id].range[0]}–{pages[p.id].range[1]}</span>
              {/if}
            </header>
            {#each notices(p) as n}
              <div class="notice">{n.message}</div>
            {/each}
            {#if collapsed.includes(p.id)}
              <!-- eingeklappt -->
            {:else if p.kind === 'module'}
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
{#if editor.tableMode === 'inspector'}<Inspector />{/if}
</div>

<style>
  .vwrap {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .visual {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 6px 18px 40px 6px;
    background: var(--panel-2);
    scrollbar-gutter: stable;
  }
  .inbreak {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px dashed var(--line-strong);
    pointer-events: none;
    z-index: 1;
  }
  .inbreak span {
    position: absolute;
    right: 8px;
    top: -8px;
    font: 9.5px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    background: var(--panel);
    padding: 0 4px;
  }
  .pagebreak {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 2px 0 4px 22px;
    font: 10.5px var(--ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .pagebreak::before,
  .pagebreak::after {
    content: '';
    flex: 1;
    border-top: 1px dashed var(--line-strong);
  }
  .chev {
    width: 16px;
    height: 16px;
    margin-left: -4px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: none;
    color: var(--muted);
    font: 14px/1 var(--ui);
    transform: rotate(90deg);
    transition: transform 0.12s;
    cursor: pointer;
  }
  .chev:hover {
    background: var(--line);
    color: var(--text);
  }
  .part.collapsed .chev {
    transform: none;
  }
  .part.collapsed .frame {
    padding-bottom: 6px;
  }
  .part.collapsed header {
    margin-bottom: 0;
  }
  .summary {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
    font-size: 12px;
  }
  .spacer {
    flex: 1;
  }
  .pages {
    font-size: 10.5px;
    color: var(--muted);
    white-space: nowrap;
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
    cursor: var(--cur-grab);
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
    cursor: var(--cur-grabbing);
  }

  .frame {
    position: relative;
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
  .state {
    font: inherit;
    color: var(--muted);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .state:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .state.update {
    color: var(--accent-strong);
    font-weight: 600;
  }
  .part.editing .frame {
    border-color: var(--accent-strong);
    border-style: dashed;
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
