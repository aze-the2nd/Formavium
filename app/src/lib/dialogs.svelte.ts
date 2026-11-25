// In-App-Dialoge (Bestätigung und Texteingabe) mit Promise-Schnittstelle.

export type DialogRequest = {
  title: string;
  message?: string;
  input?: string;
  okLabel: string;
  cancelLabel: string;
  danger?: boolean;
  resolve: (value: string | boolean | null) => void;
};

export const dialogState = $state<{ current: DialogRequest | null }>({ current: null });

export function confirmDialog(opts: {
  title: string;
  message: string;
  okLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}): Promise<boolean> {
  return new Promise((resolve) => {
    dialogState.current = {
      okLabel: 'OK',
      cancelLabel: 'Abbrechen',
      ...opts,
      resolve: (v) => resolve(v === true),
    };
  });
}

export function promptDialog(opts: { title: string; message?: string; value: string; okLabel?: string }): Promise<string | null> {
  return new Promise((resolve) => {
    dialogState.current = {
      title: opts.title,
      message: opts.message,
      input: opts.value,
      okLabel: opts.okLabel ?? 'OK',
      cancelLabel: 'Abbrechen',
      resolve: (v) => resolve(typeof v === 'string' ? v : null),
    };
  });
}
