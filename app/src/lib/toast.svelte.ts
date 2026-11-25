// Kurze Rückmeldungen am unteren Rand, auf Wunsch mit einer Aktion.

export type Toast = { kind: 'info' | 'error'; text: string; action?: { label: string; run: () => void } };

export const toastState = $state<{ current: Toast | null }>({ current: null });

let timer: ReturnType<typeof setTimeout> | undefined;

export function notify(kind: Toast['kind'], text: string, action?: Toast['action']) {
  toastState.current = { kind, text, action };
  clearTimeout(timer);
  timer = setTimeout(() => (toastState.current = null), action || kind === 'error' ? 9000 : 4000);
}

export function dismissToast() {
  clearTimeout(timer);
  toastState.current = null;
}
