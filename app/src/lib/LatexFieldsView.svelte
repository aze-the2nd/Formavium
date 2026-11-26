<script lang="ts">
  // LaTeX-Element eines Moduls: Der Quelltext ist fest, bearbeitet werden
  // seine Eingabefelder. Fester Text erscheint nur, wenn er etwas zum
  // Verständnis beiträgt; reine Steuerbefehle bleiben unsichtbar.
  import EditableText from './EditableText.svelte';
  import ImageView from './ImageView.svelte';
  import { texPreview } from './texpreview';
  import type { Command, Doc, El } from './api';

  type Latex = Extract<El, { kind: 'latex' }>;
  type Props = {
    el: Latex;
    part: string;
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
    onImage: (part: string, field: string, file: File) => void;
  };
  let { el, part, onCommand, onImage }: Props = $props();

  const setText = (path: string, value: string, numeric: boolean) =>
    onCommand({ op: 'set_value', part, path, value: { type: numeric ? 'number' : 'text', value } });

  let texts = $derived(el.fields.filter((f) => f.kind !== 'image'));
  let images = $derived(el.fields.filter((f) => f.kind === 'image'));

  /** Lesbarer fester Text: Feldmarken als ‹Bezeichnung›, Befehle entfernt. */
  let prose = $derived.by(() => {
    const marked = el.source.replace(/\\(?:feld|rohfeld|bildfeld)(?:\[[^\]]*\])?\{([^{}]*)\}/g, (_, id: string) => {
      const f = el.fields.find((x) => x.path.split('/').pop() === id);
      return `‹${f?.label.replace(/\s*\(.*\)$/, '') ?? id}›`;
    });
    // Befehle ohne Inhalt, Umgebungen, Maße und Kommentare weglassen.
    const text = marked
      .replace(/%.*$/gm, '')
      .replace(/\\(begin|end)\{[^}]*\}(\[[^\]]*\])?(\{[^}]*\})*/g, ' ')
      .replace(/\\(vspace|hspace|setlength|rule|includegraphics|makebox|parbox|raisebox|fontsize|definecolor|renewcommand|newcommand|providecommand)\*?(\[[^\]]*\])*(\{[^{}]*\})*/g, ' ')
      .replace(/\\(?:par|noindent|newline|newpage|hfill|vfill|centering|bigskip|medskip|smallskip|relax|bfseries|itshape|small|large|Large|LARGE|footnotesize|normalsize|endgroup|begingroup|hline|cline|selectfont)\b/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();
    const letters = text.replace(/‹[^›]*›|\\[A-Za-z]+|[^A-Za-zÄÖÜäöüß]/g, '');
    return letters.length >= 3 ? text : '';
  });

  // TeX-Felder zeigen ihre Vorschau, bis sie angeklickt werden.
  let rawEditing = $state<string | null>(null);

  const short = (label: string) => label.replace(/\s*\((?:optional|LaTeX[^)]*)\)\s*$/i, '');
  const optional = (label: string) => /optional/i.test(label);
</script>

{#if el.fields.length === 0}
  {#if prose}
    <p class="prose" title="Fester Text des Moduls. Änderbar in der Modul-Werkstatt.">{@html texPreview(prose)}</p>
  {/if}
{:else}
  <div class="fields">
    {#each images as f (f.path)}
      <div class="picture" data-field={f.path}>
        <ImageView {part} field={f.path} value={f.value || null} width={1} {onCommand} {onImage} />
      </div>
    {/each}
    {#if texts.length}
      <div class="list">
        {#each texts as f (f.path)}
          <div class="row" data-field={f.path}>
            <span class="label" title={f.label}>
              {short(f.label)}{#if f.kind === 'raw'}<span class="tag">TeX</span>{/if}{#if optional(f.label)}<span class="opt">optional</span>{/if}
            </span>
            <div class="value" class:raw={f.kind === 'raw' && rawEditing === f.path}>
              {#if f.kind === 'raw' && rawEditing !== f.path}
                <button
                  class="preview"
                  title="Klicken zum Bearbeiten (LaTeX)"
                  aria-label="{f.label} bearbeiten"
                  onclick={() => (rawEditing = f.path)}
                  onfocus={() => (rawEditing = f.path)}
                >
                  {#if f.value}{@html texPreview(f.value)}{:else}<span class="empty">{short(f.label)}</span>{/if}
                </button>
              {:else}
                <EditableText
                  multiline
                  autofocus={rawEditing === f.path}
                  value={f.value}
                  label={f.label}
                  placeholder={short(f.label)}
                  onCommit={(v: string) => setText(f.path, v, f.numeric)}
                  onBlur={() => {
                    if (rawEditing === f.path) rawEditing = null;
                  }}
                />
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
    {#if prose && texts.length + images.length < 2}
      <p class="prose" title="Fester Text des Moduls. Änderbar in der Modul-Werkstatt.">{@html texPreview(prose)}</p>
    {/if}
  </div>
{/if}

<style>
  .fields {
    margin: 2px 0 8px;
    container: lf / inline-size;
  }
  .picture {
    max-width: 460px;
    margin: 0 auto;
  }
  /* Wie eine Tabelle im Mockup: Bezeichnung links, Wert rechts. */
  .list {
    display: grid;
    border-left: 1px solid var(--grid-line);
    border-top: 1px solid var(--grid-line);
    font-family: var(--ui);
    font-size: 13px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(110px, 34%) minmax(0, 1fr);
    min-height: 36px;
  }
  .label,
  .value {
    border-right: 1px solid var(--grid-line);
    border-bottom: 1px solid var(--grid-line);
  }
  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 11px;
    font-size: 12px;
    color: var(--muted);
    background: var(--grid-index);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .tag,
  .opt {
    font-size: 9px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .tag {
    color: var(--accent-strong);
    font-weight: 700;
  }
  .opt {
    color: var(--line-strong);
  }
  .value {
    min-width: 0;
    display: flex;
    align-items: center;
    padding: 2px 4px;
    color: var(--grid-label);
    --et-font: var(--ui);
    --et-size: 13px;
    --et-pad: 5px 7px;
  }
  .value > :global(*) {
    flex: 1;
  }
  .value.raw {
    --et-font: var(--mono);
    --et-size: 12px;
  }
  .preview {
    display: block;
    width: 100%;
    min-height: 30px;
    text-align: left;
    font: 13px var(--ui);
    color: inherit;
    background: none;
    border: 1px solid transparent;
    border-radius: 5px;
    padding: 5px 7px;
    cursor: text;
  }
  .preview:hover {
    background: var(--grid-hover);
  }
  .preview :global(sub),
  .preview :global(sup) {
    font-size: 0.7em;
  }
  .empty {
    color: var(--muted);
    opacity: 0.7;
  }
  /* Schmale Spalten (etwa neben einem Bild): Bezeichnung über dem Wert. */
  @container lf (max-width: 340px) {
    .row {
      grid-template-columns: minmax(0, 1fr);
    }
    .label {
      white-space: normal;
      border-bottom: none;
      padding-bottom: 0;
    }
  }
  .prose {
    margin: 2px 0 8px;
    font-size: 14px;
    color: var(--text-2);
  }
</style>
