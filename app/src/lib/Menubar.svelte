<script lang="ts">
  // Menüleiste: kompakte Titel, die Einträge kommen aus dem Befehlsregister.
  import { groups, menuItems, type Cmd, type Group } from './commands';
  import { lastClosed, menuState, openMenuBelow } from './menus.svelte';

  type Props = { commands: Cmd[] };
  let { commands }: Props = $props();

  const owner = (g: Group) => `menubar:${g}`;
  let open = $derived(menuState.current?.owner);

  function show(el: Element, g: Group) {
    openMenuBelow(el, menuItems(commands, g), owner(g));
  }

  function press(e: PointerEvent, g: Group) {
    if (e.button !== 0) return;
    // Der Klick hat das offene Menü dieses Titels gerade geschlossen.
    if (lastClosed.owner === owner(g) && performance.now() - lastClosed.at < 80) return;
    show(e.currentTarget as Element, g);
  }

  /** Bei offenem Menü wechselt das Überfahren zum Nachbarmenü. */
  function enter(e: PointerEvent, g: Group) {
    if (open?.startsWith('menubar:') && open !== owner(g)) show(e.currentTarget as Element, g);
  }

  function key(e: KeyboardEvent, g: Group) {
    if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowDown') {
      e.preventDefault();
      show(e.currentTarget as Element, g);
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      const buttons = Array.from((e.currentTarget as Element).parentElement!.querySelectorAll('button'));
      const i = buttons.indexOf(e.currentTarget as HTMLButtonElement);
      buttons[(i + (e.key === 'ArrowRight' ? 1 : buttons.length - 1)) % buttons.length].focus();
    }
  }
</script>

<nav class="menubar" aria-label="Menü">
  {#each groups as g}
    <button
      class:open={open === owner(g)}
      aria-haspopup="menu"
      aria-expanded={open === owner(g)}
      onmousedown={(e) => e.preventDefault()}
      onpointerdown={(e) => press(e, g)}
      onpointerenter={(e) => enter(e, g)}
      onkeydown={(e) => key(e, g)}>{g}</button
    >
  {/each}
</nav>

<style>
  .menubar {
    display: flex;
    gap: 1px;
  }
  button {
    font: inherit;
    font-size: 13px;
    padding: 5px 9px;
    border-radius: 6px;
    border: none;
    background: none;
    color: var(--text);
    cursor: default;
    white-space: nowrap;
  }
  button:hover,
  button.open {
    background: var(--panel-2);
  }
  button.open {
    box-shadow: inset 0 0 0 1px var(--line);
  }
</style>
