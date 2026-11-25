// Feedback als Issue: Text zusammenstellen, senden oder im Browser öffnen.

import { invokeApp, isTauri } from './api';

export const REPO = 'aze-the2nd/Formavium';
export const TOKEN_HELP_URL = 'https://github.com/settings/personal-access-tokens/new';

export type Kind = 'bug' | 'idea';
export type Draft = { kind: Kind; title: string; body: string; withContext: boolean };

const DRAFT_KEY = 'formavium.feedback';

export function loadDraft(): Draft {
  const empty: Draft = { kind: 'bug', title: '', body: '', withContext: true };
  try {
    const saved = JSON.parse(localStorage.getItem(DRAFT_KEY) ?? 'null') as Partial<Draft> | null;
    return saved ? { ...empty, ...saved, kind: saved.kind === 'idea' ? 'idea' : 'bug' } : empty;
  } catch {
    return empty;
  }
}

export function saveDraft(draft: Draft | null) {
  try {
    if (draft && (draft.title || draft.body)) localStorage.setItem(DRAFT_KEY, JSON.stringify(draft));
    else localStorage.removeItem(DRAFT_KEY);
  } catch {
    /* ohne Speicher geht nur der Entwurf verloren */
  }
}

/**
 * Technische Angaben zum Anhängen. Bewusst ohne Dokumentinhalt, Dateinamen
 * oder Pfade: Das Issue ist öffentlich.
 */
export function technicalContext(version: string | null, views: string[]): string {
  const ua = navigator.userAgent;
  const windows = /Windows NT ([\d.]+)/.exec(ua)?.[1];
  const engine = /Edg\/([\d.]+)/.exec(ua)?.[1] ?? /Chrome\/([\d.]+)/.exec(ua)?.[1];
  return [
    `Formavium ${version ?? '(Entwicklung)'}`,
    windows ? `Windows NT ${windows}` : navigator.platform,
    engine ? `WebView ${engine}` : null,
    `Fenster ${window.innerWidth}×${window.innerHeight}`,
    `Ansichten: ${views.join(', ')}`,
  ]
    .filter(Boolean)
    .join(' · ');
}

export function issueBody(draft: Draft, context: string): string {
  const body = draft.body.trim();
  return draft.withContext ? `${body}\n\n---\n${context}` : body;
}

/** Vorausgefülltes Issue-Formular auf GitHub. */
export function issueUrl(draft: Draft, context: string): string {
  const params = new URLSearchParams({
    labels: draft.kind === 'bug' ? 'bug' : 'enhancement',
    title: draft.title.trim(),
    // Browser und GitHub begrenzen die Länge der Adresse.
    body: issueBody(draft, context).slice(0, 5000),
  });
  return `https://github.com/${REPO}/issues/new?${params}`;
}

export async function openInBrowser(url: string) {
  if (isTauri) await invokeApp('open_repo_url', { url });
  else window.open(url, '_blank', 'noopener');
}

export async function hasToken(): Promise<boolean> {
  if (!isTauri) return false;
  return (await invokeApp<{ has_token: boolean }>('feedback_status')).has_token;
}

export const setToken = (token: string) => invokeApp<void>('feedback_set_token', { token });
export const clearToken = () => invokeApp<void>('feedback_clear_token');

export function submit(draft: Draft, context: string) {
  return invokeApp<{ number: number; url: string }>('feedback_submit', {
    feedback: { kind: draft.kind, title: draft.title, body: issueBody(draft, context) },
  });
}
