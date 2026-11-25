<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import SourceView from './lib/SourceView.svelte';
  import VisualView from './lib/VisualView.svelte';
  import PdfView from './lib/PdfView.svelte';
  import Dialog from './lib/Dialog.svelte';
  import Menu from './lib/Menu.svelte';
  import Shortcuts from './lib/Shortcuts.svelte';
  import { confirmDialog } from './lib/dialogs.svelte';
  import { openMenuBelow, type MenuItem } from './lib/menus.svelte';
  import { clearResources } from './lib/resources';
  import { notify, toastState } from './lib/toast.svelte';
  import {
    call,
    isTauri,
    pickDir,
    pickOpen,
    pickSave,
    toBase64,
    type Command,
    type Diag,
    type Doc,
    type Preview,
    type SourceEditResult,
  } from './lib/api';

  let doc = $state<Doc | null>(null);
  let preview = $state<Preview | null>(null);
  let sourceView = $state<SourceView>();
  let showKeys = $state(false);

  const fvm = [{ name: 'Formavium-Dokument', extensions: ['fvm'] }];
  const defaultDir = isTauri ? '' : '/work/out/';

  async function guarded<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn();
    } catch (e) {
      notify('error', e instanceof Error ? e.message : String(e));
      return undefined;
    }
  }

  async function pollPreview() {
    if (!doc) return;
    try {
      preview = await call<Preview>('preview');
    } catch {
      /* Verbindung kurz weg: beim nächsten Takt erneut */
    }
  }

  let pollTimer: ReturnType<typeof setInterval>;
  onMount(async () => {
    doc = (await guarded(() => call<Doc | null>('state'))) ?? null;
    pollTimer = setInterval(pollPreview, 300);
    pollPreview();
  });
  onDestroy(() => clearInterval(pollTimer));

  $effect(() => {
    document.title = doc ? `${doc.title}${doc.dirty ? ' •' : ''} – Formavium` : 'Formavium';
  });

  // --- Dokumentbefehle ---

  // Befehle nacheinander ausführen: Jeder baut auf der Revision auf, die der
  // vorige hinterlassen hat (etwa Zelle setzen, dann Zeile einfügen).
  let queue: Promise<unknown> = Promise.resolve();
  function enqueue<T>(job: () => Promise<T>): Promise<T> {
    const run = queue.then(job);
    queue = run.catch(() => undefined);
    return run;
  }

  function command(cmd: Command): Promise<Doc | undefined> {
    return enqueue(async () => {
      if (!doc) return undefined;
      const base = doc.revision;
      const next = await guarded(() => call<Doc>('edit', { base, command: cmd }));
      if (next) doc = next;
      pollPreview();
      return next;
    });
  }

  async function sourceEdit(change: { base: number; from: number; to: number; insert: string }): Promise<Doc> {
    let r = await call<SourceEditResult>('source_edit', change);
    if (r.outcome === 'needs_conversion') {
      const ok = await confirmDialog({
        title: 'Visuelle Zuordnung aufheben?',
        message: r.message,
        okLabel: 'In freies LaTeX umwandeln',
        cancelLabel: 'Änderung verwerfen',
        danger: true,
      });
      if (!ok) return doc!;
      r = await call<SourceEditResult>('source_edit', { ...change, allow_convert: true });
    }
    if (r.outcome === 'needs_conversion') return doc!;
    if (r.outcome === 'rejected' || r.outcome === 'stale') notify('info', r.message);
    if (r.outcome === 'applied' && r.converted) notify('info', 'Abschnitt in freies LaTeX umgewandelt. Strg+Z stellt das Modul wieder her.');
    doc = r.doc;
    pollPreview();
    return r.doc;
  }

  const history = (name: 'undo' | 'redo') =>
    enqueue(async () => {
      const d = await guarded(() => call<Doc>(name));
      if (d) doc = d;
    });
  const undo = () => history('undo');
  const redo = () => history('redo');

  async function save() {
    if (!doc) return;
    const d = await enqueue(() => guarded(() => call<Doc>('save')));
    if (d) {
      doc = d;
      notify('info', `Gespeichert: ${d.path}`);
    }
  }

  function opened(d: Doc) {
    clearResources();
    doc = d;
    preview = null;
  }

  async function openDoc() {
    const path = await pickOpen('Dokument öffnen', fvm, `${defaultDir}anleitung.fvm`);
    if (!path) return;
    const d = await guarded(() => call<Doc>('open', { path }));
    if (d) opened(d);
  }

  async function newSample() {
    const path = await pickSave('Übungsdokument „Montageanleitung“ anlegen', fvm, `${defaultDir}anleitung.fvm`);
    if (!path) return;
    const d = await guarded(() => call<Doc>('new_sample', { path }));
    if (d) {
      opened(d);
      // Das Dokument liegt in einem eigenen Ordner mit seinem Namen.
      notify('info', `Angelegt: ${d.path}`);
    }
  }

  async function savePdf() {
    if (!doc || !preview) return;
    if (preview.status === 'empty' || preview.status === 'failed') {
      notify('error', 'Es gibt noch kein erfolgreich erzeugtes PDF.');
      return;
    }
    if (preview.status === 'stale') {
      const ok = await confirmDialog({
        title: 'Älteren Stand speichern?',
        message: `Die Vorschau zeigt Revision ${preview.shown_revision}. Revision ${preview.failed_revision} enthält Fehler. Gespeichert wird genau das angezeigte PDF.`,
        okLabel: 'Angezeigtes PDF speichern',
      });
      if (!ok) return;
    } else if (doc.notices.length) {
      const ok = await confirmDialog({
        title: 'Hinweise vor der Ausgabe',
        message: doc.notices.map((n) => `• ${n.message}`).join('\n'),
        okLabel: 'Trotzdem speichern',
      });
      if (!ok) return;
    }
    const path = await pickSave('Als PDF speichern', [{ name: 'PDF', extensions: ['pdf'] }], doc.path.replace(/\.fvm$/, '.pdf'));
    if (!path) return;
    const r = await guarded(() => call<{ revision: number; sha256: string }>('save_pdf', { path }));
    if (r) notify('info', `PDF gespeichert (Revision ${r.revision}, SHA-256 ${r.sha256.slice(0, 12)}…)`);
  }

  async function exportLatex() {
    if (!doc) return;
    const dir = await pickDir('LaTeX-Projekt exportieren nach', doc.path.replace(/\.fvm$/, '-latex'));
    if (!dir) return;
    const r = await guarded(() => call<{ resources: number }>('export', { dir }));
    if (r) notify('info', `LaTeX-Projekt exportiert (${r.resources} Ressourcen) nach ${dir}`);
  }

  /** Bild aus Dateiauswahl, Ablegen oder Zwischenablage übernehmen. */
  async function setImage(part: string, field: string, file: File) {
    const r = await guarded(async () =>
      call<{ path: string }>('import_image_data', { name: file.name, data: await toBase64(file) }),
    );
    if (r) await command({ op: 'set_value', part, path: field, value: { type: 'image', value: r.path } });
  }

  // --- Ansichten als Kacheln: umordnen, Breite ändern, ein- und ausblenden ---

  type PaneKey = 'source' | 'visual' | 'pdf';
  const paneKeys: PaneKey[] = ['source', 'visual', 'pdf'];
  const paneTitle: Record<PaneKey, string> = { source: 'Quelltext', visual: 'Visuell', pdf: 'PDF' };
  const paneLabel: Record<PaneKey, string> = {
    source: 'LaTeX-Quelltext',
    visual: 'Visueller Editor',
    pdf: 'PDF-Vorschau',
  };
  type Layout = { order: PaneKey[]; show: Record<PaneKey, boolean>; size: Record<PaneKey, number> };

  function loadLayout(): Layout {
    const layout: Layout = {
      order: [...paneKeys],
      show: { source: true, visual: true, pdf: true },
      size: { source: 1, visual: 1, pdf: 1 },
    };
    try {
      const saved = JSON.parse(localStorage.getItem('formavium.layout') ?? 'null') as Partial<Layout> | null;
      if (!saved) return layout;
      const order = (saved.order ?? []).filter((k) => paneKeys.includes(k));
      for (const k of paneKeys) {
        if (!order.includes(k)) order.push(k);
        const show = saved.show?.[k];
        if (typeof show === 'boolean') layout.show[k] = show;
        const size = saved.size?.[k];
        if (typeof size === 'number' && size > 0.05 && size < 20) layout.size[k] = size;
      }
      if (!paneKeys.some((k) => layout.show[k])) layout.show.visual = true;
      layout.order = [...new Set(order)];
    } catch {
      /* ohne gespeicherte Anordnung gilt die Vorgabe */
    }
    return layout;
  }

  let layout = $state(loadLayout());
  let visible = $derived(layout.order.filter((k) => layout.show[k]));
  let panesEl = $state<HTMLElement>();

  $effect(() => {
    const snapshot = JSON.stringify(layout);
    try {
      localStorage.setItem('formavium.layout', snapshot);
    } catch {
      /* ohne Speicher gilt die Anordnung nur für diese Sitzung */
    }
  });

  function toggle(key: PaneKey) {
    if (layout.show[key] && visible.length === 1) return;
    layout.show[key] = !layout.show[key];
  }

  /** Sichtbare Kachel an eine neue Stelle unter den sichtbaren setzen. */
  function movePane(key: PaneKey, to: number) {
    const rest = visible.filter((k) => k !== key);
    rest.splice(Math.max(0, Math.min(to, rest.length)), 0, key);
    layout.order = [...rest, ...layout.order.filter((k) => !layout.show[k])];
  }

  function paneItems(key: PaneKey): MenuItem[] {
    const i = visible.indexOf(key);
    return [
      { heading: paneTitle[key] },
      { label: 'Nach links', disabled: i === 0, action: () => movePane(key, i - 1) },
      { label: 'Nach rechts', disabled: i === visible.length - 1, action: () => movePane(key, i + 1) },
      { label: 'Ausblenden', disabled: visible.length === 1, action: () => toggle(key) },
    ];
  }

  let paneDrag = $state<{ key: PaneKey; over: number | null; active: boolean } | null>(null);
  let paneStartX = 0;

  function paneDown(e: PointerEvent, key: PaneKey) {
    if (e.button !== 0) return;
    paneDrag = { key, over: null, active: false };
    paneStartX = e.clientX;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function paneMove(e: PointerEvent) {
    if (!paneDrag || !panesEl) return;
    if (!paneDrag.active && Math.abs(e.clientX - paneStartX) < 6) return;
    paneDrag.active = true;
    const panes = Array.from(panesEl.querySelectorAll<HTMLElement>(':scope > .pane'));
    let over = panes.length;
    for (let k = 0; k < panes.length; k++) {
      const r = panes[k].getBoundingClientRect();
      if (e.clientX < r.left + r.width / 2) {
        over = k;
        break;
      }
    }
    paneDrag.over = over;
  }

  function paneUp(e: PointerEvent) {
    const d = paneDrag;
    paneDrag = null;
    if (!d) return;
    if (!d.active) {
      // Klick ohne Ziehen: Aktionen der Ansicht.
      openMenuBelow(e.currentTarget as HTMLElement, paneItems(d.key));
      return;
    }
    if (d.over === null) return;
    const from = visible.indexOf(d.key);
    const to = d.over > from ? d.over - 1 : d.over;
    if (to !== from) movePane(d.key, to);
  }

  /** Einfügemarke beim Ziehen einer Kachel. */
  function paneDropBefore(i: number) {
    const d = paneDrag;
    if (!d?.active || d.over !== i) return false;
    const from = visible.indexOf(d.key);
    return i !== from && i !== from + 1;
  }
  function paneDropAfter(i: number) {
    const d = paneDrag;
    return !!d?.active && d.over === visible.length && i === visible.length - 1 && visible[i] !== d.key;
  }

  /** Trennlinie zwischen zwei Kacheln ziehen. */
  function splitDown(e: PointerEvent, i: number) {
    if (e.button !== 0 || !panesEl) return;
    const [a, b] = [visible[i - 1], visible[i]];
    const panes = panesEl.querySelectorAll<HTMLElement>(':scope > .pane');
    const wa = panes[i - 1].getBoundingClientRect().width;
    const wb = panes[i].getBoundingClientRect().width;
    const total = layout.size[a] + layout.size[b];
    const startX = e.clientX;
    const bar = e.currentTarget as HTMLElement;
    bar.setPointerCapture(e.pointerId);
    const min = Math.min(220, (wa + wb) / 2);
    const move = (ev: PointerEvent) => {
      const na = Math.max(min, Math.min(wa + wb - min, wa + ev.clientX - startX));
      layout.size[a] = (na / (wa + wb)) * total;
      layout.size[b] = total - layout.size[a];
    };
    const up = () => {
      bar.removeEventListener('pointermove', move);
      bar.removeEventListener('pointerup', up);
      bar.removeEventListener('pointercancel', up);
    };
    bar.addEventListener('pointermove', move);
    bar.addEventListener('pointerup', up);
    bar.addEventListener('pointercancel', up);
  }

  function splitReset(i: number) {
    const [a, b] = [visible[i - 1], visible[i]];
    const half = (layout.size[a] + layout.size[b]) / 2;
    layout.size[a] = half;
    layout.size[b] = half;
  }

  // --- Navigation ---

  function jump(offset: number) {
    layout.show.source = true;
    queueMicrotask(() => sourceView?.jumpTo(offset));
  }

  function jumpDiag(d: Diag) {
    layout.show.source = true;
    queueMicrotask(() => {
      if (d.line && preview?.diagnostics_current) sourceView?.jumpToLine(d.line);
      else if (d.part) {
        const p = doc?.parts.find((x) => x.id === d.part);
        if (p) sourceView?.jumpTo(p.offset);
      }
    });
  }

  let errorDiags = $derived(preview?.diagnostics.filter((d) => d.severity === 'error') ?? []);
  let errorLines = $derived(
    preview?.diagnostics_current ? errorDiags.flatMap((d) => (d.line ? [d.line] : [])) : [],
  );
  let errorParts = $derived(errorDiags.flatMap((d) => (d.part ? [d.part] : [])));

  function onKey(e: KeyboardEvent) {
    if (e.key === 'F1') {
      e.preventDefault();
      showKeys = !showKeys;
      return;
    }
    const mod = e.ctrlKey || e.metaKey;
    if (!mod || e.altKey) return;
    const k = e.key.toLowerCase();
    const run = (fn: () => unknown) => {
      e.preventDefault();
      fn();
    };
    if (k === 's' && !e.shiftKey) return run(save);
    if (k === 'o' && !e.shiftKey) return run(openDoc);
    if (k === 'n' && !e.shiftKey) return run(newSample);
    if (k === 'p' && !e.shiftKey) return run(savePdf);
    if (k === 'e' && e.shiftKey) return run(exportLatex);
    if (doc && !e.shiftKey && (k === '1' || k === '2' || k === '3')) {
      return run(() => toggle(paneKeys[Number(k) - 1]));
    }
    // In Textfeldern gilt das Rückgängig des Feldes bzw. des Quelltexteditors.
    if ((e.target as HTMLElement).closest('input, textarea, .cm-editor')) return;
    if (k === 'z' && !e.shiftKey) run(undo);
    else if (k === 'y' || (k === 'z' && e.shiftKey)) run(redo);
  }

  // Dateien, die neben einer Bildfläche landen, nicht im Fenster öffnen.
  function blockDrop(e: DragEvent) {
    if (e.defaultPrevented) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'none';
  }
</script>

<svelte:window onkeydown={onKey} ondragover={blockDrop} ondrop={blockDrop} />

<div class="app">
  <header class="bar">
    <div class="brand">
      <span class="mark" aria-hidden="true">f</span>
      <span class="word">formavium</span>
    </div>

    <nav class="group" aria-label="Datei">
      <button onclick={newSample} title="Übungsdokument anlegen (Strg+N)">Neu</button>
      <button onclick={openDoc} title="Strg+O">Öffnen…</button>
      <button onclick={save} disabled={!doc} title="Strg+S">Speichern</button>
    </nav>
    <nav class="group" aria-label="Bearbeiten">
      <button onclick={undo} disabled={!doc?.can_undo} title="Rückgängig (Strg+Z)">↶</button>
      <button onclick={redo} disabled={!doc?.can_redo} title="Wiederholen (Strg+Y)">↷</button>
    </nav>
    <nav class="group" aria-label="Ausgabe">
      <button onclick={savePdf} disabled={!doc || !preview?.shown_revision} title="Strg+P">Als PDF speichern</button>
      <button onclick={exportLatex} disabled={!doc} title="Strg+Umschalt+E">LaTeX exportieren</button>
    </nav>

    <div class="doc-title">
      {#if doc}
        <span class="name">{doc.title}</span>
        {#if doc.dirty}<span class="dirty" title="Ungespeicherte Änderungen">●</span>{/if}
        <span class="path" title={doc.path}>{doc.path}</span>
      {/if}
    </div>

    <div class="views" role="group" aria-label="Ansichten">
      {#each paneKeys as key, i}
        <button
          class:on={layout.show[key]}
          aria-pressed={layout.show[key]}
          title="Strg+{i + 1}"
          onclick={() => toggle(key)}>{paneTitle[key]}</button
        >
      {/each}
    </div>
  </header>

  {#if doc}
    <main class="panes" class:arranging={paneDrag?.active} bind:this={panesEl}>
      {#each visible as key, i (key)}
        {#if i > 0}
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <div
            class="splitter"
            role="separator"
            aria-orientation="vertical"
            title="Ziehen ändert die Breite, Doppelklick gleicht aus"
            onpointerdown={(e) => splitDown(e, i)}
            ondblclick={() => splitReset(i)}
          ></div>
        {/if}
        <section
          class="pane"
          class:moving={paneDrag?.active && paneDrag.key === key}
          class:drop-before={paneDropBefore(i)}
          class:drop-after={paneDropAfter(i)}
          style:flex-grow={layout.size[key]}
          aria-label={paneLabel[key]}
        >
          <header class="tile">
            <button
              class="handle"
              title="Ziehen ordnet die Ansichten um. Klick zeigt die Aktionen."
              aria-label="Ansicht {paneTitle[key]} verschieben oder Aktionen öffnen"
              onpointerdown={(e) => paneDown(e, key)}
              onpointermove={paneMove}
              onpointerup={paneUp}
              onpointercancel={() => (paneDrag = null)}
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
            <span class="tile-title">{paneTitle[key]}</span>
          </header>
          <div class="pane-body">
            {#if key === 'source'}
              <SourceView bind:this={sourceView} {doc} {errorLines} onSourceEdit={sourceEdit} onUndo={undo} onRedo={redo} />
            {:else if key === 'visual'}
              <VisualView {doc} {errorParts} onCommand={command} onImage={setImage} onJump={jump} />
            {:else}
              <PdfView {preview} revision={doc.revision} onDiag={jumpDiag} />
            {/if}
          </div>
        </section>
      {/each}
    </main>
    <footer class="statusbar">
      <span>Revision {doc.revision}</span>
      <span>{doc.parts.length} Abschnitte</span>
      {#if doc.notices.length}<span class="warn">{doc.notices.length} Hinweise</span>{/if}
      <span class="spacer"></span>
      <button class="keys" onclick={() => (showKeys = true)}>Tastenkürzel · F1</button>
      <span>{isTauri ? 'Desktop' : 'Browser · Dev-Bridge'}</span>
    </footer>
  {:else}
    <main class="welcome">
      <div class="card">
        <h1>Dokumente einmal gestalten, beliebig oft ausfüllen.</h1>
        <p>Beginne mit dem Übungsdokument „Montageanleitung“: zwei Materiallisten, freies LaTeX und eine PDF-Vorschau.</p>
        <div class="actions">
          <button class="primary" onclick={newSample}>Übungsdokument anlegen</button>
          <button class="ghost" onclick={openDoc}>Dokument öffnen…</button>
        </div>
      </div>
    </main>
  {/if}

  {#if toastState.current}
    <div class="toast {toastState.current.kind}" role="status">{toastState.current.text}</div>
  {/if}
  <Menu />
  <Shortcuts bind:open={showKeys} />
  <Dialog />
</div>

<style>
  .app {
    height: 100vh;
    display: grid;
    grid-template-rows: auto 1fr auto;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 14px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-right: 6px;
  }
  .mark {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: var(--brand);
    color: white;
    display: grid;
    place-items: center;
    font-weight: 700;
    font-family: var(--serif);
    font-size: 17px;
    font-style: italic;
  }
  .word {
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .group {
    display: flex;
    gap: 2px;
    padding-left: 12px;
    border-left: 1px solid var(--line);
  }
  .bar button {
    font: inherit;
    font-size: 13px;
    padding: 5px 10px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: none;
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }
  .bar button:hover:not(:disabled) {
    background: var(--panel-2);
    border-color: var(--line);
  }
  .bar button:disabled {
    color: var(--muted);
    opacity: 0.5;
    cursor: default;
  }
  .doc-title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    justify-content: center;
    overflow: hidden;
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
  }
  .dirty {
    color: var(--accent);
    font-size: 10px;
  }
  .path {
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--mono);
  }
  .views {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 2px;
  }
  .views button {
    border-radius: 6px;
    color: var(--muted);
  }
  .views button.on {
    background: var(--panel);
    color: var(--text);
    border-color: var(--line);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  /* Ansichten als Kacheln nebeneinander. */
  .panes {
    display: flex;
    min-height: 0;
    min-width: 0;
  }
  .panes.arranging {
    cursor: grabbing;
    user-select: none;
  }
  .pane {
    position: relative;
    flex: 1 1 0;
    min-height: 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--panel);
  }
  .pane.moving {
    opacity: 0.5;
  }
  .pane.drop-before::before,
  .pane.drop-after::after {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    width: 4px;
    background: var(--accent);
    z-index: 5;
  }
  .pane.drop-before::before {
    left: -2px;
  }
  .pane.drop-after::after {
    right: 0;
  }
  .tile {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px 3px 4px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    user-select: none;
  }
  .tile .handle {
    width: 18px;
    height: 22px;
    display: grid;
    place-items: center;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: grab;
    touch-action: none;
  }
  .tile .handle:hover {
    background: var(--line);
    color: var(--text);
  }
  .tile-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .pane-body {
    flex: 1;
    min-height: 0;
  }
  .splitter {
    flex: 0 0 5px;
    margin: 0 -2px;
    z-index: 4;
    cursor: col-resize;
    background: linear-gradient(to right, transparent 2px, var(--line) 2px, var(--line) 3px, transparent 3px);
    touch-action: none;
  }
  .splitter:hover,
  .splitter:active {
    background: var(--accent);
  }

  .statusbar {
    display: flex;
    gap: 16px;
    padding: 4px 14px;
    font-size: 11px;
    color: var(--muted);
    background: var(--panel);
    border-top: 1px solid var(--line);
  }
  .statusbar .spacer {
    flex: 1;
  }
  .statusbar .warn {
    color: var(--warn);
  }
  .statusbar .keys {
    font: inherit;
    color: inherit;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .statusbar .keys:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .welcome {
    display: grid;
    place-items: center;
    background: var(--panel-2);
    padding: 16px;
  }
  .card {
    max-width: 520px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 14px;
    padding: 32px;
  }
  .card h1 {
    font-family: var(--serif);
    font-weight: 600;
    font-size: 26px;
    line-height: 1.25;
    margin: 0 0 12px;
  }
  .card p {
    color: var(--text-2);
    line-height: 1.55;
    margin: 0 0 22px;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .toast {
    position: fixed;
    bottom: 34px;
    left: 50%;
    transform: translateX(-50%);
    max-width: min(640px, calc(100vw - 32px));
    padding: 10px 16px;
    border-radius: 8px;
    background: var(--text);
    color: var(--panel);
    font-size: 13px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
    z-index: 110;
  }
  .toast.error {
    background: var(--danger);
    color: white;
  }
</style>
