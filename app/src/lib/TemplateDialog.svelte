<script lang="ts">
  // Dokument als Vorlage speichern: Name, Beschreibung, Eingaben behalten.
  import { tick } from 'svelte';

  type Props = {
    open: boolean;
    title: string;
    dir: string | null;
    onSave: (title: string, description: string, keepValues: boolean) => void;
  };
  let { open = $bindable(), title, dir, onSave }: Props = $props();

  let name = $state('');
  let description = $state('');
  let keep = $state(false);
  let input = $state<HTMLInputElement>();

  $effect(() => {
    if (!open) return;
    name = title;
    description = '';
    keep = false;
    tick().then(() => input?.select());
  });

  function save() {
    if (name.trim()) onSave(name.trim(), description.trim(), keep);
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (open = false)}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="tpl-title"
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') open = false;
        if (e.key === 'Enter' && (e.target as HTMLElement).tagName === 'INPUT') save();
        e.stopPropagation();
      }}
    >
      <h2 id="tpl-title">Als Vorlage speichern</h2>
      <p class="lead">
        Die Vorlage übernimmt Seitenlayout, Module und Reihenfolge dieses Dokuments. Neue Dokumente beginnen damit, über
        Datei → Neu aus Vorlage.
      </p>
      <label>
        <span>Name</span>
        <input bind:this={input} bind:value={name} placeholder="z. B. Prüfbericht Übertrager" />
      </label>
      <label>
        <span>Beschreibung</span>
        <input bind:value={description} placeholder="Wofür ist die Vorlage gedacht?" />
      </label>
      <div class="choice" role="radiogroup" aria-label="Eingaben">
        <button role="radio" aria-checked={!keep} class:on={!keep} onclick={() => (keep = false)}>
          <strong>Eingaben leeren</strong>
          <span>Neue Dokumente zeigen die Vorgaben der Module. Keine alten Inhalte in neuen Dokumenten.</span>
        </button>
        <button role="radio" aria-checked={keep} class:on={keep} onclick={() => (keep = true)}>
          <strong>Inhalte behalten</strong>
          <span>Alles bleibt wie hier, mit Bildern. Gut für Beispiele und fast fertige Dokumente.</span>
        </button>
      </div>
      {#if dir}<p class="where">Gespeichert im Bibliotheksordner {dir}</p>{/if}
      <div class="buttons">
        <button class="ghost" onclick={() => (open = false)}>Abbrechen</button>
        <button class="primary" disabled={!name.trim()} onclick={save}>Vorlage speichern</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 30, 0.35);
    display: grid;
    place-items: center;
    z-index: 100;
    padding: 16px;
  }
  .sheet {
    display: grid;
    gap: 12px;
    width: min(560px, 100%);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 20px 22px 16px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    outline: none;
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .lead,
  .where {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
    line-height: 1.5;
  }
  .where {
    font-size: 11px;
    color: var(--muted);
    word-break: break-all;
  }
  label {
    display: grid;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  input {
    font: inherit;
    font-size: 14px;
    color: var(--text);
    background: var(--panel-2);
    border: 1px solid var(--line-strong);
    border-radius: 7px;
    padding: 7px 10px;
  }
  .choice {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .choice button {
    display: grid;
    gap: 4px;
    text-align: left;
    font: inherit;
    color: var(--text);
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 9px;
    padding: 10px 12px;
    cursor: pointer;
  }
  .choice button.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .choice span {
    font-size: 11.5px;
    color: var(--text-2);
    line-height: 1.45;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
