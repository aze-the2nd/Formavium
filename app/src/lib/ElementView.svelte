<script lang="ts">
  // Ein Element eines Moduls, dargestellt wie im Dokument und direkt bearbeitbar.
  import ElementView from './ElementView.svelte';
  import EditableText from './EditableText.svelte';
  import ImageView from './ImageView.svelte';
  import TableView from './TableView.svelte';
  import type { Command, Doc, El } from './api';

  type Props = {
    el: El;
    part: string;
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
    onImage: (part: string, field: string, file: File) => void;
  };
  let { el, part, onCommand, onImage }: Props = $props();

  const setText = (field: string, value: string, numeric: boolean) =>
    onCommand({ op: 'set_value', part, path: field, value: { type: numeric ? 'number' : 'text', value } });
</script>

{#if el.kind === 'heading'}
  <div class="heading h{Math.min(Math.max(el.level, 1), 3)}">
    {#if el.field}
      {@const field = el.field}
      <EditableText
        value={el.text}
        label="Überschrift"
        placeholder="Überschrift"
        onCommit={(v: string) => setText(field, v, el.numeric)}
      />
    {:else}
      <div class="fixed">{el.text}</div>
    {/if}
  </div>
{:else if el.kind === 'paragraph'}
  <div class="paragraph">
    {#if el.field}
      {@const field = el.field}
      <EditableText
        multiline
        value={el.text}
        label="Text"
        placeholder="Text"
        onCommit={(v: string) => setText(field, v, el.numeric)}
      />
    {:else}
      <p class="fixed">{el.text}</p>
    {/if}
  </div>
{:else if el.kind === 'table'}
  <TableView t={el} {part} {onCommand} />
{:else if el.kind === 'image'}
  <ImageView {part} field={el.field} value={el.value} width={el.width} {onCommand} {onImage} />
{:else if el.kind === 'row'}
  <div class="row" style:grid-template-columns="repeat({el.children.length}, minmax(0, 1fr))">
    {#each el.children as child}
      <div class="col"><ElementView el={child} {part} {onCommand} {onImage} /></div>
    {/each}
  </div>
{:else if el.kind === 'module'}
  <div class="submodule">
    <div class="sub-title">{el.name}</div>
    {#each el.children as child}
      <ElementView el={child} {part} {onCommand} {onImage} />
    {/each}
  </div>
{:else if el.kind === 'latex'}
  <pre class="latex" title="Fester LaTeX-Anteil des Moduls">{el.source}</pre>
{/if}

<style>
  .heading {
    --et-weight: 700;
    --et-line: 1.25;
    font-weight: 700;
    margin: 2px -6px 6px;
  }
  .h1 {
    --et-size: 20px;
    font-size: 20px;
  }
  .h2 {
    --et-size: 17px;
    font-size: 17px;
  }
  .h3 {
    --et-size: 15px;
    font-size: 15px;
  }
  .paragraph {
    --et-line: 1.5;
    margin: 0 -6px 8px;
    line-height: 1.5;
  }
  .fixed {
    margin: 0;
    padding: 4px 6px;
  }
  .row {
    display: grid;
    gap: 22px;
    align-items: start;
  }
  .col {
    min-width: 0;
  }
  /* In schmalen Ansichten untereinander statt nebeneinander. */
  @container part (max-width: 520px) {
    .row {
      grid-template-columns: minmax(0, 1fr) !important;
      gap: 0;
    }
    .col > :global(.image) {
      margin-top: 0;
    }
  }
  /* Bild neben einer Tabelle auf Höhe der Tabelle beginnen lassen. */
  .col > :global(.image) {
    margin-top: 22px;
  }
  .submodule {
    border-left: 2px solid var(--line-strong);
    padding-left: 12px;
    margin: 6px 0 10px;
  }
  .sub-title {
    font-family: var(--ui);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    margin-bottom: 4px;
  }
  .latex {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
    background: var(--panel-2);
    padding: 8px 10px;
    border-radius: 6px;
    white-space: pre-wrap;
    margin: 4px 0 10px;
  }
</style>
