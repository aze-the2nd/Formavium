<script lang="ts">
  // PDF-Vorschau. Klick auf eine Stelle springt zum Feld oder zur Zelle im
  // visuellen Editor (SyncTeX), die gewählte Stelle ist farbig markiert.
  import { onDestroy, onMount } from 'svelte';
  import type { PDFDocumentProxy } from 'pdfjs-dist';
  import { pdfjs } from './pdf';
  import { fetchPdf, type Diag, type Preview } from './api';

  /** Seite, links, oben, Breite, Höhe in PDF-Punkten. */
  export type Mark = [number, number, number, number, number];

  type Props = {
    preview: Preview | null;
    revision: number;
    marks: Mark[];
    onDiag: (d: Diag) => void;
    onLocate: (page: number, x: number, y: number) => void;
    /** Vom Anwender ausgelöstes Scrollen (für die Synchronisierung). */
    onScroll?: () => void;
  };
  let { preview, revision, marks, onDiag, onLocate, onScroll }: Props = $props();

  let pagesEl: HTMLDivElement;
  let pdf: PDFDocumentProxy | null = null;
  let loadedRevision = $state<number | null>(null);
  let pageCount = $state(0);
  let renderToken = 0;
  /** CSS-Pixel je PDF-Punkt der gezeichneten Seiten. */
  let scale = 1;

  function loadZoom() {
    try {
      const z = Number(localStorage.getItem('formavium.pdfZoom'));
      return z >= 0.5 && z <= 2.5 ? z : 1;
    } catch {
      return 1;
    }
  }
  let zoom = $state(loadZoom());

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

  /** Alle Seiten neu zeichnen und dann austauschen (kein Flackern). */
  async function render() {
    try {
      await draw();
    } catch (e) {
      // Ein neuerer Zeichenlauf hat diesen abgelöst: kein Fehler.
      if ((e as { name?: string })?.name !== 'RenderingCancelledException') console.warn('PDF-Vorschau:', e);
    }
  }

  async function draw() {
    if (!pdf || !pagesEl) return;
    const token = ++renderToken;
    const keep = topPoint();
    const width = Math.max(200, (pagesEl.clientWidth - 32) * zoom);
    const dpr = window.devicePixelRatio || 1;
    const wraps: HTMLDivElement[] = [];
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const base = page.getViewport({ scale: 1 });
      scale = width / base.width;
      const viewport = page.getViewport({ scale: scale * dpr });
      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      canvas.style.width = `${Math.floor(viewport.width / dpr)}px`;
      canvas.className = 'page';
      await page.render({ canvasContext: canvas.getContext('2d')!, viewport }).promise;
      if (token !== renderToken) return;
      const wrap = document.createElement('div');
      wrap.className = 'page-wrap';
      wrap.dataset.page = String(i);
      wrap.setAttribute('aria-label', `Seite ${i}`);
      const layer = document.createElement('div');
      layer.className = 'marks';
      wrap.append(canvas, layer);
      wraps.push(wrap);
    }
    pagesEl.replaceChildren(...wraps);
    drawMarks();
    if (keep) scrollToPoint(keep.page, keep.y, false);
  }

  function drawMarks() {
    if (!pagesEl) return;
    for (const layer of pagesEl.querySelectorAll<HTMLElement>('.marks')) layer.replaceChildren();
    for (const [page, x, y, w, h] of marks) {
      const layer = pagesEl.querySelector<HTMLElement>(`.page-wrap[data-page="${page}"] .marks`);
      if (!layer) continue;
      const m = document.createElement('div');
      m.className = 'mark';
      m.style.left = `${(x - 2) * scale}px`;
      m.style.top = `${(y - 1.5) * scale}px`;
      m.style.width = `${(w + 4) * scale}px`;
      m.style.height = `${(h + 3) * scale}px`;
      layer.append(m);
    }
  }

  $effect(() => {
    void marks;
    drawMarks();
  });

  /** Ersten markierten Bereich sichtbar machen, falls er außerhalb liegt. */
  export function revealMarks() {
    const first = pagesEl?.querySelector<HTMLElement>('.mark');
    first?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }

  $effect(() => {
    const rev = preview?.shown_revision ?? null;
    if (rev !== null && rev !== loadedRevision) load(rev);
  });

  // --- Lage für das synchrone Scrollen ---

  /** Oberkante des sichtbaren Bereichs als Seite und PDF-Punkt. */
  export function topPoint(): { page: number; y: number } | null {
    if (!pagesEl) return null;
    const top = pagesEl.scrollTop + 8;
    for (const wrap of pagesEl.querySelectorAll<HTMLElement>('.page-wrap')) {
      if (wrap.offsetTop + wrap.offsetHeight > top) {
        return { page: Number(wrap.dataset.page), y: Math.max(0, (top - wrap.offsetTop) / scale) };
      }
    }
    return null;
  }

  export function scrollToPoint(page: number, y: number, smooth = false) {
    const wrap = pagesEl?.querySelector<HTMLElement>(`.page-wrap[data-page="${page}"]`);
    if (!wrap) return;
    pagesEl.scrollTo({ top: wrap.offsetTop + y * scale - 8, behavior: smooth ? 'smooth' : 'auto' });
  }

  function click(e: MouseEvent) {
    const wrap = (e.target as HTMLElement).closest<HTMLElement>('.page-wrap');
    if (!wrap || e.button !== 0) return;
    const r = wrap.getBoundingClientRect();
    onLocate(Number(wrap.dataset.page), (e.clientX - r.left) / scale, (e.clientY - r.top) / scale);
  }

  function setZoom(z: number) {
    zoom = Math.round(Math.min(2.5, Math.max(0.5, z)) * 10) / 10;
    try {
      localStorage.setItem('formavium.pdfZoom', String(zoom));
    } catch {
      /* nur für diese Sitzung */
    }
    render();
  }

  function wheel(e: WheelEvent) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    setZoom(zoom + (e.deltaY < 0 ? 0.1 : -0.1));
  }

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
    <span class="spacer"></span>
    {#if preview?.pending}<span class="pending">kompiliert…</span>{/if}
    <span class="zoom" role="group" aria-label="Zoom">
      <button onclick={() => setZoom(zoom - 0.1)} disabled={zoom <= 0.5} aria-label="Verkleinern">−</button>
      <button class="level" onclick={() => setZoom(1)} title="Auf Breite einpassen (Strg+Mausrad zoomt)">{Math.round(zoom * 100)} %</button>
      <button onclick={() => setZoom(zoom + 0.1)} disabled={zoom >= 2.5} aria-label="Vergrößern">+</button>
    </span>
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

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="pages"
    class:stale={preview?.status === 'stale'}
    class:wide={zoom > 1}
    bind:this={pagesEl}
    title="Klick springt zur Stelle im Editor"
    onclick={click}
    onwheel={wheel}
    onscroll={() => onScroll?.()}
  ></div>
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
    padding: 5px 10px 5px 14px;
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
  .spacer {
    flex: 1;
  }
  .pending {
    color: var(--muted);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .zoom {
    display: flex;
    align-items: center;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  .zoom button {
    font: inherit;
    font-size: 12px;
    min-width: 24px;
    height: 22px;
    border: none;
    background: none;
    color: var(--text-2);
    cursor: pointer;
  }
  .zoom .level {
    min-width: 48px;
    font-size: 11px;
    color: var(--muted);
  }
  .zoom button:hover:not(:disabled) {
    background: var(--panel-2);
    color: var(--text);
  }
  .zoom button:disabled {
    opacity: 0.4;
    cursor: default;
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
  .pages.wide {
    overflow-x: auto;
    align-items: flex-start;
  }
  .pages.stale :global(canvas) {
    opacity: 0.55;
    filter: grayscale(0.4);
  }
  .pages :global(.page-wrap) {
    position: relative;
    flex: none;
    cursor: pointer;
  }
  .pages :global(.page) {
    display: block;
    height: auto;
    background: white;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.18), 0 6px 18px rgba(0, 0, 0, 0.08);
  }
  .pages :global(.marks) {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .pages :global(.mark) {
    position: absolute;
    border-radius: 2px;
    background: rgba(47, 111, 223, 0.16);
    box-shadow: 0 0 0 1.5px rgba(47, 111, 223, 0.75);
    animation: mark-in 0.18s ease-out;
  }
  @keyframes -global-mark-in {
    from {
      opacity: 0;
    }
  }
</style>
