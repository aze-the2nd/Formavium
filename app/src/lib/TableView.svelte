<script lang="ts">
  // Tabelle im visuellen Editor, bedient wie eine Tabellenkalkulation:
  // Zellen auswählen (Klick, Ziehen, Umschalt+Pfeil), bearbeiten (Tippen,
  // Eingabe, F2, Doppelklick), Bereiche kopieren, einfügen, leeren und
  // verbinden. Drei Bearbeitungsarten: direkt in der Zelle, Raster mit
  // Eingabezeile, Zelleditor unten in der Ansicht.
  import { tick } from 'svelte';
  import EditableText from './EditableText.svelte';
  import { editor, seedInspector, type Target } from './editor.svelte';
  import { openMenu, type MenuItem } from './menus.svelte';
  import { texPreview } from './texpreview';
  import type { Col, Command, Doc, El } from './api';

  type Table = Extract<El, { kind: 'table' }>;
  type Props = {
    t: Table;
    part: string;
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
  };
  let { t, part, onCommand }: Props = $props();

  let frame = $state<HTMLDivElement>();
  let table = $state<HTMLTableElement>();
  let renaming = $state<string | null>(null);

  // --- Auswahl über Kennungen, damit sie Umordnungen übersteht ---

  type Pos = { r: number; c: number };
  let anchorId = $state<{ row: string; col: string } | null>(null);
  let extentId = $state<{ row: string; col: string } | null>(null);
  let focused = $state(false);
  let editing = $state<{ row: string; col: string; value: string } | null>(null);

  const rowIndex = (id: string) => t.rows.findIndex((r) => r.id === id);
  const colIndex = (id: string) => t.columns.findIndex((c) => c.id === id);
  const isCovered = (r: number, c: number) => r > 0 && !!t.rows[r]?.merged[c];
  function master(p: Pos): Pos {
    let r = p.r;
    while (r > 0 && t.rows[r]?.merged[p.c]) r--;
    return { r, c: p.c };
  }
  function spanEnd(r: number, c: number) {
    let end = r;
    while (t.rows[end + 1]?.merged[c]) end++;
    return end;
  }

  let anchor = $derived.by((): Pos | null => {
    if (!anchorId) return null;
    const r = rowIndex(anchorId.row);
    const c = colIndex(anchorId.col);
    return r < 0 || c < 0 ? null : master({ r, c });
  });
  let extent = $derived.by((): Pos | null => {
    if (!extentId || !anchor) return anchor;
    const r = rowIndex(extentId.row);
    const c = colIndex(extentId.col);
    return r < 0 || c < 0 ? anchor : { r, c };
  });
  let bounds = $derived.by(() => {
    if (!anchor || !extent) return null;
    let r0 = Math.min(anchor.r, extent.r);
    let r1 = Math.max(anchor.r, extent.r);
    const c0 = Math.min(anchor.c, extent.c);
    const c1 = Math.max(anchor.c, extent.c);
    // Verbundene Blöcke ganz einschließen.
    for (let c = c0; c <= c1; c++) {
      r0 = Math.min(r0, master({ r: r0, c }).r);
      r1 = Math.max(r1, spanEnd(r1, c));
    }
    return { r0, r1, c0, c1 };
  });
  const inRange = (r: number, c: number) =>
    !!bounds && c >= bounds.c0 && c <= bounds.c1 && spanEnd(r, c) >= bounds.r0 && r <= bounds.r1;
  // Eine verbundene Zelle allein ist kein Bereich.
  let isRange = $derived(!!anchor && !!extent && (master(extent).r !== anchor.r || extent.c !== anchor.c));

  function idAt(p: Pos) {
    const row = t.rows[p.r]?.id;
    const col = t.columns[p.c]?.id;
    return row && col ? { row, col } : null;
  }

  function select(p: Pos, extend = false, focus = true) {
    const m = master({ r: Math.max(0, Math.min(t.rows.length - 1, p.r)), c: Math.max(0, Math.min(t.columns.length - 1, p.c)) });
    if (!extend || !anchorId) anchorId = idAt(m);
    extentId = idAt(extend ? p : m);
    if (focus) tick().then(focusAnchor);
  }

  function cellEl(p: Pos) {
    return table?.querySelector<HTMLElement>(`td[data-r="${p.r}"][data-c="${p.c}"]`);
  }

  function focusAnchor() {
    if (!anchor) return;
    const el = cellEl(anchor);
    el?.focus({ preventScroll: true });
    el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }

  const address = (p: Pos) => `${letter(p.c)}${p.r + 1}`;
  function letter(c: number) {
    let name = '';
    for (let n = c + 1; n > 0; n = Math.floor((n - 1) / 26)) name = String.fromCharCode(65 + ((n - 1) % 26)) + name;
    return name;
  }
  const valueAt = (p: Pos) => t.rows[p.r]?.cells[p.c] ?? '';

  // Gewählte Stelle für die Markierung im PDF und den Zelleditor.
  $effect(() => {
    if (!focused || !anchor || !bounds) return;
    const targets: Target[] = [];
    for (let r = bounds.r0; r <= Math.min(bounds.r1, bounds.r0 + 40); r++) {
      for (let c = bounds.c0; c <= bounds.c1; c++) {
        if (isCovered(r, c)) continue;
        targets.push({ part, path: t.rows_path, row: t.rows[r].id, column: t.columns[c].id });
      }
    }
    editor.focus = targets;
  });

  $effect(() => {
    if (!focused || !anchor || editor.tableMode !== 'inspector') return;
    const p = anchor;
    const col = t.columns[p.c];
    const row = t.rows[p.r];
    if (!col || !row) return;
    editor.cell = {
      key: `${part}/${t.path}/${row.id}/${col.id}`,
      title: `${col.header}${t.caption ? ` · ${t.caption}` : ''}`,
      address: address(p),
      value: valueAt(p),
      latex: col.latex,
      commit: (v) => setCell(row.id, col.id, v),
      back: () => focusAnchor(),
    };
  });

  // Sprung aus der PDF-Vorschau.
  $effect(() => {
    const req = editor.request;
    if (!req || req.target.part !== part || req.target.path !== t.rows_path || !req.target.row) return;
    const r = rowIndex(req.target.row);
    const c = req.target.column ? colIndex(req.target.column) : 0;
    if (r < 0 || c < 0) return;
    focused = true;
    select({ r, c });
  });

  // --- Befehle ---

  async function run(cmd: Command) {
    hideInsert();
    await onCommand(cmd);
    await tick();
  }

  const setCell = (row: string, column: string, value: string) =>
    onCommand({ op: 'set_cell', part, path: t.rows_path, row, column, value });

  async function insertRow(index: number, keepCol = anchor?.c ?? 0) {
    await run({ op: 'insert_row', part, path: t.rows_path, index });
    select({ r: index, c: keepCol });
  }

  async function removeRows() {
    if (!bounds) return;
    const ids = t.rows.slice(bounds.r0, bounds.r1 + 1).map((r) => r.id);
    const next = t.rows[bounds.r1 + 1]?.id ?? t.rows[bounds.r0 - 1]?.id;
    const col = anchorId?.col;
    for (const row of ids) await onCommand({ op: 'remove_row', part, path: t.rows_path, row });
    if (next && col) {
      anchorId = extentId = { row: next, col };
      await tick();
      focusAnchor();
    }
  }

  async function moveRow(delta: number) {
    if (!anchor) return;
    const row = t.rows[anchor.r];
    const to = anchor.r + delta;
    if (!row || to < 0 || to >= t.rows.length) return;
    await run({ op: 'move_row', part, path: t.rows_path, row: row.id, to });
    focusAnchor();
  }

  async function insertColumn(index: number) {
    const before = new Set(t.columns.map((c) => c.id));
    await run({ op: 'insert_column', part, table: t.path, index, header: 'Spalte' });
    renaming = t.columns.find((c) => !before.has(c.id))?.id ?? null;
  }

  async function removeColumns() {
    if (!bounds) return;
    const ids = t.columns.slice(bounds.c0, bounds.c1 + 1).map((c) => c.id);
    if (ids.length >= t.columns.length) return;
    for (const column of ids) await onCommand({ op: 'remove_column', part, table: t.path, column });
    select({ r: anchor?.r ?? 0, c: Math.min(bounds.c0, t.columns.length - 1) });
  }

  async function moveColumn(from: number, to: number) {
    if (from === to || to < 0 || to >= t.columns.length) return;
    await run({ op: 'move_column', part, table: t.path, from, to });
    focusAnchor();
  }

  /** Bereich mit Werten füllen: Einfügen, Leeren. */
  async function fill(r: number, c: number, cells: string[][]) {
    await run({ op: 'paste_cells', part, table: t.path, row: r, column: c, cells });
  }

  async function clearRange() {
    if (!bounds) return;
    const block = Array.from({ length: bounds.r1 - bounds.r0 + 1 }, () =>
      Array.from({ length: bounds!.c1 - bounds!.c0 + 1 }, () => ''),
    );
    await fill(bounds.r0, bounds.c0, block);
    focusAnchor();
  }

  async function merge(on: boolean) {
    if (!bounds) return;
    await run({
      op: 'merge_cells',
      part,
      table: t.path,
      rows: t.rows.slice(bounds.r0, bounds.r1 + 1).map((r) => r.id),
      columns: t.columns.slice(bounds.c0, bounds.c1 + 1).map((c) => c.id),
      merge: on,
    });
    if (bounds) select({ r: bounds.r0, c: bounds.c0 });
  }

  const style = (c: Col, change: { align?: Col['align']; latex?: boolean }) =>
    run({ op: 'set_column_style', part, table: t.path, column: c.id, ...change });

  const setHeader = (colId: string, header: string) =>
    onCommand({ op: 'set_column_header', part, table: t.path, column: colId, header });

  // --- Bearbeiten ---

  function beginEdit(replacement?: string) {
    if (!anchor) return;
    if (editor.tableMode === 'inspector') {
      seedInspector(replacement ?? '');
      return;
    }
    if (editor.tableMode === 'grid') {
      const bar = frame?.parentElement?.querySelector<HTMLInputElement>('.formula input');
      if (bar) {
        if (replacement !== undefined) bar.value = replacement;
        bar.focus();
        if (replacement === undefined) bar.select();
        return;
      }
    }
    const id = idAt(anchor);
    if (!id) return;
    editing = { ...id, value: replacement ?? valueAt(anchor) };
    tick().then(() => {
      const input = table?.querySelector<HTMLInputElement>('.cell-input');
      input?.focus();
      if (replacement === undefined) input?.select();
      else input?.setSelectionRange(input.value.length, input.value.length);
    });
  }

  async function finishEdit(commit: boolean, then?: 'down' | 'right' | 'left') {
    const e = editing;
    if (!e) return;
    editing = null;
    const r = rowIndex(e.row);
    const c = colIndex(e.col);
    if (commit && r >= 0 && c >= 0 && e.value !== valueAt({ r, c })) await setCell(e.row, e.col, e.value);
    await tick();
    if (then === 'down') {
      const below = spanEnd(r, c) + 1;
      if (below >= t.rows.length) await insertRow(t.rows.length, c);
      else select({ r: below, c });
    } else if (then === 'right' || then === 'left') step(then === 'right' ? 1 : -1);
    else focusAnchor();
  }

  function editKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      finishEdit(true, 'down');
    } else if (e.key === 'Tab') {
      e.preventDefault();
      finishEdit(true, e.shiftKey ? 'left' : 'right');
    } else if (e.key === 'Escape') {
      e.preventDefault();
      finishEdit(false);
    }
  }

  /** Tab: nächste Zelle, am Zeilenende in die nächste Zeile. */
  function step(dir: 1 | -1) {
    if (!anchor) return;
    const n = t.columns.length;
    let i = anchor.r * n + anchor.c + dir;
    while (i >= 0 && i < t.rows.length * n && isCovered(Math.floor(i / n), i % n)) i += dir;
    if (i < 0 || i >= t.rows.length * n) return;
    select({ r: Math.floor(i / n), c: i % n });
  }

  function move(key: string, extend: boolean) {
    if (!anchor || !extent) return;
    const from = extend ? extent : anchor;
    const p = { ...from };
    if (key === 'ArrowLeft') p.c = Math.max(0, p.c - 1);
    if (key === 'ArrowRight') p.c = Math.min(t.columns.length - 1, p.c + 1);
    if (key === 'ArrowUp') p.r = Math.max(0, (extend ? p.r : master(p).r) - 1);
    if (key === 'ArrowDown') p.r = Math.min(t.rows.length - 1, (extend ? p.r : spanEnd(p.r, p.c)) + 1);
    if (key === 'Home') p.c = 0;
    if (key === 'End') p.c = t.columns.length - 1;
    select(p, extend);
  }

  function cellKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    let handled = true;
    if (e.altKey && e.key === 'ArrowUp') moveRow(-1);
    else if (e.altKey && e.key === 'ArrowDown') moveRow(1);
    else if (e.altKey && e.key === 'ArrowLeft' && anchor) moveColumn(anchor.c, anchor.c - 1);
    else if (e.altKey && e.key === 'ArrowRight' && anchor) moveColumn(anchor.c, anchor.c + 1);
    else if (!mod && /^(Arrow|Home$|End$)/.test(e.key)) move(e.key, e.shiftKey);
    else if (e.key === 'Enter' && mod && anchor) insertRow(e.shiftKey ? anchor.r : spanEnd(anchor.r, anchor.c) + 1);
    else if (e.key === 'Backspace' && mod && e.shiftKey) removeRows();
    else if (e.key === 'Enter' || e.key === 'F2') beginEdit();
    else if (e.key === 'Tab') step(e.shiftKey ? -1 : 1);
    else if (e.key === 'Delete' || e.key === 'Backspace') clearRange();
    else if (e.key === 'Escape') {
      // Zurück zum Abschnitt: Dort gelten Strg+↑/↓ und Entf für den Abschnitt.
      frame?.closest<HTMLElement>('li.part')?.focus();
    } else if (e.key.length === 1 && !mod && !e.altKey && !e.isComposing) beginEdit(e.key);
    else handled = false;
    if (handled) {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  // --- Zwischenablage ---

  function rangeText() {
    if (!bounds) return '';
    const lines: string[] = [];
    for (let r = bounds.r0; r <= bounds.r1; r++) {
      const cells: string[] = [];
      for (let c = bounds.c0; c <= bounds.c1; c++) cells.push(isCovered(r, c) ? '' : valueAt({ r, c }));
      lines.push(cells.join('\t'));
    }
    return lines.join('\n');
  }

  function onCopy(e: ClipboardEvent, cut = false) {
    if (editing || !(e.target as HTMLElement).matches('td')) return;
    e.clipboardData?.setData('text/plain', rangeText());
    e.preventDefault();
    if (cut) clearRange();
  }

  function parseBlock(text: string): string[][] {
    const lines = text.replace(/\r\n?/g, '\n').replace(/\n$/, '').split('\n');
    const delimiter = lines.some((l) => l.includes('\t')) ? '\t' : lines.length > 1 && lines.every((l) => l.includes(';')) ? ';' : null;
    return lines.map((l) => (delimiter ? l.split(delimiter) : [l]).map((v) => v.trim()));
  }

  async function onPaste(e: ClipboardEvent) {
    if (editing || !bounds || !(e.target as HTMLElement).matches('td')) return;
    const text = e.clipboardData?.getData('text/plain');
    if (!text) return;
    e.preventDefault();
    const block = parseBlock(text);
    await fill(bounds.r0, bounds.c0, block);
    anchorId = idAt({ r: bounds.r0, c: bounds.c0 });
    extentId = idAt({
      r: Math.min(t.rows.length - 1, bounds.r0 + block.length - 1),
      c: Math.min(t.columns.length - 1, bounds.c0 + Math.max(...block.map((l) => l.length)) - 1),
    });
    await tick();
    focusAnchor();
  }

  // --- Zeiger: auswählen und Bereiche ziehen ---

  let dragging = false;
  function cellDown(e: PointerEvent, r: number, c: number) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('.cell-input')) return;
    if (editing) finishEdit(true);
    e.preventDefault();
    focused = true;
    select({ r, c }, e.shiftKey);
    dragging = true;
    hideInsert();
  }
  function cellEnter(r: number, c: number) {
    if (dragging) select({ r, c }, true, false);
  }
  function pointerUp() {
    dragging = false;
  }

  // --- Spaltenkopf: wählen, ziehen, umbenennen ---

  let colDrag = $state<{ from: number; over: number; active: boolean } | null>(null);
  let colStartX = 0;
  function headDown(e: PointerEvent, i: number) {
    if (e.button !== 0 || renaming) return;
    colDrag = { from: i, over: i, active: false };
    colStartX = e.clientX;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function headMove(e: PointerEvent) {
    if (!colDrag) return;
    if (!colDrag.active && Math.abs(e.clientX - colStartX) < 5) return;
    colDrag.active = true;
    const th = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('th[data-col]');
    if (th && table?.contains(th)) colDrag.over = Number(th.dataset.col);
  }
  function headUp() {
    const d = colDrag;
    colDrag = null;
    if (!d) return;
    if (d.active) moveColumn(d.from, d.over);
    else selectColumn(d.from);
  }
  function selectColumn(c: number) {
    focused = true;
    anchorId = idAt({ r: 0, c });
    extentId = idAt({ r: t.rows.length - 1, c });
    tick().then(focusAnchor);
  }
  function selectRow(r: number) {
    focused = true;
    anchorId = idAt({ r, c: 0 });
    extentId = idAt({ r, c: t.columns.length - 1 });
    tick().then(focusAnchor);
  }

  // --- Kontextmenü ---

  function columnItems(c: number): MenuItem[] {
    const col = t.columns[c];
    return [
      { heading: `Spalte ${letter(c)} · ${col.header}` },
      { label: 'Spalte links einfügen', action: () => insertColumn(c) },
      { label: 'Spalte rechts einfügen', action: () => insertColumn(c + 1) },
      { label: 'Umbenennen', hint: 'Doppelklick', action: () => (renaming = col.id) },
      { label: 'Nach links', hint: 'Alt+←', disabled: c === 0, action: () => moveColumn(c, c - 1) },
      { label: 'Nach rechts', hint: 'Alt+→', disabled: c === t.columns.length - 1, action: () => moveColumn(c, c + 1) },
      { label: 'Linksbündig', checked: col.align === 'left', action: () => style(col, { align: 'left' }) },
      { label: 'Zentriert', checked: col.align === 'center', action: () => style(col, { align: 'center' }) },
      { label: 'Rechtsbündig', checked: col.align === 'right', action: () => style(col, { align: 'right' }) },
      {
        label: 'Zellen enthalten LaTeX',
        checked: col.latex,
        action: () => style(col, { latex: !col.latex }),
      },
      { label: 'Spalte löschen', danger: true, disabled: t.columns.length < 2, action: () => removeColumns() },
    ];
  }

  function cellMenu(e: MouseEvent, r: number, c: number) {
    e.preventDefault();
    e.stopPropagation();
    if (!inRange(r, c)) select({ r, c });
    focused = true;
    const anyMerged = !!bounds && t.rows.slice(bounds.r0, bounds.r1 + 1).some((row, i) =>
      row.merged.slice(bounds!.c0, bounds!.c1 + 1).some((m, k) => m || spanEnd(bounds!.r0 + i, bounds!.c0 + k) > bounds!.r0 + i),
    );
    const n = bounds ? bounds.r1 - bounds.r0 + 1 : 1;
    openMenu({ x: e.clientX, y: e.clientY }, [
      { label: 'Kopieren', hint: 'Strg+C', action: () => navigator.clipboard?.writeText(rangeText()) },
      { label: 'Leeren', hint: 'Entf', action: clearRange },
      { separator: true },
      { label: 'Zeile darüber einfügen', hint: 'Strg+Umschalt+Eingabe', action: () => insertRow(bounds?.r0 ?? r) },
      { label: 'Zeile darunter einfügen', hint: 'Strg+Eingabe', action: () => insertRow((bounds?.r1 ?? r) + 1) },
      { label: n > 1 ? `${n} Zeilen löschen` : 'Zeile löschen', hint: 'Strg+Umschalt+Rück', danger: true, action: removeRows },
      { separator: true },
      { label: 'Zellen verbinden', disabled: n < 2, action: () => merge(true) },
      { label: 'Verbindung aufheben', disabled: !anyMerged, action: () => merge(false) },
      { separator: true },
      ...columnItems(c),
    ]);
  }

  // --- Einfügestellen beim Überfahren ---

  type Gap = { axis: 'row' | 'col'; index: number; edge: number; along: number };
  let gap = $state<Gap | null>(null);
  let box = $state({ left: 0, top: 0, width: 0, height: 0 });

  function hover(e: PointerEvent) {
    if (!frame || !table || dragging || colDrag || e.pointerType === 'touch') return;
    if ((e.target as HTMLElement).closest('.insert, .cell-input')) return;
    const f = frame.getBoundingClientRect();
    const tb = table.getBoundingClientRect();
    const body = table.tBodies[0];
    const rows = Array.from(body.rows).filter((tr) => tr.dataset.row !== undefined);
    const head = table.tHead!.rows[table.tHead!.rows.length - 1];
    const cells = Array.from(head.cells).filter((c) => c.dataset.col !== undefined);
    if (!cells.length) return;
    const left = cells[0].getBoundingClientRect().left;
    const top = head.getBoundingClientRect().bottom;
    const bottom = rows.length ? rows[rows.length - 1].getBoundingClientRect().bottom : top;
    box = { left: left - f.left + frame.scrollLeft, top: tb.top - f.top, width: tb.right - left, height: bottom - tb.top };
    const x = e.clientX;
    const y = e.clientY;
    const keep = 9;
    const take = 6;
    let best: Gap | null = null;
    let bestD = Infinity;
    const consider = (g: Gap, d: number) => {
      const limit = gap && gap.axis === g.axis && gap.index === g.index ? keep : take;
      if (d <= limit && d < bestD) {
        best = g;
        bestD = d;
      }
    };
    if (x >= left - 16 && x <= tb.right + 16) {
      const edges = [top, ...rows.map((tr) => tr.getBoundingClientRect().bottom)];
      edges.forEach((edge, index) => consider({ axis: 'row', index, edge: edge - f.top, along: x - f.left + frame!.scrollLeft }, Math.abs(y - edge)));
    }
    if (y >= tb.top - 16 && y <= bottom + 16) {
      const edges = [left, ...cells.map((c) => c.getBoundingClientRect().right)];
      edges.forEach((edge, index) =>
        consider({ axis: 'col', index, edge: edge - f.left + frame!.scrollLeft, along: y - f.top }, Math.abs(x - edge)),
      );
    }
    gap = best;
  }
  function hideInsert() {
    gap = null;
  }
  function insertAtGap() {
    const g = gap;
    if (!g) return;
    if (g.axis === 'row') insertRow(g.index);
    else insertColumn(g.index);
  }

  // --- Eingabezeile im Raster ---

  let formula = $derived(anchor ? valueAt(anchor) : '');
  async function formulaKey(e: KeyboardEvent) {
    e.stopPropagation();
    const input = e.currentTarget as HTMLInputElement;
    if (e.key === 'Enter' && anchor) {
      e.preventDefault();
      const id = idAt(anchor);
      if (id && input.value !== valueAt(anchor)) await setCell(id.row, id.col, input.value);
      move('ArrowDown', false);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      input.value = formula;
      focusAnchor();
    }
  }

  const headerLabel = (c: Col) => (c.unit && c.unit_placement === 'header' ? ` [${c.unit}]` : '');
  let grid = $derived(editor.tableMode === 'grid');
</script>

<svelte:window onpointerup={pointerUp} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="tv"
  class:active={focused}
  class:grid
  onfocusin={() => (focused = true)}
  onfocusout={(e) => {
    if (!(e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget)) && !(e.relatedTarget as HTMLElement | null)?.closest?.('.inspector')) {
      focused = false;
    }
  }}
  oncopy={(e) => onCopy(e)}
  oncut={(e) => onCopy(e, true)}
  onpaste={onPaste}
>
  {#if t.caption}<div class="caption">{t.caption}</div>{/if}
  {#if grid}
    <label class="formula">
      <span class="coord">{anchor && focused ? address(anchor) : '–'}</span>
      <span class="fx">ƒx</span>
      <input
        value={focused ? formula : ''}
        disabled={!focused || !anchor}
        spellcheck="false"
        aria-label="Inhalt der gewählten Zelle"
        onkeydown={formulaKey}
      />
    </label>
  {/if}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="frame" bind:this={frame} onpointermove={hover} onpointerleave={hideInsert}>
    <table bind:this={table} aria-label={t.caption ?? 'Tabelle'} aria-multiselectable="true" role="grid">
      <colgroup>
        {#if grid}<col class="numcol" />{/if}
        {#each t.columns as c (c.id)}<col style:width={c.width ? `${Math.round(c.width * 100)}%` : null} />{/each}
      </colgroup>
      <thead>
        {#if grid}
          <tr class="letters">
            <th class="corner"></th>
            {#each t.columns as c, i (c.id)}
              <th class:current={focused && !!bounds && i >= bounds.c0 && i <= bounds.c1}>
                <button tabindex="-1" onclick={() => selectColumn(i)} aria-label="Spalte {letter(i)} auswählen">{letter(i)}</button>
              </th>
            {/each}
          </tr>
        {/if}
        <tr class="head">
          {#if grid}<th class="corner"></th>{/if}
          {#each t.columns as c, i (c.id)}
            <th
              data-col={i}
              style:text-align={c.align}
              class:over={colDrag?.active && colDrag.over === i && colDrag.from !== i}
              class:moving={colDrag?.active && colDrag.from === i}
              title="Klick wählt die Spalte, Ziehen verschiebt sie, Doppelklick benennt um"
              onpointerdown={(e) => headDown(e, i)}
              onpointermove={headMove}
              onpointerup={headUp}
              onpointercancel={() => (colDrag = null)}
              ondblclick={() => (renaming = c.id)}
              oncontextmenu={(e) => {
                e.preventDefault();
                selectColumn(i);
                openMenu({ x: e.clientX, y: e.clientY }, columnItems(i));
              }}
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
                    }
                  }}
                  onpointerdown={(e: PointerEvent) => e.stopPropagation()}
                />
              {:else}
                {c.header}<span class="unit">{headerLabel(c)}</span>{#if c.latex}<span class="tex" title="Zellen enthalten LaTeX">TeX</span>{/if}
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each t.rows as row, r (row.id)}
          <tr data-row={r}>
            {#if grid}
              <th class="num" class:current={focused && !!bounds && r >= bounds.r0 && r <= bounds.r1}>
                <button tabindex="-1" onclick={() => selectRow(r)} aria-label="Zeile {r + 1} auswählen">{r + 1}</button>
              </th>
            {/if}
            {#each t.columns as c, ci (c.id)}
              {#if !isCovered(r, ci)}
                {@const span = spanEnd(r, ci) - r + 1}
                {@const isAnchor = focused && !!anchor && anchor.r === r && anchor.c === ci}
                {@const chosen = focused && inRange(r, ci)}
                <td
                  rowspan={span > 1 ? span : undefined}
                  data-r={r}
                  data-c={ci}
                  tabindex={isAnchor || (!focused && r === 0 && ci === 0) ? 0 : -1}
                  class:selected={chosen && isRange}
                  class:anchor={isAnchor}
                  class:latex={c.latex}
                  style:text-align={c.align}
                  aria-label="{c.header}, Zeile {r + 1}{row.cells[ci] ? '' : ', leer'}"
                  aria-selected={chosen}
                  onpointerdown={(e) => cellDown(e, r, ci)}
                  onpointerenter={() => cellEnter(r, ci)}
                  ondblclick={() => beginEdit()}
                  onkeydown={cellKey}
                  oncontextmenu={(e) => cellMenu(e, r, ci)}
                >
                  {#if editing && editing.row === row.id && editing.col === c.id}
                    <input
                      class="cell-input"
                      class:mono={c.latex}
                      value={editing.value}
                      oninput={(e) => {
                        if (editing) editing.value = e.currentTarget.value;
                      }}
                      spellcheck="false"
                      aria-label="Zelle {address({ r, c: ci })} bearbeiten"
                      onkeydown={editKey}
                      onblur={() => finishEdit(true)}
                    />
                  {:else if row.cells[ci]}
                    {#if c.latex}{@html texPreview(row.cells[ci])}{:else}{row.cells[ci]}{/if}{#if c.unit && c.unit_placement === 'value'}<span
                        class="unit">&nbsp;{c.unit}</span
                      >{/if}
                  {/if}
                </td>
              {/if}
            {/each}
          </tr>
        {:else}
          <tr class="empty">
            <td colspan={t.columns.length + (grid ? 1 : 0)}>
              <button onclick={() => insertRow(0)}>Noch keine Zeilen – klicken, um die erste anzulegen</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    {#if gap}
      {#if gap.axis === 'row'}
        <div class="guide h" style:top="{gap.edge}px" style:left="{box.left}px" style:width="{box.width}px"></div>
      {:else}
        <div class="guide v" style:left="{gap.edge}px" style:top="{box.top}px" style:height="{box.height}px"></div>
      {/if}
      <button
        class="insert"
        tabindex="-1"
        title={gap.axis === 'row' ? `Zeile ${gap.index + 1} hier einfügen` : `Spalte ${letter(gap.index)} hier einfügen`}
        aria-label={gap.axis === 'row' ? 'Zeile hier einfügen' : 'Spalte hier einfügen'}
        style:left="{gap.axis === 'row' ? Math.min(Math.max(gap.along, box.left + 10), box.left + box.width - 10) : gap.edge}px"
        style:top="{gap.axis === 'row' ? gap.edge : Math.min(Math.max(gap.along, box.top + 10), box.top + box.height - 10)}px"
        onpointerdown={(e) => e.preventDefault()}
        onclick={insertAtGap}>+</button
      >
    {/if}
  </div>
  {#if focused}
    <div class="foot">
      <button class="quiet" onclick={() => insertRow(t.rows.length)}>＋ Zeile</button>
      <button class="quiet" onclick={() => insertColumn(t.columns.length)}>＋ Spalte</button>
      <span class="hint">
        {#if isRange && bounds}
          {bounds.r1 - bounds.r0 + 1} × {bounds.c1 - bounds.c0 + 1} Zellen · Strg+C kopiert, Entf leert
        {:else if editor.tableMode === 'inspector'}
          Eingabe bearbeitet die Zelle unten im Zelleditor
        {:else}
          Tippen oder Doppelklick bearbeitet · Umschalt+Pfeil markiert
        {/if}
      </span>
    </div>
  {/if}
</div>

<style>
  .tv {
    font-family: var(--ui);
    margin: 4px 0 10px;
  }
  .caption {
    font-weight: 600;
    margin-bottom: 6px;
  }
  .frame {
    position: relative;
    overflow-x: auto;
    padding: 2px;
    scrollbar-width: thin;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-size: 13px;
    table-layout: auto;
  }
  th,
  td {
    border: 1px solid var(--grid-line);
  }
  .head th {
    position: relative;
    font-weight: 600;
    padding: 5px 9px;
    background: var(--grid-head);
    white-space: nowrap;
    cursor: var(--cur-grab);
    user-select: none;
  }
  .head th:hover {
    background: var(--grid-head-hover);
  }
  .head th.over {
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .head th.moving {
    opacity: 0.45;
  }
  .unit {
    color: var(--muted);
    font-weight: 400;
  }
  .tex {
    margin-left: 6px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--accent-strong);
    vertical-align: 1px;
  }
  td {
    height: 30px;
    padding: 4px 9px;
    line-height: 1.35;
    position: relative;
    vertical-align: middle;
    cursor: var(--cur-cell);
    outline: none;
    user-select: none;
    overflow-wrap: anywhere;
  }
  td:hover {
    background: var(--field-hover);
  }
  td.selected {
    background: var(--sel-bg);
  }
  td.anchor {
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--sel-anchor);
  }
  td.latex {
    font-family: var(--serif);
    font-size: 14px;
  }
  td :global(sub),
  td :global(sup) {
    font-size: 0.72em;
  }
  .cell-input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    min-width: 0;
    font: inherit;
    color: var(--text);
    background: var(--panel);
    border: 2px solid var(--accent);
    padding: 3px 7px;
    outline: none;
    text-align: inherit;
    user-select: text;
  }
  .cell-input.mono {
    font-family: var(--mono);
    font-size: 12px;
  }

  /* Raster: Spaltenbuchstaben und Zeilennummern. */
  .numcol {
    width: 30px;
  }
  .letters th,
  .num,
  .corner {
    background: var(--grid-index);
    color: var(--muted);
    font: 10.5px var(--mono);
    font-weight: 400;
    padding: 0;
    text-align: center;
  }
  .letters th {
    height: 20px;
  }
  .letters button,
  .num button {
    width: 100%;
    height: 100%;
    min-height: 20px;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .letters button:hover,
  .num button:hover {
    color: var(--text);
    background: var(--grid-head-hover);
  }
  .current {
    background: var(--sel-bg) !important;
    color: var(--accent-strong) !important;
  }
  .formula {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 2px 6px;
  }
  .coord {
    min-width: 34px;
    font: 11px var(--mono);
    color: var(--accent-strong);
    text-align: center;
  }
  .fx {
    font: italic 13px var(--serif);
    color: var(--muted);
  }
  .formula input {
    flex: 1;
    min-width: 0;
    height: 26px;
    font: 12px var(--mono);
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    padding: 3px 8px;
  }
  .formula input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .formula input:disabled {
    opacity: 0.5;
  }

  .empty td {
    padding: 0;
    cursor: default;
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

  .guide {
    position: absolute;
    background: var(--accent);
    pointer-events: none;
    z-index: 2;
  }
  .guide.h {
    height: 2px;
    transform: translateY(-1px);
  }
  .guide.v {
    width: 2px;
    transform: translateX(-1px);
  }
  .insert {
    position: absolute;
    z-index: 3;
    transform: translate(-50%, -50%);
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    padding: 0 0 1px;
    border-radius: 50%;
    border: 2px solid var(--panel);
    background: var(--accent);
    color: white;
    font: 600 14px/1 var(--ui);
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
    animation: pop 0.1s ease-out;
  }
  .insert:hover {
    background: var(--accent-strong);
  }
  @keyframes pop {
    from {
      transform: translate(-50%, -50%) scale(0.6);
      opacity: 0;
    }
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 4px;
    font-size: 11px;
    color: var(--muted);
  }
  .quiet {
    font: inherit;
    color: var(--accent-strong);
    background: none;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 2px 6px;
    cursor: pointer;
  }
  .quiet:hover {
    border-color: var(--line);
    background: var(--panel-2);
  }
  .hint {
    margin-left: auto;
    text-align: right;
  }
</style>
