// Gemeinsamer Zustand des visuellen Editors: Bearbeitungsart der Tabellen,
// die gewählte Stelle (für die Markierung im PDF), Sprungwünsche aus der
// Vorschau und die aktive Zelle für den Zelleditor.

export type Target = { part: string; path?: string; row?: string; column?: string };
export type TableMode = 'inline' | 'grid' | 'inspector';

export type ActiveCell = {
  /** Eindeutig je Tabelle und Zelle, für Aktualisierungen. */
  key: string;
  title: string;
  address: string;
  value: string;
  latex: boolean;
  commit: (value: string) => Promise<unknown>;
  /** Fokus zurück in die Tabelle. */
  back: () => void;
};

function loadMode(): TableMode {
  try {
    const m = localStorage.getItem('formavium.tableMode');
    return m === 'grid' || m === 'inspector' ? m : 'inline';
  } catch {
    return 'inline';
  }
}

export const editor = $state<{
  tableMode: TableMode;
  /** Gewählte Stellen im visuellen Editor. */
  focus: Target[];
  /** Sprungwunsch, etwa nach einem Klick im PDF. */
  request: { target: Target; seq: number } | null;
  cell: ActiveCell | null;
  /** Anfangstext, mit dem der Zelleditor startet (getipptes Zeichen). */
  inspectorSeed: { text: string; seq: number } | null;
}>({ tableMode: loadMode(), focus: [], request: null, cell: null, inspectorSeed: null });

export function setTableMode(mode: TableMode) {
  editor.tableMode = mode;
  try {
    localStorage.setItem('formavium.tableMode', mode);
  } catch {
    /* gilt dann nur für diese Sitzung */
  }
}

let seq = 0;
export function requestFocus(target: Target) {
  editor.request = { target, seq: ++seq };
}

export function seedInspector(text: string) {
  editor.inspectorSeed = { text, seq: ++seq };
}

/** Gleiche Ziele (für Vergleiche ohne erneutes Markieren). */
export function sameTargets(a: Target[], b: Target[]) {
  return a.length === b.length && a.every((t, i) => JSON.stringify(t) === JSON.stringify(b[i]));
}
