// Kontextmenü: ein Menü für die ganze Anwendung, geöffnet per Rechtsklick,
// Klick auf einen Griff oder die Menütaste.

import { notify } from './toast.svelte';

export type MenuItem =
  | { label: string; hint?: string; danger?: boolean; disabled?: boolean; action: () => void }
  | { separator: true }
  | { heading: string };

type OpenMenu = { x: number; y: number; items: MenuItem[]; restore: HTMLElement | null };

export const menuState = $state<{ current: OpenMenu | null }>({ current: null });

export function openMenu(at: { x: number; y: number }, items: MenuItem[]) {
  const restore = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  menuState.current = { x: at.x, y: at.y, items, restore };
}

/** Menü unter einem Element öffnen (Klick auf einen Griff). */
export function openMenuBelow(el: Element, items: MenuItem[]) {
  const r = el.getBoundingClientRect();
  openMenu({ x: r.left, y: r.bottom + 4 }, items);
}

export function closeMenu(restoreFocus = true) {
  const m = menuState.current;
  menuState.current = null;
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
