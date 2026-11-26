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
            <div class="value" class:raw={f.kind === 'raw'}>
              <EditableText
                multiline
                value={f.value}
                label={f.label}
                placeholder={short(f.label)}
                onCommit={(v: string) => setText(f.path, v, f.numeric)}
              />
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
  .list {
    display: grid;
    gap: 2px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(90px, 30%) minmax(0, 1fr);
    align-items: baseline;
    gap: 0 12px;
  }
  .label {
    font-family: var(--ui);
    font-size: 11.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag,
  .opt {
    margin-left: 6px;
    font-size: 9.5px;
    letter-spacing: 0.04em;
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
    margin-left: -6px;
  }
  .value.raw {
    --et-font: var(--mono);
    --et-size: 12.5px;
  }
  /* Schmale Spalten (etwa neben einem Bild): Bezeichnung über dem Wert. */
  @container lf (max-width: 340px) {
    .row {
      grid-template-columns: minmax(0, 1fr);
    }
    .label {
      white-space: normal;
      margin-top: 4px;
    }
  }
  .prose {
    margin: 2px 0 8px;
    font-size: 14px;
    color: var(--text-2);
  }
</style>
