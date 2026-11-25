// Automatische Updates aus den GitHub-Releases (nur in der Desktop-App).
//
// Beim Start wird nach einer neueren Version gesucht. Ist noch kein Dokument
// offen, lädt und installiert die App sie sofort und startet neu. Später
// gefundene Updates warten auf einen Klick, damit keine Arbeit verloren geht.

import type { Update } from '@tauri-apps/plugin-updater';
import { isTauri } from './api';
import { notify } from './toast.svelte';

type Phase = 'idle' | 'checking' | 'current' | 'pending' | 'downloading' | 'installing';

export const updateState = $state<{
  phase: Phase;
  /** Installierte Version. */
  current: string | null;
  /** Gefundene neuere Version. */
  version: string | null;
  notes: string | null;
  /** Anteil des Downloads, 0 bis 1. */
  progress: number | null;
  /** Der Fortschritt verdeckt die Oberfläche (Installation beim Start). */
  blocking: boolean;
}>({ phase: 'idle', current: null, version: null, notes: null, progress: null, blocking: false });

let found: Update | null = null;

export async function loadVersion() {
  if (!isTauri) return;
  try {
    const { getVersion } = await import('@tauri-apps/api/app');
    updateState.current = await getVersion();
  } catch {
    /* ohne Versionsangabe weiter */
  }
}

/** Sucht nach einer neueren Version. `quiet`: Fehler und „aktuell“ nicht melden. */
export async function checkForUpdate(quiet = true): Promise<boolean> {
  if (!isTauri) {
    if (!quiet) notify('info', 'Updates gibt es nur in der installierten App.');
    return false;
  }
  if (updateState.phase === 'downloading' || updateState.phase === 'installing') return false;
  updateState.phase = 'checking';
  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    found = await check({ timeout: 15000 });
  } catch (e) {
    updateState.phase = 'idle';
    // Offline ist der Normalfall dieser App, kein Fehler.
    if (!quiet) notify('error', `Update-Suche nicht möglich: ${e instanceof Error ? e.message : String(e)}`);
    return false;
  }
  if (!found) {
    updateState.phase = 'current';
    if (!quiet) notify('info', `Formavium ${updateState.current ?? ''} ist aktuell.`);
    return false;
  }
  updateState.version = found.version;
  updateState.notes = found.body ?? null;
  updateState.phase = 'pending';
  return true;
}

/** Lädt und installiert das gefundene Update. Die App startet danach neu. */
export async function installUpdate(blocking = false) {
  if (!found || updateState.phase === 'downloading' || updateState.phase === 'installing') return;
  updateState.phase = 'downloading';
  updateState.progress = 0;
  updateState.blocking = blocking;
  let total = 0;
  let loaded = 0;
  try {
    await found.downloadAndInstall((event) => {
      if (event.event === 'Started') total = event.data.contentLength ?? 0;
      else if (event.event === 'Progress') {
        loaded += event.data.chunkLength;
        updateState.progress = total ? Math.min(1, loaded / total) : null;
      } else if (event.event === 'Finished') updateState.phase = 'installing';
    });
    // Unter Windows beendet das Installationsprogramm die App und startet sie neu.
  } catch (e) {
    updateState.phase = 'pending';
    updateState.blocking = false;
    notify('error', `Update fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/**
 * Beim Start: suchen und, solange noch nichts geöffnet ist, sofort
 * installieren. Danach regelmäßig weitersuchen.
 */
export async function startAutoUpdate(nothingOpen: () => boolean) {
  await loadVersion();
  if (!isTauri) return;
  if ((await checkForUpdate()) && nothingOpen()) await installUpdate(true);
  setInterval(() => {
    if (updateState.phase === 'idle' || updateState.phase === 'current') checkForUpdate();
  }, 4 * 60 * 60 * 1000);
}
