// Kurze Rückmeldungen am unteren Rand.

export type Toast = { kind: 'info' | 'error'; text: string };

export const toastState = $state<{ current: Toast | null }>({ current: null });

let timer: ReturnType<typeof setTimeout> | undefined;

export function notify(kind: Toast['kind'], text: string) {
  toastState.current = { kind, text };
  clearTimeout(timer);
  timer = setTimeout(() => (toastState.current = null), kind === 'error' ? 8000 : 4000);
}
