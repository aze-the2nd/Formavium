<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import SourceView from './lib/SourceView.svelte';
  import VisualView from './lib/VisualView.svelte';
  import PdfView from './lib/PdfView.svelte';
  import Dialog from './lib/Dialog.svelte';
  import Feedback from './lib/Feedback.svelte';
  import Menu from './lib/Menu.svelte';
  import Menubar from './lib/Menubar.svelte';
  import Miniature from './lib/Miniature.svelte';
  import Palette from './lib/Palette.svelte';
  import Picker, { type PickerMode } from './lib/Picker.svelte';
  import Shortcuts from './lib/Shortcuts.svelte';
  import Workshop from './lib/Workshop.svelte';
  import LatexRef from './lib/LatexRef.svelte';
  import Guide from './lib/Guide.svelte';
  import TemplateDialog from './lib/TemplateDialog.svelte';
  import { editor, requestFocus, sameTargets, setTableMode, type Target, type TableMode } from './lib/editor.svelte';
  import type { Mark } from './lib/PdfView.svelte';
  import { runShortcut, type Cmd } from './lib/commands';
  import { confirmDialog, promptDialog } from './lib/dialogs.svelte';
  import { REPO, openInBrowser } from './lib/feedback';
  import { slug, type LibModule, type LibTemplate, type LibraryView, type ModuleDef } from './lib/library';
  import { openMenuBelow, type MenuItem } from './lib/menus.svelte';
  import { clearResources } from './lib/resources';
  import { dismissToast, notify, toastState } from './lib/toast.svelte';
  import { checkForUpdate, installUpdate, startAutoUpdate, updateState } from './lib/updates.svelte';
  import {
    call,
    invokeApp,
    isTauri,
    pickDir,
    pickOpen,
    pickSave,
    toBase64,
    type Command,
    type Diag,
    type Doc,
    type ModulePart,
    type PartView,
    type Preview,
    type SourceEditResult,
  } from './lib/api';

  let doc = $state<Doc | null>(null);
  let preview = $state<Preview | null>(null);
  let sourceView = $state<SourceView>();
  let visualView = $state<VisualView>();
  let pdfView = $state<PdfView>();
  let showRef = $state(false);
  let showGuide = $state(false);
  let showTemplate = $state(false);
  let showKeys = $state(false);
  let showPalette = $state(false);
  let showFeedback = $state(false);

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
  let unlistenClose: (() => void) | undefined;
  onMount(async () => {
    doc = (await guarded(() => call<Doc | null>('state'))) ?? null;
    pollTimer = setInterval(pollPreview, 300);
    pollPreview();
    guardClose();
    loadLibrary();
    startAutoUpdate(() => !doc);
  });
  onDestroy(() => {
    clearInterval(pollTimer);
    unlistenClose?.();
  });

  /** Fenster nicht mit ungespeicherten Änderungen schließen, ohne zu fragen. */
  async function guardClose() {
    if (!isTauri) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
      if (!doc?.dirty) return;
      const discard = await confirmDialog({
        title: 'Ungespeicherte Änderungen',
        message: `„${doc.title}“ wurde geändert und noch nicht gespeichert.`,
        okLabel: 'Verwerfen und schließen',
        cancelLabel: 'Zurück zum Dokument',
        danger: true,
      });
      if (!discard) event.preventDefault();
    });
  }

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

  async function save(): Promise<boolean> {
    if (!doc) return false;
    const d = await enqueue(() => guarded(() => call<Doc>('save')));
    if (!d) return false;
    doc = d;
    notify('info', `Gespeichert: ${d.path}`);
    return true;
  }

  // Zuletzt geöffnete Dokumente (nur auf diesem Rechner gemerkt).
  function loadRecent(): string[] {
    try {
      const list = JSON.parse(localStorage.getItem('formavium.recent') ?? '[]');
      return Array.isArray(list) ? list.filter((p) => typeof p === 'string').slice(0, 8) : [];
    } catch {
      return [];
    }
  }
  let recent = $state(loadRecent());
  function remember(path: string) {
    recent = [path, ...recent.filter((p) => p !== path)].slice(0, 8);
    try {
      localStorage.setItem('formavium.recent', JSON.stringify(recent));
    } catch {
      /* ohne Speicher keine Liste */
    }
  }
  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  function opened(d: Doc) {
    clearResources();
    doc = d;
    preview = null;
    remember(d.path);
  }

  /** Vor dem Wechsel des Dokuments nach ungespeicherten Änderungen fragen. */
  async function mayLeave(): Promise<boolean> {
    if (!doc?.dirty) return true;
    return confirmDialog({
      title: 'Ungespeicherte Änderungen',
      message: `„${doc.title}“ wurde geändert und noch nicht gespeichert.`,
      okLabel: 'Änderungen verwerfen',
      cancelLabel: 'Zurück zum Dokument',
      danger: true,
    });
  }

  async function openPath(path: string) {
    if (!(await mayLeave())) return;
    const d = await guarded(() => call<Doc>('open', { path }));
    if (d) opened(d);
  }

  async function openDoc() {
    const path = await pickOpen('Dokument öffnen', fvm, `${defaultDir}anleitung.fvm`);
    if (path) await openPath(path);
  }

  async function newFromTemplate(t: LibTemplate) {
    if (!(await mayLeave())) return;
    const path = await pickSave(`Neues Dokument aus „${t.name}“ anlegen`, fvm, `${defaultDir}${slug(t.name)}.fvm`);
    if (!path) return;
    const d = await guarded(() => call<Doc>('new_document', { path, template: t.id }));
    if (d) {
      closeWorkshop();
      opened(d);
      // Das Dokument liegt in einem eigenen Ordner mit seinem Namen.
      notify('info', `Angelegt: ${d.path}`);
    }
  }

  /** Dieses Dokument als freies LaTeX weiterführen (Nerd-Modus); das Original bleibt. */
  async function freeCopy() {
    if (!doc || doc.free) return;
    if (!(await mayLeave())) return;
    const ok = await confirmDialog({
      title: 'Als freies LaTeX weiterführen',
      message:
        'Formavium legt eine Kopie als freies LaTeX-Dokument an (Nerd-Modus). Dort schreibst du den Quelltext frei, ohne Module und ohne Schutz. Dieses Dokument bleibt unverändert.',
      okLabel: 'Kopie anlegen …',
    });
    if (!ok) return;
    const path = await pickSave('Freie LaTeX-Kopie speichern', fvm, doc.path.replace(/\.fvm$/, '-latex.fvm'));
    if (!path) return;
    const d = await guarded(() => call<Doc>('to_free_copy', { path }));
    if (d) {
      closeWorkshop();
      opened(d);
      notify('info', `Freie Kopie angelegt: ${d.path}`);
    }
  }

  async function newFree() {
    const t = library?.templates.find((x) => x.id === 'freies-latex');
    if (t) await newFromTemplate(t);
  }

  async function closeDoc() {
    if (!doc || !(await mayLeave())) return;
    await guarded(() => call('close'));
    closeWorkshop();
    doc = null;
    preview = null;
  }

  async function renameDoc() {
    if (!doc) return;
    const title = await promptDialog({ title: 'Titel des Dokuments', value: doc.title });
    if (title?.trim()) await command({ op: 'set_title', title: title.trim() });
  }

  // --- Bibliothek: Module und Vorlagen ---

  let library = $state<LibraryView | null>(null);
  let picker = $state<PickerMode | null>(null);

  async function loadLibrary() {
    library = (await guarded(() => call<LibraryView>('library'))) ?? library;
  }

  function openPicker(mode: PickerMode) {
    picker = mode;
    loadLibrary();
  }

  /** Antwort eines Sitzungsbefehls übernehmen, der das Dokument ändert. */
  function sessionEdit<T>(name: string, args: Record<string, unknown>, take: (result: T) => Doc): Promise<T | undefined> {
    return enqueue(async () => {
      if (!doc) return undefined;
      const result = await guarded(() => call<T>(name, { base: doc!.revision, ...args }));
      if (result) doc = take(result);
      pollPreview();
      return result;
    });
  }

  function pickModule(index?: number) {
    if (!doc) return;
    openPicker({ kind: 'module', index: index ?? visualView?.insertIndex() ?? doc.parts.length });
  }

  async function insertFromLibrary(m: LibModule, index: number) {
    const at = Math.min(index, doc?.parts.length ?? 0);
    const d = await sessionEdit<Doc>('insert_module', { index: at, module: { id: m.id, version: m.version } }, (r) => r);
    if (!d) return;
    layout.show.visual = true;
    visualView?.focusPart(at);
  }

  async function insertLatex() {
    if (!doc) return;
    const index = visualView?.insertIndex() ?? doc.parts.length;
    const d = await command({ op: 'insert_part', index, content: { kind: 'latex', source: 'Neuer Abschnitt.\n' } });
    if (d) visualView?.focusPart(index);
  }

  // Modul-Werkstatt: bearbeitet die Definition eines Moduls dieses Dokuments.
  type ModuleRef = { id: string; version: number };
  let editing = $state<ModuleRef | null>(null);
  let moduleDef = $state<ModuleDef | null>(null);
  let editingKey = $derived(editing ? `${editing.id}@${editing.version}` : null);
  let editingParts = $derived(
    (doc?.parts ?? []).filter((p): p is ModulePart => p.kind === 'module' && p.module === editingKey),
  );

  function closeWorkshop() {
    editing = null;
    moduleDef = null;
  }

  // Definition nach jeder Änderung des Dokuments neu holen (auch nach Rückgängig).
  $effect(() => {
    const ref = editing;
    const revision = doc?.revision;
    if (!ref || revision === undefined) return;
    let alive = true;
    call<ModuleDef>('module_def', { module: ref })
      .then((def) => {
        if (alive) moduleDef = def;
      })
      .catch(() => {
        // Das Modul gibt es in diesem Stand nicht mehr.
        if (alive) closeWorkshop();
      });
    return () => {
      alive = false;
    };
  });

  function editModule(p: ModulePart) {
    editing = { id: p.module_id, version: p.version };
    layout.show.visual = true;
  }

  async function publishModule(ref: ModuleRef, name: string) {
    const r = await sessionEdit<{ doc: Doc; module: ModuleRef }>('publish_module', { module: ref }, (x) => x.doc);
    if (!r) return;
    if (editing?.id === ref.id && editing.version === ref.version) editing = r.module;
    notify('info', `„${name}“ steht als Version ${r.module.version} in der Bibliothek.`);
    loadLibrary();
  }

  async function upgradeModule(p: ModulePart) {
    const d = await sessionEdit<Doc>('upgrade_module', { module: { id: p.module_id, version: p.version } }, (r) => r);
    if (!d) return;
    if (editing?.id === p.module_id) closeWorkshop();
    notify('info', `„${p.name}“ nutzt jetzt Version ${p.status.newer}. Strg+Z stellt den alten Stand wieder her.`);
  }

  async function newModule(index?: number) {
    if (!doc) return;
    const name = await promptDialog({
      title: 'Neues Modul',
      message: 'Das Modul entsteht an dieser Stelle im Dokument. Du baust es in der Werkstatt aus und speicherst es danach in der Bibliothek.',
      value: 'Neues Modul',
    });
    if (!name?.trim()) return;
    const at = index ?? visualView?.insertIndex() ?? doc.parts.length;
    const r = await sessionEdit<{ doc: Doc; module: ModuleRef }>('new_module', { index: at, name }, (x) => x.doc);
    if (!r) return;
    editing = r.module;
    layout.show.visual = true;
    visualView?.focusPart(at);
  }

  async function partToModule(p: PartView) {
    const name = await promptDialog({
      title: 'In Modul umwandeln',
      message: 'Der LaTeX-Abschnitt wird zu einem Modul. In der Werkstatt markierst du danach die Stellen, die Eingabefelder werden sollen.',
      value: 'Neues Modul',
    });
    if (!name?.trim()) return;
    const r = await sessionEdit<{ doc: Doc; module: ModuleRef }>('part_to_module', { part: p.id, name }, (x) => x.doc);
    if (r) editing = r.module;
  }

  async function saveTemplate() {
    if (!doc) return;
    showTemplate = true;
  }

  async function storeTemplate(title: string, description: string, keepValues: boolean) {
    const r = await guarded(() => call<{ id: string }>('save_template', { title, description, keep_values: keepValues }));
    if (r) {
      showTemplate = false;
      notify('info', `Vorlage „${title.trim()}“ gespeichert. Sie steht unter Datei → Neu aus Vorlage.`);
      loadLibrary();
    }
  }

  async function deleteLibraryItem(item: { module: LibModule } | { template: LibTemplate }) {
    const name = 'module' in item ? item.module.name : item.template.name;
    const ok = await confirmDialog({
      title: `„${name}“ aus der Bibliothek löschen?`,
      message: 'Dokumente, die den Baustein schon verwenden, behalten ihren Stand.',
      okLabel: 'Löschen',
      danger: true,
    });
    if (!ok) return;
    if ('module' in item) {
      await guarded(() => call('delete_library_module', { module: { id: item.module.id, version: item.module.version } }));
    } else {
      await guarded(() => call('delete_template', { id: item.template.id }));
    }
    loadLibrary();
  }

  const revealLibrary = () => guarded(() => invokeApp('reveal_library'));

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

  // --- Updates ---

  async function updateNow() {
    if (doc?.dirty) {
      const ok = await confirmDialog({
        title: `Formavium ${updateState.version} installieren`,
        message: 'Die App startet dafür neu. Das Dokument wird vorher gespeichert.',
        okLabel: 'Speichern und aktualisieren',
      });
      if (!ok || !(await save())) return;
    }
    await installUpdate(true);
  }

  async function searchUpdate() {
    if (await checkForUpdate(false)) await updateNow();
  }

  function about() {
    confirmDialog({
      title: `Formavium ${updateState.current ?? '(Entwicklung)'}`,
      message: `Strukturierte, modulbasierte LaTeX-Dokumente. Lokal, offline, ohne Adminrechte.\n\nQuelltext und Lizenz (MIT): github.com/${REPO}`,
      okLabel: 'Zum Repository',
      cancelLabel: 'Schließen',
    }).then((go) => {
      if (go) openInBrowser(`https://github.com/${REPO}`);
    });
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

  const defaultLayout = (): Layout => ({
    order: [...paneKeys],
    show: { source: true, visual: true, pdf: true },
    size: { source: 1, visual: 1, pdf: 1 },
  });

  function loadLayout(): Layout {
    const layout = defaultLayout();
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
  let visible = $derived.by(() => {
    const keys = layout.order.filter((k) => layout.show[k] && !(doc?.free && k === 'visual'));
    // Nerd-Modus: Quelltext und PDF, auch wenn sie ausgeblendet waren.
    return keys.length ? keys : (['source', 'pdf'] as PaneKey[]);
  });
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

  // --- Tabellenbearbeitung ---

  const tableModes: [TableMode, string, string][] = [
    ['inline', 'Direkt', 'Zellen direkt in der Tabelle bearbeiten'],
    ['grid', 'Raster', 'Raster mit Spalten A, B, C, Zeilennummern und Eingabezeile'],
    ['inspector', 'Zelleditor', 'Zelle unten im Zelleditor mit Vorschau bearbeiten'],
  ];

  // --- PDF ↔ Editor: SyncTeX ---

  type SyncData = {
    revision: number;
    current: boolean;
    pages: number;
    lines: ([number, number] | null)[];
    parts: [string, number, number][];
  };
  let syncData = $state<SyncData | null>(null);
  let marks = $state<Mark[]>([]);

  // Neue Vorschau: Zuordnung neu laden.
  $effect(() => {
    const rev = preview?.shown_revision;
    if (!rev || !doc) {
      syncData = null;
      return;
    }
    call<SyncData | null>('sync')
      .then((d) => (syncData = d))
      .catch(() => (syncData = null));
  });

  /** Seitenbereich je Teil, für Seitenmarken im visuellen Editor. */
  let partPages = $derived.by(() => {
    if (!syncData) return null;
    const out: Record<string, { range: [number, number]; breaks: [number, number][] }> = {};
    for (const [id, l0, l1] of syncData.parts) {
      let min = Infinity;
      let max = 0;
      const breaks: [number, number][] = [];
      for (let l = l0; l <= l1; l++) {
        const at = syncData.lines[l];
        if (!at) continue;
        // Seitenwechsel innerhalb des Teils: ungefähre Lage als Anteil.
        if (max && at[0] > max) breaks.push([at[0], (l - l0) / Math.max(1, l1 - l0)]);
        min = Math.min(min, at[0]);
        max = Math.max(max, at[0]);
      }
      if (max) out[id] = { range: [min, max], breaks };
    }
    return out;
  });

  // Gewählte Stelle im Editor: im PDF markieren.
  let markTimer: ReturnType<typeof setTimeout>;
  let lastTargets: Target[] = [];
  $effect(() => {
    const targets = editor.focus;
    void syncData;
    clearTimeout(markTimer);
    markTimer = setTimeout(async () => {
      if (!targets.length) {
        marks = [];
        return;
      }
      const changed = !sameTargets(targets, lastTargets);
      lastTargets = targets;
      marks = (await call<Mark[]>('highlight', { targets }).catch(() => [])) ?? [];
      if (changed && !syncScroll) pdfView?.revealMarks();
    }, 120);
  });

  // Für Tests im Browser während der Entwicklung.
  if (import.meta.env.DEV) (window as unknown as Record<string, unknown>).__formavium = { locate: (p: number, x: number, y: number) => locate(p, x, y) };

  /** Klick im PDF: zum Feld oder zur Zelle im Editor springen. */
  async function locate(page: number, x: number, y: number) {
    const hit = await call<{
      kind: string;
      part?: string;
      path?: string;
      row?: string;
      column?: string;
      line: number;
      current: boolean;
    } | null>('locate', { page, x, y }).catch(() => null);
    if (!hit) return;
    const visualShown = visible.includes('visual');
    if (hit.part && visualShown) {
      requestFocus({ part: hit.part, path: hit.path, row: hit.row, column: hit.column });
    } else if (visible.includes('source') && hit.current) {
      sourceView?.jumpToLine(hit.line);
    } else if (!doc?.free && hit.part) {
      layout.show.visual = true;
      requestFocus({ part: hit.part, path: hit.path, row: hit.row, column: hit.column });
    } else {
      layout.show.source = true;
      queueMicrotask(() => sourceView?.jumpToLine(hit.line));
    }
  }

  // --- Synchrones Scrollen ---

  function loadSync() {
    try {
      return localStorage.getItem('formavium.syncScroll') !== 'false';
    } catch {
      return true;
    }
  }
  let syncScroll = $state(loadSync());
  function setSyncScroll(on: boolean) {
    syncScroll = on;
    try {
      localStorage.setItem('formavium.syncScroll', String(on));
    } catch {
      /* nur für diese Sitzung */
    }
  }

  let driver: PaneKey | null = null;
  let driverAt = 0;
  function interact(key: PaneKey) {
    driver = key;
    driverAt = performance.now();
  }
  let syncTimer: ReturnType<typeof setTimeout> | undefined;
  function scrolled(key: PaneKey) {
    if (!syncScroll || driver !== key || performance.now() - driverAt > 1500) return;
    // Höchstens einmal je Bildaufbau nachziehen.
    if (syncTimer) return;
    syncTimer = setTimeout(() => {
      syncTimer = undefined;
      follow(key);
    }, 16);
  }

  /** Zeile des erzeugten Quelltexts am oberen Rand der Ansicht. */
  function lineOf(key: PaneKey): number | null {
    if (!doc) return null;
    if (key === 'source') return sourceView?.topLine() ?? null;
    if (key === 'visual') {
      const pos = visualView?.topPosition();
      if (!pos) return null;
      const i = doc.parts.findIndex((p) => p.id === pos.part);
      if (i < 0) return null;
      const start = doc.parts[i].line;
      const end = doc.parts[i + 1]?.line ?? start + 1;
      return start + pos.fraction * (end - start);
    }
    const pt = pdfView?.topPoint();
    if (!pt || !syncData) return null;
    let best: number | null = null;
    let bestAt: [number, number] | null = null;
    syncData.lines.forEach((at, line) => {
      if (!at) return;
      const before = at[0] < pt.page || (at[0] === pt.page && at[1] <= pt.y);
      if (before && (!bestAt || at[0] > bestAt[0] || (at[0] === bestAt[0] && at[1] >= bestAt[1]))) {
        best = line;
        bestAt = at;
      }
    });
    return best;
  }

  /** Stelle im PDF zu einer (gebrochenen) Zeile. */
  function pointOf(line: number): { page: number; y: number } | null {
    if (!syncData) return null;
    const L = syncData.lines;
    const base = Math.floor(line);
    let prev = base;
    while (prev > 0 && !L[prev]) prev--;
    let next = base + 1;
    while (next < L.length && !L[next]) next++;
    const a = L[prev];
    const b = L[next];
    if (!a) return b ? { page: b[0], y: b[1] } : null;
    if (!b || b[0] !== a[0]) return { page: a[0], y: a[1] };
    const f = (line - prev) / Math.max(1, next - prev);
    return { page: a[0], y: a[1] + f * (b[1] - a[1]) };
  }

  function follow(from: PaneKey) {
    const line = lineOf(from);
    if (line === null || !doc) return;
    for (const key of visible) {
      if (key === from) continue;
      if (key === 'source') sourceView?.scrollToLine(line);
      else if (key === 'visual') {
        const parts = doc.parts;
        let i = parts.length - 1;
        while (i > 0 && parts[i].line > line) i--;
        if (i < 0) continue;
        const start = parts[i].line;
        const end = parts[i + 1]?.line ?? start + 1;
        visualView?.scrollToPart(parts[i].id, Math.max(0, Math.min(1, (line - start) / Math.max(1, end - start))));
      } else {
        const pt = pointOf(line);
        if (pt) pdfView?.scrollToPoint(pt.page, pt.y);
      }
    }
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

  // --- Befehle: eine Liste für Menüleiste, Palette und Tastenkürzel ---

  let commands = $derived.by((): Cmd[] => {
    const has = !!doc;
    return [
      { id: 'new', title: 'Neu aus Vorlage …', group: 'Datei', keys: 'Strg+N', run: () => openPicker({ kind: 'template' }) },
      { id: 'open', title: 'Öffnen …', group: 'Datei', keys: 'Strg+O', run: openDoc },
      ...recent.slice(0, 5).map(
        (path, i): Cmd => ({
          id: `recent:${path}`,
          title: fileName(path),
          detail: path,
          group: 'Datei',
          heading: i === 0 ? 'Zuletzt geöffnet' : undefined,
          enabled: path !== doc?.path,
          run: () => openPath(path),
        }),
      ),
      { id: 'save', title: 'Speichern', group: 'Datei', keys: 'Strg+S', divider: true, enabled: has, run: save },
      { id: 'pdf', title: 'Als PDF speichern …', group: 'Datei', keys: 'Strg+P', divider: true, enabled: has && !!preview?.shown_revision, run: savePdf },
      { id: 'export', title: 'LaTeX exportieren …', group: 'Datei', keys: 'Strg+Umschalt+E', enabled: has, run: exportLatex },
      { id: 'template', title: 'Als Vorlage speichern …', group: 'Datei', divider: true, enabled: has, run: saveTemplate },
      { id: 'free-copy', title: 'Als freies LaTeX weiterführen …', group: 'Datei', enabled: has && !doc?.free, run: freeCopy },
      { id: 'free-new', title: 'Neues freies LaTeX-Dokument (Nerd-Modus) …', group: 'Datei', paletteOnly: true, run: newFree },
      { id: 'rename', title: 'Titel ändern …', group: 'Datei', enabled: has, run: renameDoc },
      { id: 'close', title: 'Dokument schließen', group: 'Datei', keys: 'Strg+W', divider: true, enabled: has, run: closeDoc },

      { id: 'insert-module', title: 'Modul aus der Bibliothek …', group: 'Einfügen', keys: 'Strg+M', enabled: has && !doc?.free, run: () => pickModule() },
      { id: 'insert-latex', title: 'LaTeX-Abschnitt', group: 'Einfügen', enabled: has && !doc?.free, run: insertLatex },
      { id: 'new-module', title: 'Neues Modul erstellen …', group: 'Einfügen', divider: true, enabled: has && !doc?.free, run: () => newModule() },
      { id: 'latex-ref', title: 'LaTeX-Befehle nachschlagen …', group: 'Einfügen', keys: 'Strg+Umschalt+L', divider: true, run: () => (showRef = true) },
      ...(library?.modules ?? [])
        .filter((m) => m.latest)
        .map(
          (m): Cmd => ({
            id: `module:${m.id}`,
            title: `Modul einfügen: ${m.name}`,
            detail: m.description,
            group: 'Einfügen',
            paletteOnly: true,
            enabled: has,
            run: () => insertFromLibrary(m, visualView?.insertIndex() ?? doc?.parts.length ?? 0),
          }),
        ),
      ...(library?.templates ?? []).map(
        (t): Cmd => ({
          id: `template:${t.id}`,
          title: `Neu aus Vorlage: ${t.name}`,
          detail: t.description,
          group: 'Datei',
          paletteOnly: true,
          run: () => newFromTemplate(t),
        }),
      ),

      { id: 'library', title: 'Module und Vorlagen …', group: 'Bibliothek', run: () => openPicker({ kind: 'manage' }) },
      { id: 'library-publish', title: 'Bearbeitetes Modul in Bibliothek speichern', group: 'Bibliothek', enabled: !!editing && !!editingParts[0]?.status.local, run: () => editing && moduleDef && publishModule(editing, moduleDef.name) },
      { id: 'library-dir', title: 'Bibliotheksordner öffnen', group: 'Bibliothek', divider: true, enabled: isTauri, run: revealLibrary },

      { id: 'undo', title: 'Rückgängig', group: 'Bearbeiten', keys: 'Strg+Z', notInFields: true, enabled: !!doc?.can_undo, run: undo },
      { id: 'redo', title: 'Wiederholen', group: 'Bearbeiten', keys: 'Strg+Y', notInFields: true, enabled: !!doc?.can_redo, run: redo },
      { id: 'redo2', title: 'Wiederholen', group: 'Bearbeiten', keys: 'Strg+Umschalt+Z', notInFields: true, paletteOnly: true, enabled: false, run: redo },

      ...paneKeys.map(
        (key, i): Cmd => ({
          id: `view:${key}`,
          title: paneTitle[key],
          group: 'Ansicht',
          keys: `Strg+${i + 1}`,
          checked: layout.show[key],
          enabled: has,
          run: () => toggle(key),
        }),
      ),
      { id: 'layout-reset', title: 'Anordnung zurücksetzen', group: 'Ansicht', divider: true, run: () => (layout = defaultLayout()) },
      { id: 'sync-scroll', title: 'Ansichten synchron scrollen', group: 'Ansicht', checked: syncScroll, run: () => setSyncScroll(!syncScroll) },
      ...tableModes.map(
        ([mode, label], i): Cmd => ({
          id: `table-mode:${mode}`,
          title: `Tabellen: ${label}`,
          group: 'Ansicht',
          heading: i === 0 ? 'Tabellen bearbeiten' : undefined,
          divider: i === 0,
          checked: editor.tableMode === mode,
          run: () => setTableMode(mode),
        }),
      ),
      { id: 'palette', title: 'Befehlspalette', group: 'Ansicht', keys: 'Strg+K', divider: true, run: () => (showPalette = true) },

      { id: 'guide', title: 'Erste Schritte und Anleitungen', group: 'Hilfe', run: () => (showGuide = true) },
      { id: 'keys', title: 'Tastenkürzel', group: 'Hilfe', keys: 'F1', run: () => (showKeys = !showKeys) },
      { id: 'ref', title: 'LaTeX-Befehle', group: 'Hilfe', run: () => (showRef = true) },
      { id: 'feedback', title: 'Feedback senden …', group: 'Hilfe', keys: 'Strg+Umschalt+F', run: () => (showFeedback = true) },
      { id: 'update', title: 'Nach Updates suchen', group: 'Hilfe', divider: true, run: searchUpdate },
      { id: 'about', title: 'Über Formavium', group: 'Hilfe', run: about },
    ];
  });

  function onKey(e: KeyboardEvent) {
    // Strg+Umschalt+Z ist ein zweites Kürzel für Wiederholen.
    const all = commands.map((c) => (c.id === 'redo2' ? { ...c, enabled: !!doc?.can_redo } : c));
    runShortcut(e, all);
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

    <Menubar {commands} />

    <div class="doc-title">
      {#if doc}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span class="name" title="Doppelklick ändert den Titel" ondblclick={renameDoc}>{doc.title}</span>
        {#if doc.free}<span class="nerd" title="Freies LaTeX-Dokument: alles im Quelltext, ohne Module">Nerd-Modus</span>{/if}
        {#if doc.dirty}<span class="dirty" title="Ungespeicherte Änderungen">●</span>{/if}
        <span class="path" title={doc.path}>{doc.path}</span>
      {/if}
    </div>

    <button class="search" title="Befehlspalette (Strg+K)" onclick={() => (showPalette = true)}>
      <span>Befehle suchen</span><kbd>Strg+K</kbd>
    </button>

    <div class="views" role="group" aria-label="Ansichten">
      {#each paneKeys as key, i}
        <button
          class:on={layout.show[key]}
          aria-pressed={layout.show[key]}
          title="Strg+{i + 1}"
          disabled={!doc || (doc.free && key === 'visual')}
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
            {#if key === 'visual'}
              <span class="tile-spacer"></span>
              <span class="modes" role="radiogroup" aria-label="Tabellen bearbeiten">
                {#each tableModes as [mode, label, hint]}
                  <button
                    role="radio"
                    aria-checked={editor.tableMode === mode}
                    class:on={editor.tableMode === mode}
                    title={hint}
                    onclick={() => setTableMode(mode)}>{label}</button
                  >
                {/each}
              </span>
            {:else if key === 'pdf' || key === 'source'}
              <span class="tile-spacer"></span>
              <button
                class="sync"
                class:on={syncScroll}
                title="Ansichten synchron scrollen"
                aria-pressed={syncScroll}
                onclick={() => setSyncScroll(!syncScroll)}>⇅ synchron</button
              >
            {/if}
          </header>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="pane-body"
            onwheelcapture={() => interact(key)}
            onpointerdowncapture={() => interact(key)}
            onkeydowncapture={() => interact(key)}
          >
            {#if key === 'source'}
              <SourceView
                bind:this={sourceView}
                {doc}
                {errorLines}
                onSourceEdit={sourceEdit}
                onUndo={undo}
                onRedo={redo}
                onScroll={() => scrolled('source')}
              />
            {:else if key === 'visual'}
              <VisualView
                bind:this={visualView}
                {doc}
                {errorParts}
                editing={editingKey}
                onCommand={command}
                onImage={setImage}
                onJump={jump}
                onPick={pickModule}
                onEditModule={editModule}
                onPublish={(p) => publishModule({ id: p.module_id, version: p.version }, p.name)}
                onUpgrade={upgradeModule}
                onToModule={partToModule}
                pages={partPages}
                onScroll={() => scrolled('visual')}
              />
            {:else}
              <PdfView
                bind:this={pdfView}
                {preview}
                revision={doc.revision}
                {marks}
                onDiag={jumpDiag}
                onLocate={locate}
                onScroll={() => scrolled('pdf')}
              />
            {/if}
          </div>
        </section>
      {/each}
      {#if editing && moduleDef}
        <section class="pane workshop" aria-label="Modul-Werkstatt">
          <Workshop
            def={moduleDef}
            status={editingParts[0]?.status ?? null}
            uses={editingParts.length}
            canPublish={!!library?.dir}
            onUpdate={(def) => command({ op: 'update_module', def })}
            onPublish={() => editing && moduleDef && publishModule(editing, moduleDef.name)}
            onClose={closeWorkshop}
          />
        </section>
      {/if}
    </main>
  {:else}
    <main class="welcome">
      <div class="card">
        <h1>Dokumente einmal gestalten, beliebig oft ausfüllen.</h1>
        <p>Wähle eine Vorlage oder öffne ein Dokument. Mit Strg+K erreichst du jeden Befehl über die Tastatur.</p>
        {#if library?.templates.length}
          <div class="templates">
            {#each library.templates.slice(0, 6) as t (t.id)}
              <button class="template" onclick={() => newFromTemplate(t)} title={t.description}>
                <Miniature outline={t.outline} />
                <span class="file">{t.name}</span>
              </button>
            {/each}
          </div>
        {/if}
        <div class="actions">
          <button class="primary" onclick={() => openPicker({ kind: 'template' })}>Alle Vorlagen …</button>
          <button class="ghost" onclick={openDoc}>Dokument öffnen …</button>
        </div>
        {#if recent.length}
          <h2>Zuletzt geöffnet</h2>
          <ul class="recent">
            {#each recent.slice(0, 5) as path}
              <li>
                <button onclick={() => openPath(path)}>
                  <span class="file">{fileName(path)}</span>
                  <span class="dir">{path}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </main>
  {/if}

  <footer class="statusbar">
    {#if doc}
      <span>Revision {doc.revision}</span>
      <span>{doc.free ? 'Freies LaTeX' : `${doc.parts.length} Abschnitte`}</span>
      {#if doc.notices.length}<span class="warn">{doc.notices.length} Hinweise</span>{/if}
    {/if}
    <span class="spacer"></span>
    {#if updateState.phase === 'pending'}
      <button class="chip" onclick={updateNow} title="Die App startet dafür neu">
        Update {updateState.version} installieren
      </button>
    {:else if updateState.phase === 'downloading' || updateState.phase === 'installing'}
      <span>Update {updateState.version} {Math.round((updateState.progress ?? 0) * 100)} %</span>
    {/if}
    <button class="quiet" onclick={() => (showFeedback = true)} title="Fehler melden oder Idee einreichen (Strg+Umschalt+F)">
      Feedback
    </button>
    <button class="quiet" onclick={() => (showKeys = true)}>Tastenkürzel · F1</button>
    <span>{isTauri ? `Version ${updateState.current ?? ''}` : 'Browser · Dev-Bridge'}</span>
  </footer>

  {#if toastState.current}
    {@const t = toastState.current}
    <div class="toast {t.kind}" role="status">
      <span>{t.text}</span>
      {#if t.action}
        <button
          onclick={() => {
            t.action?.run();
            dismissToast();
          }}>{t.action.label}</button
        >
      {/if}
    </div>
  {/if}

  {#if updateState.blocking && (updateState.phase === 'downloading' || updateState.phase === 'installing')}
    <div class="updating" role="alertdialog" aria-modal="true" aria-label="Update wird installiert">
      <div class="update-card">
        <h2>Formavium {updateState.version} wird installiert</h2>
        <p>
          {updateState.phase === 'installing'
            ? 'Die App startet gleich neu.'
            : 'Die neue Version wird geladen. Danach startet die App neu.'}
        </p>
        <div class="meter" class:busy={updateState.progress === null || updateState.phase === 'installing'}>
          <div style:width="{Math.round((updateState.progress ?? 0) * 100)}%"></div>
        </div>
      </div>
    </div>
  {/if}

  <Picker
    bind:mode={picker}
    {library}
    onModule={insertFromLibrary}
    onTemplate={newFromTemplate}
    onNewModule={newModule}
    onDelete={deleteLibraryItem}
    onRevealDir={isTauri ? revealLibrary : undefined}
  />
  <Menu />
  <LatexRef bind:open={showRef} source={doc?.source ?? ''} />
  <Guide
    bind:open={showGuide}
    hasDoc={!!doc}
    onAction={(a) => {
      showGuide = false;
      if (a === 'template') openPicker({ kind: 'template' });
      else if (a === 'module') newModule();
      else if (a === 'save-template') saveTemplate();
      else if (a === 'free') newFree();
      else if (a === 'library') openPicker({ kind: 'manage' });
      else if (a === 'ref') showRef = true;
    }}
  />
  <TemplateDialog bind:open={showTemplate} title={doc?.title ?? ''} dir={library?.dir ?? null} onSave={storeTemplate} />
  <Palette bind:open={showPalette} {commands} />
  <Shortcuts bind:open={showKeys} />
  <Feedback bind:open={showFeedback} version={updateState.current} views={visible.map((k) => paneTitle[k])} />
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
    gap: 12px;
    padding: 6px 14px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-right: 2px;
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
  .search {
    display: flex;
    align-items: center;
    gap: 14px;
    font: inherit;
    font-size: 12px;
    color: var(--muted);
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 5px 8px 5px 10px;
    cursor: pointer;
    white-space: nowrap;
  }
  .search:hover {
    border-color: var(--line-strong);
    color: var(--text);
  }
  .search kbd {
    font: inherit;
    font-size: 11px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
  }
  .views {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 2px;
  }
  .views button {
    font: inherit;
    font-size: 13px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: none;
    color: var(--muted);
    cursor: pointer;
    white-space: nowrap;
  }
  .views button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .views button.on:not(:disabled) {
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
    cursor: var(--cur-grabbing);
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
  /* Die Werkstatt steht fest am rechten Rand. */
  .pane.workshop {
    flex: 0 0 clamp(300px, 30vw, 440px);
    border-left: 1px solid var(--line-strong);
  }
  .templates {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 10px;
    margin-bottom: 18px;
  }
  .template {
    display: grid;
    gap: 6px;
    font: inherit;
    text-align: left;
    color: var(--text);
    background: none;
    border: 1px solid transparent;
    border-radius: 10px;
    padding: 6px;
    cursor: pointer;
  }
  .template:hover {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .template .file {
    font-size: 13px;
    font-weight: 600;
    padding: 0 2px;
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
    cursor: var(--cur-grab);
    touch-action: none;
  }
  .tile .handle:hover {
    background: var(--line);
    color: var(--text);
  }
  .tile-spacer {
    flex: 1;
  }
  .modes {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 1px;
  }
  .modes button,
  .sync {
    font: inherit;
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
    white-space: nowrap;
  }
  .modes button:hover,
  .sync:hover {
    color: var(--text);
  }
  .modes button.on {
    background: var(--panel);
    border-color: var(--line);
    color: var(--text);
  }
  .sync.on {
    color: var(--accent-strong);
  }
  .nerd {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--accent-strong);
    background: var(--accent-soft);
    border-radius: 4px;
    padding: 1px 6px;
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
    cursor: var(--cur-col-resize);
    background: linear-gradient(to right, transparent 2px, var(--line) 2px, var(--line) 3px, transparent 3px);
    touch-action: none;
  }
  .splitter:hover,
  .splitter:active {
    background: var(--accent);
  }

  .statusbar {
    display: flex;
    align-items: center;
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
  .statusbar .quiet {
    font: inherit;
    color: inherit;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  .statusbar .quiet:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .statusbar .chip {
    font: inherit;
    font-weight: 600;
    color: white;
    background: var(--accent);
    border: none;
    border-radius: 10px;
    padding: 1px 10px;
    cursor: pointer;
  }
  .welcome {
    display: grid;
    place-items: center;
    background: var(--panel-2);
    padding: 16px;
    overflow: auto;
  }
  .card {
    width: min(720px, 100%);
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
  .card h2 {
    margin: 26px 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .recent {
    list-style: none;
    margin: 0 -8px;
    padding: 0;
  }
  .recent button {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    font: inherit;
    text-align: left;
    color: var(--text);
    background: none;
    border: none;
    border-radius: 7px;
    padding: 6px 8px;
    cursor: pointer;
  }
  .recent button:hover {
    background: var(--panel-2);
  }
  .recent .file {
    font-weight: 600;
    white-space: nowrap;
  }
  .recent .dir {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toast {
    position: fixed;
    bottom: 34px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 14px;
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
  .toast button {
    font: inherit;
    font-weight: 600;
    color: inherit;
    background: none;
    border: 1px solid currentColor;
    border-radius: 6px;
    padding: 2px 10px;
    cursor: pointer;
    white-space: nowrap;
  }
  .updating {
    position: fixed;
    inset: 0;
    z-index: 120;
    display: grid;
    place-items: center;
    background: rgba(15, 20, 30, 0.45);
  }
  .update-card {
    width: min(420px, calc(100vw - 32px));
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 22px 24px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }
  .update-card h2 {
    margin: 0 0 6px;
    font-size: 16px;
  }
  .update-card p {
    margin: 0 0 14px;
    color: var(--text-2);
    font-size: 13px;
  }
  .meter {
    height: 6px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .meter div {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .meter.busy div {
    width: 40% !important;
    animation: slide 1.1s ease-in-out infinite alternate;
  }
  @keyframes slide {
    from {
      transform: translateX(-20%);
    }
    to {
      transform: translateX(170%);
    }
  }
</style>
