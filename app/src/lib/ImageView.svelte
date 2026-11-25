<script lang="ts">
  // Bildfläche: zeigt das Bild. Klick, Ablegen oder Einfügen ersetzt es.
  import { openMenu } from './menus.svelte';
  import { resourceUrl } from './resources';
  import type { Command, Doc } from './api';

  type Props = {
    part: string;
    field: string;
    value: string | null;
    /** Anteil der Zeilenbreite, wie im PDF. */
    width: number;
    onCommand: (cmd: Command) => Promise<Doc | undefined>;
    onImage: (part: string, field: string, file: File) => void;
  };
  let { part, field, value, width, onCommand, onImage }: Props = $props();

  let url = $state<string | null>(null);
  let missing = $state(false);
  let over = $state(false);
  let picker = $state<HTMLInputElement>();

  let name = $derived(value?.split('/').pop() ?? '');

  $effect(() => {
    const path = value;
    url = null;
    missing = false;
    if (!path) return;
    let alive = true;
    resourceUrl(path).then((u) => {
      if (!alive) return;
      url = u;
      missing = u === null;
    });
    return () => {
      alive = false;
    };
  });

  const hasFile = (e: DragEvent) => !!e.dataTransfer && Array.from(e.dataTransfer.types).includes('Files');

  function take(files: FileList | null | undefined) {
    const file = files?.[0];
    if (file) onImage(part, field, file);
  }

  function remove() {
    if (value) onCommand({ op: 'clear_value', part, path: field });
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      picker?.click();
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      e.stopPropagation();
      remove();
    }
  }

  function menu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    openMenu({ x: e.clientX, y: e.clientY }, [
      { heading: value ? name : 'Bild' },
      { label: value ? 'Bild ersetzen…' : 'Bild wählen…', hint: 'Eingabe', action: () => picker?.click() },
      { label: 'Bild entfernen', hint: 'Entf', danger: true, disabled: !value, action: remove },
    ]);
  }
</script>

<div class="image" style:width="{Math.round(width * 100)}%">
  <div
    class="frame"
    class:empty={!url}
    class:over
    role="button"
    tabindex="0"
    aria-label={value ? `Bild ${name}. Eingabetaste ersetzt es, Entf entfernt es.` : 'Bild wählen'}
    title={value ? `${name} – klicken oder Bild hierher ziehen, um es zu ersetzen` : undefined}
    onclick={() => picker?.click()}
    onkeydown={key}
    oncontextmenu={menu}
    onpaste={(e) => take(e.clipboardData?.files)}
    ondragenter={(e) => hasFile(e) && (over = true)}
    ondragover={(e) => {
      if (!hasFile(e)) return;
      e.preventDefault();
      e.dataTransfer!.dropEffect = 'copy';
      over = true;
    }}
    ondragleave={(e) => {
      if (!(e.relatedTarget instanceof Node && e.currentTarget.contains(e.relatedTarget))) over = false;
    }}
    ondrop={(e) => {
      e.preventDefault();
      over = false;
      take(e.dataTransfer?.files);
    }}
  >
    {#if url}
      <img src={url} alt={name} draggable="false" />
    {:else}
      <div class="hint">
        <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
          <rect x="3" y="5" width="18" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="1.5" />
          <circle cx="8.5" cy="10" r="1.6" fill="currentColor" />
          <path d="M4 17l5-5 4 4 3-3 4 4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
        </svg>
        {#if value && missing}
          <span>Bilddatei fehlt: {value}</span>
        {:else if value}
          <span>Bild wird geladen…</span>
        {:else}
          <span>Bild hierher ziehen oder klicken</span>
        {/if}
      </div>
    {/if}
    {#if over}<div class="drop">Bild hier ablegen</div>{/if}
  </div>
  <input
    bind:this={picker}
    type="file"
    accept=".png,.jpg,.jpeg,.pdf,image/png,image/jpeg,application/pdf"
    hidden
    tabindex="-1"
    onchange={(e) => {
      take(e.currentTarget.files);
      e.currentTarget.value = '';
    }}
  />
</div>

<style>
  .image {
    max-width: 100%;
    margin: 4px 0 10px;
  }
  .frame {
    position: relative;
    display: grid;
    place-items: center;
    border-radius: 6px;
    cursor: pointer;
    outline: 1px solid transparent;
    outline-offset: 3px;
    overflow: hidden;
    transition: outline-color 0.1s;
  }
  .frame:hover {
    outline-color: var(--line-strong);
  }
  .frame:focus-visible,
  .frame.over {
    outline: 2px solid var(--accent);
  }
  .frame.empty {
    min-height: 120px;
    border: 1.5px dashed var(--line-strong);
    background: var(--panel-2);
  }
  .frame.empty:hover {
    border-color: var(--accent);
    outline-color: transparent;
  }
  img {
    display: block;
    width: 100%;
    max-height: 420px;
    object-fit: contain;
    user-select: none;
  }
  .hint {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 14px;
    color: var(--muted);
    font-family: var(--ui);
    font-size: 12px;
    text-align: center;
    word-break: break-word;
  }
  .drop {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 16%, var(--panel));
    color: var(--accent-strong);
    font-family: var(--ui);
    font-size: 13px;
    font-weight: 600;
    pointer-events: none;
  }
</style>
