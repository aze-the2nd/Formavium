<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import type { PDFDocumentProxy } from 'pdfjs-dist';
  import { pdfjs } from './pdf';
  import { fetchPdf, type Diag, type Preview } from './api';

  type Props = {
    preview: Preview | null;
    revision: number;
    onDiag: (d: Diag) => void;
  };
  let { preview, revision, onDiag }: Props = $props();

  let pagesEl: HTMLDivElement;
  let pdf: PDFDocumentProxy | null = null;
  let loadedRevision = $state<number | null>(null);
  let pageCount = $state(0);
  let renderToken = 0;

  async function load(rev: number) {
    const bytes = await fetchPdf();
    if (!bytes) return;
    const next = await pdfjs.getDocument({ data: bytes, isEvalSupported: false }).promise;
    pdf?.destroy();
    pdf = next;
    loadedRevision = rev;
    pageCount = next.numPages;
    await render();
  }

  /** Alle Seiten in neue Canvases zeichnen und dann austauschen (kein Flackern). */
  async function render() {
    if (!pdf || !pagesEl) return;
    const token = ++renderToken;
    const width = Math.max(200, pagesEl.clientWidth - 32);
    const dpr = window.devicePixelRatio || 1;
    const canvases: HTMLCanvasElement[] = [];
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const base = page.getViewport({ scale: 1 });
      const viewport = page.getViewport({ scale: (width / base.width) * dpr });
      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      canvas.style.width = `${Math.floor(viewport.width / dpr)}px`;
      canvas.className = 'page';
      canvas.setAttribute('aria-label', `Seite ${i}`);
      await page.render({ canvasContext: canvas.getContext('2d')!, viewport }).promise;
      if (token !== renderToken) return;
      canvases.push(canvas);
    }
    pagesEl.replaceChildren(...canvases);
  }

  $effect(() => {
    const rev = preview?.shown_revision ?? null;
    if (rev !== null && rev !== loadedRevision) load(rev);
  });

  let resizeTimer: ReturnType<typeof setTimeout>;
  let observer: ResizeObserver | undefined;
  onMount(() => {
    // Nur bei geänderter Breite neu zeichnen, nicht bei Höhenänderungen.
    let lastWidth = pagesEl.clientWidth;
    observer = new ResizeObserver(() => {
      if (pagesEl.clientWidth === lastWidth) return;
      lastWidth = pagesEl.clientWidth;
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(render, 150);
    });
    observer.observe(pagesEl);
  });
  onDestroy(() => {
    observer?.disconnect();
    pdf?.destroy();
  });

  const fmt = (ms: number | null) => (ms === null ? '' : `${(ms / 1000).toLocaleString('de-DE', { maximumFractionDigits: 2 })} s`);
  let errors = $derived(preview?.diagnostics.filter((d) => d.severity === 'error') ?? []);
</script>

<div class="pdf">
  <div class="status {preview?.status ?? 'empty'}" role="status">
    {#if !preview || preview.status === 'empty'}
      <span class="dot"></span> Noch keine Vorschau
    {:else if preview.status === 'current'}
      <span class="dot"></span>
      {preview.shown_revision === revision ? 'Aktuell' : 'Vorschau'} · Revision {preview.shown_revision}
      <span class="meta">{fmt(preview.duration_ms)}{pageCount ? ` · ${pageCount} ${pageCount === 1 ? 'Seite' : 'Seiten'}` : ''}</span>
    {:else if preview.status === 'stale'}
      <span class="dot"></span>
      <strong>Älterer Stand</strong> (Revision {preview.shown_revision}) – Revision {preview.failed_revision} enthält Fehler
    {:else}
      <span class="dot"></span>
      <strong>Noch kein PDF</strong> – Revision {preview.failed_revision} lässt sich nicht kompilieren
    {/if}
    {#if preview?.pending}<span class="pending">kompiliert…</span>{/if}
  </div>

  {#if errors.length}
    <ul class="diags">
      {#each errors as d}
        <li>
          <button onclick={() => onDiag(d)} title="Zur Stelle springen">
            <span class="msg">{d.message}</span>
            <span class="where">
              {d.place ?? 'ohne Ortsangabe'}{d.line && preview?.diagnostics_current ? ` · Zeile ${d.line}` : ''}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="pages" class:stale={preview?.status === 'stale'} bind:this={pagesEl}></div>
</div>

<style>
  .pdf {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--pdf-bg);
    min-height: 0;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    font-size: 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--muted);
    flex: none;
  }
  .current .dot {
    background: var(--ok);
  }
  .stale {
    background: var(--warn-soft);
  }
  .stale .dot {
    background: var(--warn);
  }
  .status.failed {
    background: var(--danger-soft);
  }
  .failed .dot {
    background: var(--danger);
  }
  .meta {
    color: var(--muted);
  }
  .pending {
    margin-left: auto;
    color: var(--muted);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .diags {
    list-style: none;
    margin: 0;
    padding: 6px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    max-height: 30%;
    overflow: auto;
  }
  .diags button {
    display: grid;
    width: 100%;
    text-align: left;
    gap: 2px;
    background: none;
    border: none;
    border-left: 3px solid var(--danger);
    padding: 6px 10px;
    cursor: pointer;
    color: var(--text);
    border-radius: 0 6px 6px 0;
  }
  .diags button:hover {
    background: var(--danger-soft);
  }
  .msg {
    font-weight: 600;
    font-size: 13px;
  }
  .where {
    font-size: 12px;
    color: var(--muted);
  }
  .pages {
    flex: 1;
    /* Platz für die Bildlaufleiste fest reservieren: Sonst ändert ihr
       Erscheinen die Breite, die Seite wird neu skaliert, die Leiste
       verschwindet wieder, und die Ansicht flackert. */
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    min-height: 0;
  }
  .pages.stale :global(canvas) {
    opacity: 0.55;
    filter: grayscale(0.4);
  }
  .pages :global(.page) {
    /* Bis zum Neuzeichnen nach einer Größenänderung passt die Seite sich an. */
    max-width: 100%;
    height: auto;
    flex: none;
    background: white;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.18), 0 6px 18px rgba(0, 0, 0, 0.08);
  }
</style>
