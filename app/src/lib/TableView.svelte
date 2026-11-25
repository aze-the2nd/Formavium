<script lang="ts">
  // Tabelle mit direkter Bearbeitung. Einfügen über Plus-Zeichen, die beim
  // Überfahren außerhalb der Tabelle erscheinen, alles Weitere über
  // Rechtsklick und Tastenkürzel.
  import { tick } from 'svelte';
  import EditableText from './EditableText.svelte';
  import { clipboardItems, openMenu, type MenuItem } from './menus.svelte';
  import type { Col, Command, Doc, El } from './api';

  type Table = Extract<El, { kind: 'table' }>;
  type Props = {
    t: Table;
    part: string;
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
  };
  let { t, part, onCommand }: Props = $props();

  let wrap = $state<HTMLDivElement>();
  let table = $state<HTMLTableElement>();
  /** Spalte, deren Kopf gerade umbenannt wird. */
  let renaming = $state<string | null>(null);

  // --- Fokus über Kennungen, damit er Umordnungen übersteht ---

  function focusCell(rowId: string | undefined, colId: string | undefined) {
    if (!rowId || !colId) return;
    const sel = `[data-rid="${CSS.escape(rowId)}"][data-cid="${CSS.escape(colId)}"]`;
    table?.querySelector<HTMLElement>(sel)?.focus();
  }

  function focusHeader(colId: string | undefined) {
    if (colId) table?.querySelector<HTMLElement>(`th[data-cid="${CSS.escape(colId)}"]`)?.focus();
  }

  // --- Befehle ---

  async function run(cmd: Command) {
    colGap = rowGap = null;
    await onCommand(cmd);
    await tick();
  }

  async function insertRow(index: number, col = 0) {
    const colId = t.columns[col]?.id;
    await run({ op: 'insert_row', part, path: t.rows_path, index });
    focusCell(t.rows[index]?.id, colId);
  }

  async function removeRow(r: number, col = 0) {
    const row = t.rows[r];
    if (!row) return;
    const next = (t.rows[r + 1] ?? t.rows[r - 1])?.id;
    const colId = t.columns[col]?.id;
    await run({ op: 'remove_row', part, path: t.rows_path, row: row.id });
    focusCell(next, colId);
  }

  async function moveRow(r: number, to: number, col = 0) {
    const row = t.rows[r];
    if (!row || to < 0 || to >= t.rows.length) return;
    const colId = t.columns[col]?.id;
    await run({ op: 'move_row', part, path: t.rows_path, row: row.id, to });
    focusCell(row.id, colId);
  }

  async function insertColumn(index: number) {
    const before = new Set(t.columns.map((c) => c.id));
    await run({ op: 'insert_column', part, table: t.path, index, header: 'Spalte' });
    // Neue Spalte gleich benennen.
    renaming = t.columns.find((c) => !before.has(c.id))?.id ?? null;
  }

  async function removeColumn(c: number) {
    const col = t.columns[c];
    if (!col || t.columns.length < 2) return;
    const next = (t.columns[c + 1] ?? t.columns[c - 1])?.id;
    await run({ op: 'remove_column', part, table: t.path, column: col.id });
    focusHeader(next);
  }

  /** Spalte verschieben. `row` gesetzt: Fokus bleibt in dieser Zelle. */
  async function moveColumn(from: number, to: number, row?: number) {
    const col = t.columns[from];
    if (!col || from === to || to < 0 || to >= t.columns.length) return;
    const rowId = row === undefined ? undefined : t.rows[row]?.id;
    await run({ op: 'move_column', part, table: t.path, from, to });
    if (rowId) focusCell(rowId, col.id);
    else focusHeader(col.id);
  }

  const setCell = (rowId: string, colId: string, value: string) =>
    onCommand({ op: 'set_cell', part, path: t.rows_path, row: rowId, column: colId, value });

  const setHeader = (colId: string, header: string) =>
    onCommand({ op: 'set_column_header', part, table: t.path, column: colId, header });

  // --- Tastatur ---

  function cellKey(e: KeyboardEvent, r: number, c: number) {
    const mod = e.ctrlKey || e.metaKey;
    const last = t.rows.length - 1;
    const go = (row: number) => focusCell(t.rows[row]?.id, t.columns[c]?.id);
    let handled = true;
    if (e.key === 'Enter' && mod) insertRow(e.shiftKey ? r : r + 1, c);
    else if (e.key === 'Enter' && e.shiftKey) go(Math.max(0, r - 1));
    else if (e.key === 'Enter') r < last ? go(r + 1) : insertRow(r + 1, c);
    else if (e.key === 'Backspace' && mod && e.shiftKey) removeRow(r, c);
    else if (e.altKey && e.key === 'ArrowUp') moveRow(r, r - 1, c);
    else if (e.altKey && e.key === 'ArrowDown') moveRow(r, r + 1, c);
    else if (e.altKey && e.key === 'ArrowLeft') moveColumn(c, c - 1, r);
    else if (e.altKey && e.key === 'ArrowRight') moveColumn(c, c + 1, r);
    else if (!mod && e.key === 'ArrowUp' && r > 0) go(r - 1);
    else if (!mod && e.key === 'ArrowDown' && r < last) go(r + 1);
    else handled = false;
    if (handled) e.preventDefault();
  }

  function headerKey(e: KeyboardEvent, c: number) {
    if (renaming) return;
    let handled = true;
    if (e.altKey && e.key === 'ArrowLeft') moveColumn(c, c - 1);
    else if (e.altKey && e.key === 'ArrowRight') moveColumn(c, c + 1);
    else if (e.key === 'F2' || e.key === 'Enter') renaming = t.columns[c].id;
    else if (e.key === 'ArrowDown') focusCell(t.rows[0]?.id, t.columns[c].id);
    else handled = false;
    if (handled) e.preventDefault();
  }

  // --- Kontextmenüs ---

  function columnItems(c: number): MenuItem[] {
    const id = t.columns[c].id;
    return [
      { label: 'Spalte links einfügen', action: () => insertColumn(c) },
      { label: 'Spalte rechts einfügen', action: () => insertColumn(c + 1) },
      { label: 'Spalte nach links', hint: 'Alt+←', disabled: c === 0, action: () => moveColumn(c, c - 1) },
      {
        label: 'Spalte nach rechts',
        hint: 'Alt+→',
        disabled: c === t.columns.length - 1,
        action: () => moveColumn(c, c + 1),
      },
      { label: 'Spalte umbenennen', hint: 'F2', action: () => (renaming = id) },
      { label: 'Spalte löschen', danger: true, disabled: t.columns.length < 2, action: () => removeColumn(c) },
    ];
  }

  function cellMenu(e: MouseEvent, r: number, c: number) {
    e.preventDefault();
    e.stopPropagation();
    const field = (e.target as HTMLElement).closest('input');
    openMenu({ x: e.clientX, y: e.clientY }, [
      ...(field ? clipboardItems(field) : []),
      { label: 'Zeile darüber einfügen', hint: 'Strg+Umschalt+Eingabe', action: () => insertRow(r, c) },
      { label: 'Zeile darunter einfügen', hint: 'Strg+Eingabe', action: () => insertRow(r + 1, c) },
      { label: 'Zeile nach oben', hint: 'Alt+↑', disabled: r === 0, action: () => moveRow(r, r - 1, c) },
      {
        label: 'Zeile nach unten',
        hint: 'Alt+↓',
        disabled: r === t.rows.length - 1,
        action: () => moveRow(r, r + 1, c),
      },
      { label: 'Zeile löschen', hint: 'Strg+Umschalt+Rück', danger: true, action: () => removeRow(r, c) },
      { separator: true },
      ...columnItems(c),
    ]);
  }

  function headerMenu(e: MouseEvent, c: number) {
    e.preventDefault();
    e.stopPropagation();
    openMenu({ x: e.clientX, y: e.clientY }, columnItems(c));
  }

  // --- Spalten am Kopf ziehen (Zeiger-Ereignisse, siehe README) ---

  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);
  let dragging = $state(false);
  let dragStartX = 0;

  function headerDown(e: PointerEvent, i: number) {
    if (e.button !== 0 || renaming) return;
    dragFrom = i;
    dragOver = i;
    dragStartX = e.clientX;
    dragging = false;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function headerMove(e: PointerEvent) {
    if (dragFrom === null) return;
    if (!dragging && Math.abs(e.clientX - dragStartX) < 5) return;
    dragging = true;
    colGap = rowGap = null;
    const th = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('th[data-col]');
    if (th && table?.contains(th)) dragOver = Number(th.dataset.col);
  }

  function headerUp() {
    const [from, to, moved] = [dragFrom, dragOver, dragging];
    dragFrom = dragOver = null;
    dragging = false;
    if (moved && from !== null && to !== null) moveColumn(from, to);
  }

  // --- Einfügestellen beim Überfahren ---

  type Gap = { index: number; pos: number };
  let colGap = $state<Gap | null>(null);
  let rowGap = $state<Gap | null>(null);
  /** Lage der Tabelle in ihrem Rahmen. */
  let box = $state({ top: 0, left: 0, width: 0, height: 0 });

  function hover(e: PointerEvent) {
    if (!wrap || !table?.tHead || dragging) return;
    if ((e.target as HTMLElement).closest('.plus')) return;
    const w = wrap.getBoundingClientRect();
    const tb = table.getBoundingClientRect();
    box = { top: tb.top - w.top, left: tb.left - w.left, width: tb.width, height: tb.height };
    const { clientX: x, clientY: y } = e;

    // Spaltengrenze in der Nähe: Rand hervorheben, Plus über der Tabelle.
    let col: Gap | null = null;
    if (y <= tb.bottom) {
      const heads = Array.from(table.tHead.rows[0].cells).map((c) => c.getBoundingClientRect());
      const edges = [heads[0].left, ...heads.map((h) => h.right)];
      let best = 7;
      for (let index = 0; index < edges.length; index++) {
        const d = Math.abs(x - edges[index]);
        if (d < best) {
          best = d;
          col = { index, pos: edges[index] - w.left };
        }
      }
    }

    let row: Gap | null = null;
    if (!col) {
      const rows = Array.from(table.tBodies[0].rows)
        .filter((tr) => !tr.classList.contains('empty'))
        .map((tr) => tr.getBoundingClientRect());
      const head = table.tHead.getBoundingClientRect();
      if (x < tb.left) {
        // Linker Rand: nächstgelegene Zeilengrenze.
        const edges = [head.bottom, ...rows.map((r) => r.bottom)];
        let best = Infinity;
        for (let index = 0; index < edges.length; index++) {
          const d = Math.abs(y - edges[index]);
          if (d < best) {
            best = d;
            row = { index, pos: edges[index] - w.top };
          }
        }
      } else if (y >= (rows.at(-1)?.top ?? head.top)) {
        // Unterste Zeile oder darunter: neue Zeile am Ende.
        row = { index: rows.length, pos: tb.bottom - w.top };
      }
    }
    colGap = col;
    rowGap = row;
  }

  function leave() {
    colGap = rowGap = null;
  }

  function headerLabel(c: Col) {
    return c.unit && c.unit_placement === 'header' ? ` [${c.unit}]` : '';
  }
</script>

<div
  class="tw"
  role="group"
  aria-label={t.caption ?? 'Tabelle'}
  bind:this={wrap}
  onpointermove={hover}
  onpointerleave={leave}
>
  {#if t.caption}<div class="caption">{t.caption}</div>{/if}
  <table bind:this={table}>
    <thead>
      <tr>
        {#each t.columns as c, i (c.id)}
          <th
            data-col={i}
            data-cid={c.id}
            tabindex="0"
            class:over={dragging && dragOver === i && dragFrom !== i}
            class:dragging={dragging && dragFrom === i}
            style:--et-align={c.align}
            style:text-align={c.align}
            aria-label="Spalte {c.header}. Alt+Pfeiltaste verschiebt, F2 benennt um."
            onpointerdown={(e) => headerDown(e, i)}
            onpointermove={headerMove}
            onpointerup={headerUp}
            onpointercancel={headerUp}
            ondblclick={() => (renaming = c.id)}
            onkeydown={(e) => headerKey(e, i)}
            oncontextmenu={(e) => headerMenu(e, i)}
          >
            {#if renaming === c.id}
              <EditableText
                value={c.header}
                label="Spaltenkopf"
                autofocus
                onCommit={(v: string) => setHeader(c.id, v)}
                onBlur={() => (renaming = null)}
                onKey={(e: KeyboardEvent) => {
                  e.stopPropagation();
                  if (e.key === 'Enter' || e.key === 'Escape') {
                    e.preventDefault();
                    renaming = null;
                    tick().then(() => focusHeader(c.id));
                  }
                }}
                onpointerdown={(e: PointerEvent) => e.stopPropagation()}
              />
            {:else}
              <span class="grip" aria-hidden="true">
                <svg viewBox="0 0 8 12" width="8" height="12"
                  ><g fill="currentColor"
                    ><circle cx="2" cy="2" r="1.1" /><circle cx="6" cy="2" r="1.1" /><circle cx="2" cy="6" r="1.1" /><circle
                      cx="6"
                      cy="6"
                      r="1.1"
                    /><circle cx="2" cy="10" r="1.1" /><circle cx="6" cy="10" r="1.1" /></g
                  ></svg
                >
              </span>{c.header}<span class="unit">{headerLabel(c)}</span>
            {/if}
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each t.rows as row, r (row.id)}
        <tr>
          {#each t.columns as c, ci (c.id)}
            <td style:--et-align={c.align} oncontextmenu={(e) => cellMenu(e, r, ci)}>
              <span class="cell">
                <EditableText
                  value={row.cells[ci] ?? ''}
                  label="{c.header}, Zeile {r + 1}"
                  data-rid={row.id}
                  data-cid={c.id}
                  size={1}
                  onCommit={(v: string) => setCell(row.id, c.id, v)}
                  onKey={(e: KeyboardEvent) => cellKey(e, r, ci)}
                />
                {#if c.unit && c.unit_placement === 'value' && (row.cells[ci] ?? '').trim()}
                  <span class="unit">{c.unit}</span>
                {/if}
              </span>
            </td>
          {/each}
        </tr>
      {:else}
        <tr class="empty">
          <td colspan={t.columns.length}>
            <button onclick={() => insertRow(0)}>Noch keine Zeilen – klicken, um die erste anzulegen</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  {#if colGap}
    {#key colGap.index}
      <div class="guide v" style:left="{colGap.pos}px" style:top="{box.top}px" style:height="{box.height}px"></div>
      <button
        class="plus"
        tabindex="-1"
        title="Spalte hier einfügen"
        aria-label="Spalte hier einfügen"
        style:left="{colGap.pos}px"
        style:top="{box.top - 12}px"
        onclick={() => insertColumn(colGap!.index)}>+</button
      >
    {/key}
  {/if}
  {#if rowGap}
    {#key rowGap.index}
      {#if rowGap.index === t.rows.length}
        <button
          class="plus bar"
          tabindex="-1"
          title="Zeile anfügen"
          aria-label="Zeile anfügen"
          style:left="{box.left}px"
          style:top="{rowGap.pos + 4}px"
          style:width="{box.width}px"
          onclick={() => insertRow(t.rows.length)}>+</button
        >
      {:else}
        <div class="guide h" style:top="{rowGap.pos}px" style:left="{box.left}px" style:width="{box.width}px"></div>
        <button
          class="plus"
          tabindex="-1"
          title="Zeile hier einfügen"
          aria-label="Zeile hier einfügen"
          style:left="{box.left - 12}px"
          style:top="{rowGap.pos}px"
          onclick={() => insertRow(rowGap!.index)}>+</button
        >
      {/if}
    {/key}
  {/if}
</div>

<style>
  /* Rand ringsum: Dort erscheinen die Plus-Zeichen, außerhalb der Zellen. */
  .tw {
    position: relative;
    padding: 22px 6px 24px 24px;
    margin: 0 -6px 0 -24px;
    font-family: var(--ui);
    /* Breiter als der Platz (viele Spalten neben einem Bild): waagerecht rollen. */
    overflow-x: auto;
    scrollbar-width: thin;
  }
  .caption {
    font-weight: 600;
    margin-bottom: 6px;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-size: 13px;
    border-top: 2px solid var(--text);
    border-bottom: 2px solid var(--text);
  }
  th {
    position: relative;
    font-weight: 600;
    padding: 6px 8px;
    border-bottom: 1px solid var(--text);
    cursor: grab;
    user-select: none;
    white-space: nowrap;
    outline: none;
  }
  th:focus-visible {
    box-shadow: inset 0 0 0 2px var(--accent);
    border-radius: 4px;
  }
  th.dragging {
    opacity: 0.4;
  }
  th.over {
    box-shadow: inset 3px 0 0 var(--accent);
    background: var(--accent-soft);
  }
  /* Griff nur beim Überfahren oder mit Fokus. */
  .grip {
    position: absolute;
    left: -2px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--muted);
    opacity: 0;
    transition: opacity 0.1s;
    line-height: 0;
  }
  th:hover .grip,
  th:focus-visible .grip {
    opacity: 1;
  }
  /* Spaltenbreite folgt dem Inhalt, wie im gesetzten Dokument. */
  td {
    padding: 1px 3px;
    --et-sizing: content;
  }
  tbody tr:hover td {
    background: var(--field-hover);
  }
  .cell {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .unit {
    color: var(--muted);
    font-weight: 400;
    white-space: nowrap;
  }
  .cell .unit {
    font-size: 12px;
    padding-right: 5px;
  }
  .empty td {
    padding: 0;
  }
  .empty button {
    width: 100%;
    font: inherit;
    font-size: 12px;
    color: var(--muted);
    background: none;
    border: none;
    padding: 10px;
    cursor: pointer;
  }
  .empty button:hover {
    color: var(--accent-strong);
  }

  .guide {
    position: absolute;
    background: var(--accent);
    pointer-events: none;
    animation: appear 0.12s 0.08s both;
  }
  .guide.v {
    width: 2px;
    transform: translateX(-1px);
  }
  .guide.h {
    height: 2px;
    transform: translateY(-1px);
  }
  .plus {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    padding: 0 0 1px;
    border-radius: 50%;
    border: none;
    background: var(--accent);
    color: white;
    font: 600 14px/1 var(--ui);
    cursor: pointer;
    animation: appear 0.12s 0.08s both;
    z-index: 2;
  }
  .plus:hover {
    background: var(--accent-strong);
  }
  .plus.bar {
    transform: none;
    height: 16px;
    border-radius: 5px;
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .plus.bar:hover {
    background: var(--accent);
    color: white;
  }
  @keyframes appear {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>
