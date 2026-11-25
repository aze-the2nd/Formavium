<script lang="ts">
  // Aufzählung oder nummerierte Liste, direkt bearbeitbar: Eingabe legt den
  // nächsten Punkt an, Rücktaste im leeren Punkt entfernt ihn.
  import { tick } from 'svelte';
  import EditableText from './EditableText.svelte';
  import { openMenu } from './menus.svelte';
  import type { Command, Doc, El } from './api';

  type List = Extract<El, { kind: 'list' }>;
  type Props = { list: List; part: string; onCommand: (cmd: Command) => Promise<Doc | undefined> };
  let { list, part, onCommand }: Props = $props();

  let root = $state<HTMLElement>();

  function focusItem(id: string | undefined, atEnd = false) {
    if (!id) return;
    const el = root?.querySelector<HTMLTextAreaElement>(`[data-item="${CSS.escape(id)}"]`);
    el?.focus();
    if (el && atEnd) el.setSelectionRange(el.value.length, el.value.length);
  }

  async function insert(index: number) {
    await onCommand({ op: 'insert_row', part, path: list.path, index });
    await tick();
    focusItem(list.items[index]?.id);
  }

  async function remove(i: number) {
    const item = list.items[i];
    if (!item) return;
    const next = (list.items[i - 1] ?? list.items[i + 1])?.id;
    await onCommand({ op: 'remove_row', part, path: list.path, row: item.id });
    await tick();
    focusItem(next, true);
  }

  async function move(i: number, to: number) {
    const item = list.items[i];
    if (!item || to < 0 || to >= list.items.length) return;
    await onCommand({ op: 'move_row', part, path: list.path, row: item.id, to });
    await tick();
    focusItem(item.id);
  }

  const setText = (id: string, value: string) =>
    onCommand({ op: 'set_cell', part, path: list.path, row: id, column: 'text', value });

  function key(e: KeyboardEvent, i: number) {
    const field = e.target as HTMLTextAreaElement;
    let handled = true;
    if (e.key === 'Enter' && !e.shiftKey) insert(i + 1);
    else if (e.key === 'Backspace' && field.value === '' && list.items.length > 1) remove(i);
    else if (e.altKey && e.key === 'ArrowUp') move(i, i - 1);
    else if (e.altKey && e.key === 'ArrowDown') move(i, i + 1);
    else if (e.key === 'ArrowUp' && field.selectionStart === 0 && i > 0) focusItem(list.items[i - 1].id, true);
    else if (e.key === 'ArrowDown' && field.selectionEnd === field.value.length && i < list.items.length - 1)
      focusItem(list.items[i + 1].id);
    else handled = false;
    if (handled) e.preventDefault();
  }

  function menu(e: MouseEvent, i: number) {
    // In Textfeldern bleibt das Menü des Systems, daneben das der Liste.
    if ((e.target as HTMLElement).closest('textarea')) return;
    e.preventDefault();
    e.stopPropagation();
    openMenu({ x: e.clientX, y: e.clientY }, [
      { label: 'Punkt darüber einfügen', action: () => insert(i) },
      { label: 'Punkt darunter einfügen', hint: 'Eingabe', action: () => insert(i + 1) },
      { label: 'Nach oben', hint: 'Alt+↑', disabled: i === 0, action: () => move(i, i - 1) },
      { label: 'Nach unten', hint: 'Alt+↓', disabled: i === list.items.length - 1, action: () => move(i, i + 1) },
      { label: 'Punkt entfernen', danger: true, action: () => remove(i) },
    ]);
  }
</script>

<svelte:element this={list.ordered ? 'ol' : 'ul'} class="list" bind:this={root}>
  {#each list.items as item, i (item.id)}
    <li oncontextmenu={(e) => menu(e, i)}>
      <EditableText
        multiline
        value={item.text}
        label="Punkt {i + 1}"
        placeholder="Punkt"
        data-item={item.id}
        onCommit={(v: string) => setText(item.id, v)}
        onKey={(e: KeyboardEvent) => key(e, i)}
      />
    </li>
  {:else}
    <li class="empty"><button onclick={() => insert(0)}>Noch keine Punkte – klicken, um den ersten anzulegen</button></li>
  {/each}
</svelte:element>

<style>
  .list {
    margin: 2px 0 10px;
    padding-left: 26px;
  }
  li {
    padding-left: 2px;
    line-height: 1.5;
  }
  li::marker {
    color: var(--muted);
  }
  .empty {
    list-style: none;
    margin-left: -26px;
  }
  .empty button {
    font: inherit;
    font-family: var(--ui);
    font-size: 12px;
    color: var(--muted);
    background: none;
    border: none;
    padding: 6px 0;
    cursor: pointer;
  }
  .empty button:hover {
    color: var(--accent-strong);
  }
</style>
