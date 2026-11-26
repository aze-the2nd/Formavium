// Typen der Bibliothek und der Moduldefinitionen, wie der Kern sie liefert.

import type { ContentKind, Value } from './api';

export type TextRef = { literal: string } | { field: string };

export type ColumnDef = {
  id: string;
  header: TextRef;
  unit: string | null;
  unit_placement: 'header' | 'value';
  align: 'left' | 'center' | 'right';
  /** Feste Breite als Anteil der Zeilenbreite, sonst nach Inhalt. */
  width?: number | null;
};

export type ElementDef = { id: string } & (
  | { kind: 'heading'; level: number; text: TextRef }
  | { kind: 'paragraph'; text: TextRef }
  | { kind: 'table'; columns: ColumnDef[]; rows: string; caption: TextRef | null }
  | { kind: 'image'; path: string; width: number }
  | { kind: 'list'; items: string; ordered: boolean }
  | { kind: 'row'; children: ElementDef[] }
  | { kind: 'module'; module: { id: string; version: number } }
  | { kind: 'latex'; source: string }
);

export type FieldKind = 'text' | 'number' | 'image' | 'table';
export type FieldDef = { id: string; label: string; kind: FieldKind; default: Value | null };

export type ModuleDef = {
  id: string;
  version: number;
  name: string;
  description?: string;
  /** LaTeX für den Dokumentkopf, das das Modul braucht. */
  preamble?: string;
  fields: FieldDef[];
  body: ElementDef[];
};

/** Aufbau in Kurzform, für die Miniaturen der Auswahl. */
export type Outline =
  | { kind: 'heading' | 'paragraph' | 'image' | 'latex' | 'module' | 'list' }
  | { kind: 'table'; cols: number }
  | { kind: 'row'; children: Outline[] };

export type LibModule = {
  id: string;
  version: number;
  name: string;
  description: string;
  kinds: ContentKind[];
  source: 'builtin' | 'user';
  /** Neueste Version dieser Kennung. */
  latest: boolean;
  outline: Outline[];
};

export type LibTemplate = {
  id: string;
  name: string;
  description: string;
  source: 'builtin' | 'user';
  outline: Outline[];
};

export type LibraryView = {
  /** Ordner der eigenen Bibliothek, falls eingerichtet. */
  dir: string | null;
  modules: LibModule[];
  templates: LibTemplate[];
};

export const kindLabel: Record<ContentKind, string> = {
  text: 'Text',
  table: 'Tabelle',
  image: 'Bild',
  latex: 'LaTeX',
};

/** Kennung aus einem Namen, wie der Kern sie bildet. */
export function slug(name: string): string {
  const map: Record<string, string> = { ä: 'ae', ö: 'oe', ü: 'ue', ß: 'ss' };
  const out = name
    .trim()
    .toLowerCase()
    .replace(/[äöüß]/g, (c) => map[c])
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return out || 'unbenannt';
}

/** Neue, im Modul noch freie Kennung mit dem gewünschten Stamm. */
export function freshId(stem: string, used: Iterable<string>): string {
  const taken = new Set(used);
  const base = slug(stem);
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) if (!taken.has(`${base}-${n}`)) return `${base}-${n}`;
}

export function elementIds(body: ElementDef[]): string[] {
  return body.flatMap((e) => (e.kind === 'row' ? [e.id, ...elementIds(e.children)] : [e.id]));
}

/** Kennungen der Felder, die ein LaTeX-Quelltext mit \feld, \rohfeld oder \bildfeld nutzt. */
export function latexFields(source: string): { id: string; image: boolean }[] {
  const out: { id: string; image: boolean }[] = [];
  const re = /\\(feld|rohfeld|bildfeld)(?:\[[^\]]*\])?\{([^{}]*)\}/g;
  for (let m = re.exec(source); m; m = re.exec(source)) out.push({ id: m[2], image: m[1] === 'bildfeld' });
  return out;
}
