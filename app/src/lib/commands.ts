// Befehlsregister: eine Liste für Menüleiste, Befehlspalette und Tastenkürzel.

import { matchesKeys } from './keys';
import type { MenuItem } from './menus.svelte';

export const groups = ['Datei', 'Bearbeiten', 'Einfügen', 'Ansicht', 'Bibliothek', 'Hilfe'] as const;
export type Group = (typeof groups)[number];

export type Cmd = {
  id: string;
  title: string;
  group: Group;
  /** Tastenkürzel in Anzeigeform, z. B. „Strg+Umschalt+E“. */
  keys?: string;
  /** In Textfeldern gilt die Taste dem Feld (Rückgängig, Wiederholen). */
  notInFields?: boolean;
  enabled?: boolean;
  /** Häkchen im Menü für ein- und ausschaltbare Befehle. */
  checked?: boolean;
  /** Trennlinie davor im Menü. */
  divider?: boolean;
  /** Überschrift davor im Menü. */
  heading?: string;
  /** Zusatzangabe in der Palette, etwa ein Pfad. */
  detail?: string;
  /** Nur in der Palette, nicht im Menü. */
  paletteOnly?: boolean;
  run: () => void;
};

export function menuItems(commands: Cmd[], group: Group): MenuItem[] {
  const items: MenuItem[] = [];
  for (const c of commands) {
    if (c.group !== group || c.paletteOnly) continue;
    if (c.divider && items.length) items.push({ separator: true });
    if (c.heading) items.push({ heading: c.heading });
    items.push({
      label: c.title,
      hint: c.keys,
      disabled: c.enabled === false,
      checked: c.checked,
      action: c.run,
    });
  }
  return items;
}

/** Führt den Befehl zum Tastendruck aus. Liefert `true`, wenn einer passte. */
export function runShortcut(e: KeyboardEvent, commands: Cmd[]): boolean {
  const inField = !!(e.target as HTMLElement | null)?.closest?.('input, textarea, .cm-editor');
  for (const c of commands) {
    if (!c.keys || !matchesKeys(e, c.keys)) continue;
    if (c.notInFields && inField) return false;
    e.preventDefault();
    if (c.enabled !== false) c.run();
    return true;
  }
  return false;
}

/** Treffergüte für die Palette: alle Suchwörter müssen vorkommen. */
export function score(query: string, text: string): number {
  const hay = text.toLowerCase();
  let total = 0;
  for (const word of query.toLowerCase().split(/\s+/).filter(Boolean)) {
    const at = hay.indexOf(word);
    if (at < 0) return -1;
    // Treffer am Wortanfang zählen mehr.
    total += at === 0 || /[\s:„(/-]/.test(hay[at - 1]) ? 2 : 1;
  }
  return total;
}
