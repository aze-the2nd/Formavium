<script lang="ts">
  // Modul-Werkstatt: bearbeitet die Definition eines Moduls dieses Dokuments.
  // Jede Änderung wirkt sofort auf alle Verwendungen im Dokument und damit
  // auf visuelle Ansicht und PDF. „In Bibliothek speichern“ macht den Stand
  // wiederverwendbar.
  import EditableText from './EditableText.svelte';
  import {
    elementIds,
    freshId,
    latexFields,
    type ColumnDef,
    type ElementDef,
    type FieldDef,
    type ModuleDef,
    type TextRef,
  } from './library';
  import { openMenu, openMenuBelow, type MenuItem } from './menus.svelte';

  type Props = {
    def: ModuleDef;
    status: { local: boolean; newer: number | null } | null;
    /** Anzahl der Verwendungen in diesem Dokument. */
    uses: number;
    canPublish: boolean;
    onUpdate: (def: ModuleDef) => Promise<unknown>;
    onPublish: () => void;
    onClose: () => void;
  };
  let { def, status, uses, canPublish, onUpdate, onPublish, onClose }: Props = $props();

  type Kind = ElementDef['kind'];
  const kindName: Record<Kind, string> = {
    heading: 'Überschrift',
    paragraph: 'Text',
    list: 'Liste',
    table: 'Tabelle',
    image: 'Bild',
    latex: 'LaTeX',
    row: 'Nebeneinander',
    module: 'Untermodul',
  };

  // --- Definition ändern: Kopie bearbeiten, Felder aufräumen, senden ---

  function locate(body: ElementDef[], id: string): { list: ElementDef[]; index: number } | null {
    for (let index = 0; index < body.length; index++) {
      const el = body[index];
      if (el.id === id) return { list: body, index };
      if (el.kind === 'row') {
        const found = locate(el.children, id);
        if (found) return found;
      }
    }
    return null;
  }

  function usedFields(body: ElementDef[]): Set<string> {
    const used = new Set<string>();
    const ref = (t: TextRef | null) => {
      if (t && 'field' in t) used.add(t.field);
    };
    const walk = (list: ElementDef[]) => {
      for (const el of list) {
        if (el.kind === 'heading' || el.kind === 'paragraph') ref(el.text);
        else if (el.kind === 'table') {
          used.add(el.rows);
          ref(el.caption);
          el.columns.forEach((c) => ref(c.header));
        } else if (el.kind === 'image') used.add(el.path);
        else if (el.kind === 'list') used.add(el.items);
        else if (el.kind === 'latex') latexFields(el.source).forEach((f) => used.add(f.id));
        else if (el.kind === 'row') walk(el.children);
      }
    };
    walk(body);
    return used;
  }

  function mutate(change: (draft: ModuleDef) => void) {
    const draft = structuredClone($state.snapshot(def)) as ModuleDef;
    change(draft);
    // Felder, die ein LaTeX-Quelltext neu nennt, entstehen von selbst.
    const walk = (list: ElementDef[]) => {
      for (const el of list) {
        if (el.kind === 'row') walk(el.children);
        if (el.kind !== 'latex') continue;
        for (const f of latexFields(el.source)) {
          if (!f.id || draft.fields.some((x) => x.id === f.id)) continue;
          draft.fields.push({
            id: f.id,
            label: f.id.charAt(0).toUpperCase() + f.id.slice(1),
            kind: f.image ? 'image' : 'text',
            default: f.image ? null : { type: 'text', value: '' },
          });
        }
      }
    };
    walk(draft.body);
    // Nicht mehr verwendete Felder verschwinden.
    const used = usedFields(draft.body);
    draft.fields = draft.fields.filter((f) => used.has(f.id));
    return onUpdate(draft);
  }

  const editElement = (id: string, change: (el: ElementDef, draft: ModuleDef) => void) =>
    mutate((draft) => {
      const at = locate(draft.body, id);
      if (at) change(at.list[at.index], draft);
    });

  const editField = (id: string, change: (f: FieldDef) => void) =>
    mutate((draft) => {
      const f = draft.fields.find((x) => x.id === id);
      if (f) change(f);
    });

  const field = (id: string) => def.fields.find((f) => f.id === id);
  const fieldDefault = (id: string) => {
    const d = field(id)?.default;
    return d && (d.type === 'text' || d.type === 'number') ? d.value : '';
  };

  // --- Elemente hinzufügen, verschieben, entfernen ---

  function newField(draft: ModuleDef, stem: string, label: string, kind: FieldDef['kind'], value: FieldDef['default']) {
    const id = freshId(
      stem,
      draft.fields.map((f) => f.id),
    );
    draft.fields.push({ id, label, kind, default: value });
    return id;
  }

  function create(kind: Kind, draft: ModuleDef): ElementDef {
    const id = freshId(kind === 'row' ? 'reihe' : kindName[kind], elementIds(draft.body));
    switch (kind) {
      case 'heading':
        return {
          id,
          kind,
          level: 1,
          text: { field: newField(draft, 'titel', 'Titel', 'text', { type: 'text', value: 'Überschrift' }) },
        };
      case 'paragraph':
        return { id, kind, text: { field: newField(draft, 'text', 'Text', 'text', { type: 'text', value: 'Text' }) } };
      case 'list':
        return {
          id,
          kind,
          ordered: false,
          items: newField(draft, 'punkte', 'Punkte', 'table', {
            type: 'table',
            value: [{ id: 'z1', cells: { text: 'Erster Punkt' } }],
          }),
        };
      case 'table':
        return {
          id,
          kind,
          caption: null,
          rows: newField(draft, 'zeilen', 'Tabellenwerte', 'table', { type: 'table', value: [{ id: 'z1', cells: {} }] }),
          columns: ['Spalte 1', 'Spalte 2', 'Spalte 3'].map((header, i) => ({
            id: `s${i + 1}`,
            header: { literal: header },
            unit: null,
            unit_placement: 'header' as const,
            align: 'left' as const,
            width: null,
          })),
        };
      case 'image':
        return { id, kind, width: 1, path: newField(draft, 'bild', 'Bild', 'image', null) };
      case 'row':
        return { id, kind, children: [] };
      case 'latex':
        return { id, kind, source: '% LaTeX-Quelltext. Eingabefeld: \\feld{name}' };
      case 'module':
        return { id, kind, module: { id: '', version: 1 } };
    }
  }

  function add(kind: Kind, into?: string) {
    mutate((draft) => {
      const el = create(kind, draft);
      const row = into ? locate(draft.body, into) : null;
      const parent = row ? row.list[row.index] : null;
      if (parent?.kind === 'row') parent.children.push(el);
      else draft.body.push(el);
    });
  }

  const addable: Kind[] = ['heading', 'paragraph', 'list', 'table', 'image', 'latex', 'row'];
  const addItems = (into?: string): MenuItem[] => [
    { heading: into ? 'In die Reihe einfügen' : 'Element hinzufügen' },
    ...addable
      .filter((k) => !(into && k === 'row'))
      .map((k) => ({ label: kindName[k], action: () => add(k, into) })),
  ];

  function move(id: string, delta: number) {
    mutate((draft) => {
      const at = locate(draft.body, id);
      if (!at) return;
      const to = at.index + delta;
      if (to < 0 || to >= at.list.length) return;
      const [el] = at.list.splice(at.index, 1);
      at.list.splice(to, 0, el);
    });
  }

  function remove(id: string) {
    mutate((draft) => {
      const at = locate(draft.body, id);
      if (at) at.list.splice(at.index, 1);
    });
  }

  function elementMenu(e: MouseEvent, el: ElementDef) {
    if ((e.target as HTMLElement).closest('input, textarea')) return;
    e.preventDefault();
    e.stopPropagation();
    const at = locate(def.body, el.id);
    openMenu({ x: e.clientX, y: e.clientY }, [
      { heading: kindName[el.kind] },
      { label: 'Nach oben', disabled: !at || at.index === 0, action: () => move(el.id, -1) },
      { label: 'Nach unten', disabled: !at || at.index === at.list.length - 1, action: () => move(el.id, 1) },
      { label: 'Element entfernen', danger: true, action: () => remove(el.id) },
    ]);
  }

  // --- Text eines Elements: fester Text oder Eingabefeld ---

  function toggleField(el: ElementDef, label: string) {
    editElement(el.id, (e, draft) => {
      if (e.kind !== 'heading' && e.kind !== 'paragraph') return;
      if ('field' in e.text) {
        const d = draft.fields.find((f) => f.id === (e.text as { field: string }).field)?.default;
        e.text = { literal: d && d.type === 'text' ? d.value : '' };
      } else {
        const value = e.text.literal;
        e.text = { field: newField(draft, label, label, 'text', { type: 'text', value }) };
      }
    });
  }

  const setDefault = (id: string, value: string) =>
    editField(id, (f) => (f.default = { type: f.kind === 'number' ? 'number' : 'text', value }));

  // --- Tabellenspalten ---

  const editColumn = (elId: string, colId: string, change: (c: ColumnDef) => void) =>
    editElement(elId, (e) => {
      if (e.kind !== 'table') return;
      const c = e.columns.find((x) => x.id === colId);
      if (c) change(c);
    });

  function addColumn(elId: string) {
    editElement(elId, (e) => {
      if (e.kind !== 'table') return;
      const id = freshId(
        's',
        e.columns.map((c) => c.id),
      );
      e.columns.push({
        id,
        header: { literal: `Spalte ${e.columns.length + 1}` },
        unit: null,
        unit_placement: 'header',
        align: 'left',
        width: null,
      });
    });
  }

  function columnMenu(e: MouseEvent, el: ElementDef & { kind: 'table' }, i: number) {
    const c = el.columns[i];
    const shift = (delta: number) =>
      editElement(el.id, (x) => {
        if (x.kind !== 'table') return;
        const [col] = x.columns.splice(i, 1);
        x.columns.splice(i + delta, 0, col);
      });
    openMenuBelow(e.currentTarget as Element, [
      { heading: 'Ausrichtung' },
      ...(['left', 'center', 'right'] as const).map((a) => ({
        label: { left: 'Links', center: 'Mittig', right: 'Rechts' }[a],
        checked: c.align === a,
        action: () => editColumn(el.id, c.id, (x) => (x.align = a)),
      })),
      { heading: 'Einheit anzeigen' },
      {
        label: 'Im Spaltenkopf',
        checked: c.unit_placement === 'header',
        action: () => editColumn(el.id, c.id, (x) => (x.unit_placement = 'header')),
      },
      {
        label: 'Hinter jedem Wert',
        checked: c.unit_placement === 'value',
        action: () => editColumn(el.id, c.id, (x) => (x.unit_placement = 'value')),
      },
      { separator: true },
      { label: 'Nach links', disabled: i === 0, action: () => shift(-1) },
      { label: 'Nach rechts', disabled: i === el.columns.length - 1, action: () => shift(1) },
      {
        label: 'Spalte entfernen',
        danger: true,
        disabled: el.columns.length < 2,
        action: () =>
          editElement(el.id, (x) => {
            if (x.kind === 'table') x.columns.splice(i, 1);
          }),
      },
    ]);
  }

  const percent = (w: number | null | undefined) => (w ? String(Math.round(w * 100)) : '');
  const parsePercent = (text: string) => {
    const n = Number(text.replace(',', '.').replace('%', '').trim());
    return Number.isFinite(n) && n > 0 ? Math.min(100, Math.max(3, n)) / 100 : null;
  };

  // --- LaTeX: Auswahl zum Eingabefeld machen ---

  function selectionToField(el: ElementDef, area: HTMLTextAreaElement | null) {
    if (!area || el.kind !== 'latex') return;
    const { selectionStart: from, selectionEnd: to, value } = area;
    const selected = value.slice(from, to);
    const stem = selected.trim().split(/\s+/)[0] || 'feld';
    editElement(el.id, (e, draft) => {
      if (e.kind !== 'latex') return;
      const id = newField(draft, stem, selected.trim() || 'Feld', 'text', { type: 'text', value: selected });
      e.source = `${value.slice(0, from)}\\feld{${id}}${value.slice(to)}`;
    });
  }
</script>

{#snippet textSource(el: ElementDef & { kind: 'heading' | 'paragraph' }, label: string)}
  {#if 'field' in el.text}
    {@const id = el.text.field}
    <div class="pair">
      <span class="tag">Feld</span>
      <EditableText
        value={field(id)?.label ?? id}
        label="Bezeichnung des Feldes"
        placeholder="Bezeichnung"
        onCommit={(v: string) => editField(id, (f) => (f.label = v))}
      />
    </div>
    <div class="pair">
      <span class="tag muted">Vorgabe</span>
      <EditableText
        multiline={el.kind === 'paragraph'}
        value={fieldDefault(id)}
        label="Vorgabewert"
        placeholder="leer"
        onCommit={(v: string) => setDefault(id, v)}
      />
    </div>
  {:else}
    <div class="pair">
      <span class="tag muted">Fest</span>
      <EditableText
        multiline={el.kind === 'paragraph'}
        value={el.text.literal}
        label="Fester Text"
        placeholder={label}
        onCommit={(v: string) =>
          editElement(el.id, (e) => {
            if (e.kind === 'heading' || e.kind === 'paragraph') e.text = { literal: v };
          })}
      />
    </div>
  {/if}
{/snippet}

{#snippet element(el: ElementDef)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="el" oncontextmenu={(e) => elementMenu(e, el)}>
    <div class="el-head">
      <span class="kind">{kindName[el.kind]}</span>
      {#if el.kind === 'heading'}
        <span class="seg" role="group" aria-label="Ebene">
          {#each [1, 2, 3] as level}
            <button
              class:on={el.level === level}
              title="Ebene {level}"
              onclick={() =>
                editElement(el.id, (e) => {
                  if (e.kind === 'heading') e.level = level;
                })}>{level}</button
            >
          {/each}
        </span>
      {/if}
      {#if el.kind === 'heading' || el.kind === 'paragraph'}
        <button
          class="chip"
          class:on={'field' in el.text}
          title="Eingabefeld: Der Text wird in jeder Verwendung ausgefüllt. Sonst steht er fest im Modul."
          onclick={() => toggleField(el, el.kind === 'heading' ? 'Titel' : 'Text')}>Eingabefeld</button
        >
      {/if}
      {#if el.kind === 'list'}
        <button
          class="chip"
          class:on={el.ordered}
          onclick={() =>
            editElement(el.id, (e) => {
              if (e.kind === 'list') e.ordered = !e.ordered;
            })}>Nummeriert</button
        >
      {/if}
      <span class="spacer"></span>
      <button class="more" title="Verschieben oder entfernen" aria-label="Aktionen" onclick={(e) => elementMenu(e, el)}
        >⋯</button
      >
    </div>

    {#if el.kind === 'heading'}
      {@render textSource(el, 'Überschrift')}
    {:else if el.kind === 'paragraph'}
      {@render textSource(el, 'Text')}
    {:else if el.kind === 'table'}
      <div class="cols">
        {#each el.columns as c, i (c.id)}
          <div class="col">
            <EditableText
              value={'literal' in c.header ? c.header.literal : (field(c.header.field)?.label ?? '')}
              label="Spaltenkopf"
              placeholder="Kopf"
              onCommit={(v: string) => editColumn(el.id, c.id, (x) => (x.header = { literal: v }))}
            />
            <EditableText
              value={c.unit ?? ''}
              label="Einheit"
              placeholder="Einheit"
              onCommit={(v: string) => editColumn(el.id, c.id, (x) => (x.unit = v.trim() || null))}
            />
            <EditableText
              value={percent(c.width)}
              label="Breite in Prozent"
              placeholder="auto %"
              onCommit={(v: string) => editColumn(el.id, c.id, (x) => (x.width = parsePercent(v)))}
            />
            <button class="more" title="Ausrichtung, Einheit, Reihenfolge" aria-label="Spaltenoptionen" onclick={(e) => columnMenu(e, el, i)}
              >⋯</button
            >
          </div>
        {/each}
        <button class="add-line" onclick={() => addColumn(el.id)}>+ Spalte</button>
      </div>
    {:else if el.kind === 'image'}
      <label class="pair">
        <span class="tag muted">Breite</span>
        <input
          type="range"
          min="10"
          max="100"
          step="5"
          value={Math.round(el.width * 100)}
          onchange={(e) => {
            const width = Number(e.currentTarget.value) / 100;
            editElement(el.id, (x) => {
              if (x.kind === 'image') x.width = width;
            });
          }}
        />
        <span class="num">{Math.round(el.width * 100)} %</span>
      </label>
    {:else if el.kind === 'latex'}
      {@const id = `ws-latex-${el.id}`}
      <div class="code" {id}>
        <EditableText
          multiline
          value={el.source}
          label="LaTeX-Quelltext"
          placeholder="LaTeX"
          onCommit={(v: string) =>
            editElement(el.id, (e) => {
              if (e.kind === 'latex') e.source = v;
            })}
        />
      </div>
      <div class="hint">
        <button
          class="add-line"
          title="Ersetzt die markierte Stelle durch ein Eingabefeld. Der markierte Text wird die Vorgabe."
          onclick={() => selectionToField(el, document.querySelector<HTMLTextAreaElement>(`#${CSS.escape(id)} textarea`))}
          >Markierten Text zum Eingabefeld machen</button
        >
        <span><code>\feld&#123;name&#125;</code> Text · <code>\rohfeld&#123;name&#125;</code> LaTeX · <code>\bildfeld[…]&#123;name&#125;</code> Bild</span>
      </div>
    {:else if el.kind === 'row'}
      <div class="children">
        {#each el.children as child (child.id)}
          {@render element(child)}
        {/each}
        <button class="add-line" onclick={(e) => openMenuBelow(e.currentTarget, addItems(el.id))}>+ Element in die Reihe</button>
      </div>
    {:else if el.kind === 'module'}
      <div class="pair"><span class="tag muted">Modul</span><span>{el.module.id} · v{el.module.version}</span></div>
    {/if}
  </div>
{/snippet}

<div class="workshop">
  <header>
    <span class="title">Modul-Werkstatt</span>
    <span class="spacer"></span>
    <button class="close" title="Werkstatt schließen (Esc)" aria-label="Schließen" onclick={onClose}>×</button>
  </header>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="scroll"
    onkeydown={(e) => {
      if (e.key === 'Escape' && !(e.target as HTMLElement).closest('input, textarea')) onClose();
    }}
  >
    <div class="name">
      <EditableText
        value={def.name}
        label="Name des Moduls"
        placeholder="Name"
        onCommit={(v: string) => mutate((d) => (d.name = v.trim() || d.name))}
      />
    </div>
    <div class="desc">
      <EditableText
        multiline
        value={def.description ?? ''}
        label="Beschreibung"
        placeholder="Kurzbeschreibung für die Modulauswahl"
        onCommit={(v: string) => mutate((d) => (d.description = v.trim()))}
      />
    </div>

    <div class="state">
      <span>
        Version {def.version}
        {#if status?.local}· noch nicht in der Bibliothek{:else}· wie in der Bibliothek{/if}
        {#if uses > 1}· {uses} Verwendungen in diesem Dokument{/if}
      </span>
      <button class="primary small" disabled={!canPublish || !status?.local} onclick={onPublish}>In Bibliothek speichern</button>
    </div>

    <h3>Aufbau</h3>
    <div class="body">
      {#each def.body as el (el.id)}
        {@render element(el)}
      {:else}
        <p class="empty">Noch keine Elemente.</p>
      {/each}
    </div>
    <button class="add" onclick={(e) => openMenuBelow(e.currentTarget, addItems())}>+ Element</button>

    {#if def.fields.length}
      <h3>Eingabefelder</h3>
      <dl class="fields">
        {#each def.fields as f (f.id)}
          <dt>
            <EditableText
              value={f.label}
              label="Bezeichnung"
              placeholder={f.id}
              onCommit={(v: string) => editField(f.id, (x) => (x.label = v))}
            />
          </dt>
          <dd>
            {#if f.kind === 'text' || f.kind === 'number'}
              <EditableText
                value={fieldDefault(f.id)}
                label="Vorgabewert"
                placeholder="Vorgabe leer"
                onCommit={(v: string) => setDefault(f.id, v)}
              />
            {:else}
              <span class="tag muted">{f.kind === 'image' ? 'Bild' : 'Tabelle'}</span>
            {/if}
          </dd>
        {/each}
      </dl>
    {/if}
  </div>
</div>

<style>
  .workshop {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    font-size: 13px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    padding: 3px 6px 3px 12px;
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent-strong);
  }
  .spacer {
    flex: 1;
  }
  .close,
  .more {
    font: inherit;
    width: 22px;
    height: 22px;
    line-height: 1;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .close:hover,
  .more:hover {
    background: var(--line);
    color: var(--text);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 12px 14px 40px;
    scrollbar-gutter: stable;
  }
  .name {
    --et-size: 17px;
    --et-weight: 650;
    margin: 0 -6px;
  }
  .desc {
    margin: 0 -6px 8px;
    color: var(--text-2);
  }
  .state {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--panel-2);
    font-size: 12px;
    color: var(--text-2);
  }
  .state span {
    flex: 1;
  }
  h3 {
    margin: 18px 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .body,
  .children {
    display: grid;
    gap: 6px;
  }
  .children {
    margin-top: 6px;
    padding-left: 10px;
    border-left: 2px solid var(--line-strong);
  }
  .el {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 6px 8px 8px;
    background: var(--panel);
    min-width: 0;
  }
  .el:focus-within {
    border-color: var(--accent);
  }
  .el-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 2px;
  }
  .kind {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-2);
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  .seg button {
    font: inherit;
    font-size: 11px;
    width: 22px;
    border: none;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .seg button.on {
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-weight: 600;
  }
  .chip {
    font: inherit;
    font-size: 11px;
    padding: 1px 8px;
    border-radius: 10px;
    border: 1px solid var(--line-strong);
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .chip.on {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }
  .pair {
    display: grid;
    grid-template-columns: 58px minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 4px;
  }
  .tag {
    font-size: 11px;
    color: var(--accent-strong);
  }
  .tag.muted {
    color: var(--muted);
  }
  .num {
    font-size: 11px;
    color: var(--muted);
    width: 40px;
    text-align: right;
  }
  .cols {
    display: grid;
    gap: 1px;
  }
  .col {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr) 64px 22px;
    align-items: center;
    gap: 2px;
  }
  .code {
    --et-font: var(--mono);
    --et-size: 12px;
    --et-line: 1.5;
    background: var(--panel-2);
    border-radius: 6px;
  }
  .hint {
    display: grid;
    gap: 2px;
    margin-top: 4px;
    font-size: 11px;
    color: var(--muted);
  }
  code {
    font-family: var(--mono);
  }
  .add,
  .add-line {
    font: inherit;
    font-size: 12px;
    color: var(--accent-strong);
    background: none;
    border: none;
    padding: 4px 2px;
    cursor: pointer;
    text-align: left;
    justify-self: start;
  }
  .add {
    margin-top: 6px;
    font-weight: 600;
  }
  .add:hover,
  .add-line:hover {
    text-decoration: underline;
  }
  .empty {
    margin: 0;
    color: var(--muted);
  }
  .fields {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr);
    gap: 1px 8px;
    align-items: baseline;
    margin: 0;
  }
  .fields dt,
  .fields dd {
    margin: 0;
    min-width: 0;
  }
  .fields dt {
    font-weight: 600;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
