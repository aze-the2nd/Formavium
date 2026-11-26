// Zugriff auf die Rust-Sitzung. In der Desktop-App über Tauri-IPC, im
// Browser (Entwicklung im Container) über die Dev-Bridge.

import { promptDialog } from './dialogs.svelte';

export type ContentKind = 'text' | 'table' | 'image' | 'latex';

export type Col = {
  id: string;
  header: string;
  unit: string | null;
  unit_placement: 'header' | 'value';
  align: 'left' | 'center' | 'right';
  width: number | null;
  /** Zellen enthalten LaTeX. */
  latex: boolean;
};
/** `merged[i]`: Zelle der Spalte i ist mit der darüber verbunden. */
export type RowView = { id: string; cells: string[]; merged: boolean[] };

/** Eingabefeld innerhalb eines LaTeX-Elements. */
export type LatexField = {
  path: string;
  label: string;
  kind: 'text' | 'raw' | 'image';
  value: string;
  numeric: boolean;
};

export type El =
  | { kind: 'heading'; level: number; text: string; field: string | null; numeric: boolean }
  | { kind: 'paragraph'; text: string; field: string | null; numeric: boolean }
  | { kind: 'table'; path: string; rows_path: string; caption: string | null; columns: Col[]; rows: RowView[]; fixed: boolean }
  | { kind: 'image'; field: string; value: string | null; width: number }
  | { kind: 'list'; path: string; ordered: boolean; items: { id: string; text: string }[] }
  | { kind: 'row'; children: El[] }
  | { kind: 'module'; name: string; children: El[] }
  | { kind: 'latex'; source: string; fields: LatexField[] };

export type ModulePart = {
  kind: 'module';
  id: string;
  offset: number;
  line: number;
  name: string;
  module: string;
  module_id: string;
  version: number;
  description: string;
  required: boolean;
  /** Verhältnis zur Bibliothek: lokal geändert, neuere Version vorhanden. */
  status: { local: boolean; newer: number | null };
  body: El[];
};

export type PartView = { kind: 'latex'; id: string; offset: number; line: number; source: string } | ModulePart;

export type Doc = {
  path: string;
  title: string;
  /** Freies LaTeX-Dokument (Nerd-Modus). */
  free: boolean;
  revision: number;
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
  source: string;
  parts: PartView[];
  notices: { message: string; place: string; part: string | null }[];
  modules: { id: string; version: number; name: string; kinds: ContentKind[] }[];
};

export type Diag = {
  severity: 'error' | 'warning';
  message: string;
  line: number | null;
  place: string | null;
  part: string | null;
};

export type Preview = {
  status: 'empty' | 'current' | 'stale' | 'failed';
  shown_revision: number | null;
  failed_revision: number | null;
  pending: boolean;
  sha256: string | null;
  diagnostics: Diag[];
  diagnostics_current: boolean;
  duration_ms: number | null;
  warnings: number;
};

export type SourceEditResult =
  | { outcome: 'applied'; doc: Doc; converted?: boolean }
  | { outcome: 'stale' | 'rejected'; message: string; doc: Doc }
  | { outcome: 'needs_conversion'; message: string; modules: string[] };

// Rohe Werte und Befehle, wie der Kern sie erwartet.
export type Value =
  | { type: 'text'; value: string }
  | { type: 'number'; value: string }
  | { type: 'image'; value: string }
  | { type: 'table'; value: { id: string; cells: Record<string, string> }[] };

export type Command =
  | { op: 'set_latex'; part: string; source: string }
  | { op: 'set_value'; part: string; path: string; value: Value }
  | { op: 'clear_value'; part: string; path: string }
  | { op: 'move_column'; part: string; table: string; from: number; to: number }
  | { op: 'set_cell'; part: string; path: string; row: string; column: string; value: string }
  | { op: 'insert_row'; part: string; path: string; index: number }
  | { op: 'remove_row'; part: string; path: string; row: string }
  | { op: 'move_row'; part: string; path: string; row: string; to: number }
  | { op: 'paste_cells'; part: string; table: string; row: number; column: number; cells: string[][] }
  | { op: 'insert_column'; part: string; table: string; index: number; header: string }
  | { op: 'remove_column'; part: string; table: string; column: string }
  | { op: 'set_column_header'; part: string; table: string; column: string; header: string }
  | { op: 'insert_part'; index: number; content: unknown }
  | { op: 'remove_part'; part: string }
  | { op: 'move_part'; part: string; to: number }
  | { op: 'merge_cells'; part: string; table: string; rows: string[]; columns: string[]; merge: boolean }
  | { op: 'set_column_style'; part: string; table: string; column: string; align?: Col['align']; latex?: boolean }
  | { op: 'duplicate_part'; part: string }
  | { op: 'insert_module'; index: number; def: unknown; requires: unknown[] }
  | { op: 'update_module'; def: unknown }
  | { op: 'set_title'; title: string };

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>('call', { name, args });
  }
  const res = await fetch('/api/call', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ name, args }),
  });
  if (!res.ok) throw new Error(`Dev-Bridge antwortet mit ${res.status}`);
  const body = await res.json();
  if ('error' in body) throw new Error(body.error);
  return body.ok as T;
}

export async function fetchPdf(): Promise<Uint8Array | null> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    const buf = await invoke<ArrayBuffer | null>('pdf');
    return buf ? new Uint8Array(buf) : null;
  }
  const res = await fetch('/api/pdf', { cache: 'no-store' });
  if (!res.ok) return null;
  return new Uint8Array(await res.arrayBuffer());
}

/** Bild neben der Arbeitsdatei (relativer Pfad aus dem Dokument). */
export async function fetchResource(path: string): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    if (isTauri) {
      const { invoke } = await import('@tauri-apps/api/core');
      return new Uint8Array(await invoke<ArrayBuffer>('resource', { path }));
    }
    const res = await fetch(`/api/resource?path=${encodeURIComponent(path)}`, { cache: 'no-store' });
    return res.ok ? new Uint8Array(await res.arrayBuffer()) : null;
  } catch {
    return null;
  }
}

/** Dateiinhalt für die Übergabe an den Kern. */
export async function toBase64(file: Blob): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  let text = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}

// --- Dateiauswahl: nativ in Tauri, Pfadeingabe im Browser ---

type Filter = { name: string; extensions: string[] };

const containerHint = 'Pfad im Dev-Container (das Projektverzeichnis liegt unter /work).';

export async function pickOpen(title: string, filters: Filter[], fallback: string): Promise<string | null> {
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const r = await open({ title, filters, multiple: false, directory: false });
    return typeof r === 'string' ? r : null;
  }
  return promptDialog({ title, message: containerHint, value: fallback });
}

export async function pickSave(title: string, filters: Filter[], fallback: string): Promise<string | null> {
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    return save({ title, filters, defaultPath: fallback.split('/').pop() });
  }
  return promptDialog({ title, message: containerHint, value: fallback });
}

export async function pickDir(title: string, fallback: string): Promise<string | null> {
  if (isTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const r = await open({ title, directory: true, multiple: false });
    return typeof r === 'string' ? r : null;
  }
  return promptDialog({ title, message: containerHint, value: fallback });
}

/** Kleinster zusammenhängender Unterschied zweier Texte (UTF-16-Positionen). */
export function diff(a: string, b: string): { from: number; to: number; insert: string } | null {
  if (a === b) return null;
  let start = 0;
  const min = Math.min(a.length, b.length);
  while (start < min && a.charCodeAt(start) === b.charCodeAt(start)) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a.charCodeAt(endA - 1) === b.charCodeAt(endB - 1)) {
    endA--;
    endB--;
  }
  // Ersatzpaare nicht zerteilen.
  const isLow = (s: string, i: number) => i > 0 && i < s.length && (s.charCodeAt(i) & 0xfc00) === 0xdc00;
  if (isLow(a, start) || isLow(b, start)) start--;
  if (isLow(a, endA) || isLow(b, endB)) {
    endA++;
    endB++;
  }
  return { from: start, to: endA, insert: b.slice(start, endB) };
}

/** Befehl der Desktop-Hülle außerhalb der Dokumentsitzung (nur in der App). */
export async function invokeApp<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
}
