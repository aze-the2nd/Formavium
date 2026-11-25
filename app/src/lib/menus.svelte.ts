// Kontextmenü: ein Menü für die ganze Anwendung, geöffnet per Rechtsklick,
// Klick auf einen Griff oder die Menütaste.

import { notify } from './toast.svelte';

export type MenuItem =
  | {
      label: string;
      hint?: string;
      danger?: boolean;
      disabled?: boolean;
      /** Häkchen für ein- und ausschaltbare Einträge. */
      checked?: boolean;
      action: () => void;
    }
  | { separator: true }
  | { heading: string };

type OpenMenu = {
  x: number;
  y: number;
  items: MenuItem[];
  restore: HTMLElement | null;
  /** Kennung des Auslösers, etwa ein Titel der Menüleiste. */
  owner?: string;
};

export const menuState = $state<{ current: OpenMenu | null }>({ current: null });

/** Zuletzt geschlossenes Menü: Ein Klick auf denselben Titel öffnet es nicht sofort wieder. */
export const lastClosed = { owner: undefined as string | undefined, at: 0 };

export function openMenu(at: { x: number; y: number }, items: MenuItem[], owner?: string) {
  const previous = menuState.current?.restore;
  const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // Beim Wechsel zwischen Menüs bleibt das ursprüngliche Fokusziel erhalten.
  const restore = previous && active?.closest('.menu') ? previous : active;
  menuState.current = { x: at.x, y: at.y, items, restore, owner };
}

/** Menü unter einem Element öffnen (Klick auf einen Griff). */
export function openMenuBelow(el: Element, items: MenuItem[], owner?: string) {
  const r = el.getBoundingClientRect();
  openMenu({ x: r.left, y: r.bottom + 4 }, items, owner);
}

export function closeMenu(restoreFocus = true) {
  const m = menuState.current;
  menuState.current = null;
  lastClosed.owner = m?.owner;
  lastClosed.at = performance.now();
  if (restoreFocus && m?.restore?.isConnected) m.restore.focus({ preventScroll: true });
}

/** Zwischenablage für ein Textfeld, weil das eigene Menü das des Browsers ersetzt. */
export function clipboardItems(el: HTMLInputElement | HTMLTextAreaElement): MenuItem[] {
  const hasSelection = el.selectionStart !== el.selectionEnd;
  return [
    {
      label: 'Ausschneiden',
      hint: 'Strg+X',
      disabled: !hasSelection,
      action: () => {
        el.focus();
        document.execCommand('cut');
      },
    },
    {
      label: 'Kopieren',
      hint: 'Strg+C',
      disabled: !hasSelection,
      action: () => {
        el.focus();
        document.execCommand('copy');
      },
    },
    {
      label: 'Einfügen',
      hint: 'Strg+V',
      action: async () => {
        el.focus();
        try {
          const text = await navigator.clipboard.readText();
          el.setRangeText(text, el.selectionStart ?? 0, el.selectionEnd ?? 0, 'end');
          el.dispatchEvent(new Event('input', { bubbles: true }));
        } catch {
          notify('info', 'Einfügen bitte mit Strg+V.');
        }
      },
    },
    { separator: true },
  ];
}
