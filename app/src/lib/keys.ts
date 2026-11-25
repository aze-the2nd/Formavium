// Tastenkürzel in deutscher Schreibweise („Strg+Umschalt+E“) erkennen.

const names: Record<string, string> = {
  eingabe: 'enter',
  rück: 'backspace',
  entf: 'delete',
  esc: 'escape',
  '↑': 'arrowup',
  '↓': 'arrowdown',
  '←': 'arrowleft',
  '→': 'arrowright',
};

export function matchesKeys(e: KeyboardEvent, keys: string): boolean {
  const parts = keys.toLowerCase().split('+');
  const key = parts.pop() ?? '';
  const want = { ctrl: parts.includes('strg'), shift: parts.includes('umschalt'), alt: parts.includes('alt') };
  if ((e.ctrlKey || e.metaKey) !== want.ctrl || e.shiftKey !== want.shift || e.altKey !== want.alt) return false;
  return e.key.toLowerCase() === (names[key] ?? key);
}
