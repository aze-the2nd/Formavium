<script lang="ts">
  // Schematische Miniatur eines Moduls oder einer Vorlage: zeigt den Aufbau.
  import Miniature from './Miniature.svelte';
  import type { Outline } from './library';

  type Props = { outline: Outline[]; nested?: boolean };
  let { outline, nested = false }: Props = $props();
</script>

<div class="mini" class:nested aria-hidden="true">
  {#each outline.slice(0, nested ? 4 : 9) as o}
    {#if o.kind === 'heading'}
      <div class="heading"></div>
    {:else if o.kind === 'paragraph'}
      <div class="lines"><i></i><i></i><i class="short"></i></div>
    {:else if o.kind === 'list'}
      <div class="list"><i></i><i></i></div>
    {:else if o.kind === 'table'}
      <div class="table" style:--cols={Math.min(o.cols, 6)}>
        {#each { length: Math.min(o.cols, 6) * 3 } as _, i}<b class:head={i < Math.min(o.cols, 6)}></b>{/each}
      </div>
    {:else if o.kind === 'image'}
      <div class="image">
        <svg viewBox="0 0 24 14"><path d="M2 12l6-6 4 4 3-3 7 5z" fill="currentColor" /></svg>
      </div>
    {:else if o.kind === 'row'}
      <div class="row">
        {#each o.children.slice(0, 3) as child}<Miniature outline={[child]} nested />{/each}
      </div>
    {:else if o.kind === 'module'}
      <div class="sub"></div>
    {:else}
      <div class="code"><i></i><i class="short"></i></div>
    {/if}
  {/each}
</div>

<style>
  .mini {
    display: flex;
    flex-direction: column;
    gap: 5px;
    height: 92px;
    padding: 9px 10px;
    border-radius: 7px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    overflow: hidden;
    color: var(--line-strong);
  }
  .mini.nested {
    flex: 1;
    height: auto;
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
  }
  .heading {
    height: 6px;
    width: 55%;
    border-radius: 2px;
    background: var(--muted);
    flex: none;
  }
  .lines,
  .code,
  .list {
    display: grid;
    gap: 3px;
    flex: none;
  }
  i {
    display: block;
    height: 3px;
    border-radius: 2px;
    background: var(--line-strong);
  }
  i.short {
    width: 60%;
  }
  .list i {
    margin-left: 9px;
    position: relative;
  }
  .list i::before {
    content: '';
    position: absolute;
    left: -8px;
    top: -1px;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--muted);
  }
  .code i {
    background: var(--accent);
    opacity: 0.45;
  }
  .table {
    display: grid;
    grid-template-columns: repeat(var(--cols), 1fr);
    gap: 1px;
    background: var(--line-strong);
    border: 1px solid var(--line-strong);
    flex: none;
  }
  .table b {
    height: 6px;
    background: var(--panel);
  }
  .table b.head {
    background: var(--panel-2);
    box-shadow: inset 0 -2px 0 var(--line-strong);
  }
  .image {
    flex: 1;
    min-height: 22px;
    display: grid;
    place-items: center;
    border-radius: 4px;
    background: var(--panel);
    border: 1px solid var(--line-strong);
  }
  .image svg {
    width: 26px;
  }
  .row {
    display: flex;
    gap: 6px;
    flex: 1;
    min-height: 0;
  }
  .sub {
    height: 14px;
    border-radius: 4px;
    border: 1px dashed var(--line-strong);
    flex: none;
  }
</style>
