<script lang="ts">
  // LaTeX-Element eines Moduls: Der Quelltext ist fest, bearbeitet werden
  // seine Eingabefelder. Ohne Felder zeigt es nur den Quelltext.
  import EditableText from './EditableText.svelte';
  import ImageView from './ImageView.svelte';
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

  /** Fester Anteil zur Orientierung: Feldmarken durch ihre Bezeichnung ersetzt. */
  let outline = $derived(
    el.source
      .replace(/\\(?:feld|rohfeld|bildfeld)(?:\[[^\]]*\])?\{([^{}]*)\}/g, (_, id: string) => {
        const f = el.fields.find((x) => x.path.split('/').pop() === id);
        return `‹${f?.label ?? id}›`;
      })
      .trim(),
  );
  // Nur zeigen, wenn außer den Feldern nennenswert fester Text steht.
  let hasFixedText = $derived(outline.replace(/‹[^›]*›|\\[a-zA-Z]+\*?|[{}\[\]\s\\&%$.,:;|=0-9-]/g, '').length > 12);
</script>

{#if el.fields.length === 0}
  <pre class="fixed" title="Fester LaTeX-Anteil des Moduls">{el.source}</pre>
{:else}
  <div class="fields">
    {#each images as f (f.path)}
      <div class="picture">
        <ImageView {part} field={f.path} value={f.value || null} width={1} {onCommand} {onImage} />
      </div>
    {/each}
    {#if texts.length}
      <dl>
        {#each texts as f (f.path)}
          <dt>{f.label}</dt>
          <dd class:raw={f.kind === 'raw'}>
            <EditableText
              multiline
              value={f.value}
              label={f.label}
              placeholder={f.kind === 'raw' ? 'LaTeX' : f.label}
              onCommit={(v: string) => setText(f.path, v, f.numeric)}
            />
          </dd>
        {/each}
      </dl>
    {/if}
    {#if hasFixedText}
      <pre class="fixed" title="Fester LaTeX-Anteil des Moduls. Änderbar in der Modul-Werkstatt.">{outline}</pre>
    {/if}
  </div>
{/if}

<style>
  .fixed {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
    background: var(--panel-2);
    padding: 6px 10px;
    border-radius: 6px;
    white-space: pre-wrap;
    margin: 4px 0 8px;
    max-height: 5.2em;
    overflow: hidden;
  }
  .fields {
    margin: 2px 0 8px;
  }
  .picture {
    max-width: 420px;
    margin: 0 auto;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 1px 10px;
    align-items: baseline;
    margin: 0;
  }
  dt {
    font-family: var(--ui);
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  dd.raw {
    --et-font: var(--mono);
    --et-size: 12.5px;
  }
</style>
