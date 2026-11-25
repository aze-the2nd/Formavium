// Bilder des Dokuments als anzeigbare Adressen, je Pfad einmal geladen.
import { fetchResource } from './api';
import { pdfjs } from './pdf';

const cache = new Map<string, Promise<string | null>>();

/** Beim Wechsel des Dokuments: relative Pfade meinen dann andere Dateien. */
export function clearResources() {
  for (const entry of cache.values()) {
    entry.then((url) => {
      if (url?.startsWith('blob:')) URL.revokeObjectURL(url);
    });
  }
  cache.clear();
}

export function resourceUrl(path: string): Promise<string | null> {
  let entry = cache.get(path);
  if (!entry) {
    entry = load(path).catch(() => null);
    cache.set(path, entry);
    // Fehlende Dateien später erneut versuchen.
    entry.then((url) => {
      if (url === null) cache.delete(path);
    });
  }
  return entry;
}

async function load(path: string): Promise<string | null> {
  const bytes = await fetchResource(path);
  if (!bytes) return null;
  const lower = path.toLowerCase();
  if (lower.endsWith('.pdf')) return firstPage(bytes);
  const type = lower.endsWith('.png') ? 'image/png' : 'image/jpeg';
  return URL.createObjectURL(new Blob([bytes], { type }));
}

/** Erste Seite eines PDF-Bildes als Vorschau. */
async function firstPage(bytes: Uint8Array): Promise<string | null> {
  const doc = await pdfjs.getDocument({ data: bytes, isEvalSupported: false }).promise;
  try {
    const page = await doc.getPage(1);
    const base = page.getViewport({ scale: 1 });
    const viewport = page.getViewport({ scale: Math.min(3, 900 / base.width) });
    const canvas = document.createElement('canvas');
    canvas.width = Math.floor(viewport.width);
    canvas.height = Math.floor(viewport.height);
    await page.render({ canvasContext: canvas.getContext('2d')!, viewport }).promise;
    return canvas.toDataURL('image/png');
  } finally {
    doc.destroy();
  }
}
