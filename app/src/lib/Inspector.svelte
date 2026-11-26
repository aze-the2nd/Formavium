<script lang="ts">
  // Zelleditor: bearbeitet die gewählte Tabellenzelle unten in der Ansicht,
  // mit Vorschau. Gut für längere Werte und LaTeX in Zellen.
  import { tick } from 'svelte';
  import { editor } from './editor.svelte';
  import { texPreview } from './texpreview';

  let draft = $state('');
  let key = '';
  let area = $state<HTMLTextAreaElement>();
  let cell = $derived(editor.cell);
  let dirty = $derived(!!cell && draft !== cell.value);

  // Neue Zelle oder geänderter Wert von außen: Entwurf übernehmen.
  $effect(() => {
    const c = editor.cell;
    if (!c) return;
    if (c.key !== key || document.activeElement !== area) {
      key = c.key;
      draft = c.value;
    }
  });

  // Getipptes Zeichen in der Tabelle: hier weiterschreiben.
  $effect(() => {
    const seed = editor.inspectorSeed;
    if (!seed) return;
    tick().then(() => {
      if (seed.text) draft = seed.text;
      area?.focus();
      if (seed.text) area?.setSelectionRange(draft.length, draft.length);
      else area?.select();
    });
  });

  async function apply(back = true) {
    if (!cell) return;
    if (dirty) await cell.commit(draft);
    if (back) cell.back();
  }

  function key_(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey || !e.shiftKey)) {
      e.preventDefault();
      apply();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      draft = cell?.value ?? '';
      cell?.back();
    }
  }
</script>

<section class="inspector" aria-label="Zelleditor">
  {#if cell}
    <header>
      <strong>{cell.title}</strong>
      <span class="addr">{cell.address}{cell.latex ? ' · LaTeX' : ''}</span>
      <span class="spacer"></span>
      <span class="state">{dirty ? 'Eingabe übernimmt · Umschalt+Eingabe neue Zeile' : 'Übernommen'}</span>
      <button class="apply" disabled={!dirty} onclick={() => apply()}>Übernehmen</button>
    </header>
    <div class="body">
      <textarea
        bind:this={area}
        bind:value={draft}
        class:mono={cell.latex}
        rows="2"
        spellcheck="false"
        aria-label="Inhalt der Zelle {cell.address}"
        onkeydown={key_}
        onblur={() => apply(false)}
      ></textarea>
      <div class="preview" aria-label="Vorschau">
        {#if cell.latex}{@html texPreview(draft) || '–'}{:else}{draft || '–'}{/if}
      </div>
    </div>
  {:else}
    <p class="none">Zelle in einer Tabelle wählen, um sie hier zu bearbeiten.</p>
  {/if}
</section>

<style>
  .inspector {
    flex: none;
    border-top: 1px solid var(--line-strong);
    background: var(--panel);
    padding: 8px 14px 10px;
    font-family: var(--ui);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 12px;
  }
  strong {
    font-size: 13px;
  }
  .addr,
  .state {
    color: var(--muted);
    font-size: 11px;
  }
  .spacer {
    flex: 1;
  }
  .apply {
    font: inherit;
    font-size: 11px;
    color: white;
    background: var(--accent);
    border: none;
    border-radius: 5px;
    padding: 3px 10px;
    cursor: pointer;
  }
  .apply:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
    gap: 10px;
    margin-top: 6px;
  }
  textarea {
    font: inherit;
    font-size: 13px;
    color: var(--text);
    background: var(--panel-2);
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    padding: 6px 8px;
    resize: vertical;
    min-height: 40px;
    max-height: 140px;
  }
  textarea.mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  textarea:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .preview {
    font-family: var(--serif);
    font-size: 14px;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--paper);
    color: var(--paper-ink);
    min-height: 40px;
    overflow-wrap: anywhere;
  }
  .preview :global(sub),
  .preview :global(sup) {
    font-size: 0.72em;
  }
  .none {
    margin: 2px 0;
    font-size: 12px;
    color: var(--muted);
  }
</style>
