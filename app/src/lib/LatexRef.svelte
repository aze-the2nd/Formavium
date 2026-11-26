<script lang="ts">
  // Nachschlagewerk: Formavium-Befehle, Befehle des offenen Dokuments
  // (aus Vorlage und Modulen) und häufige LaTeX-Befehle. Klick kopiert.
  import { tick } from 'svelte';
  import { score } from './commands';
  import { definedCommands, formaviumCommands, latexCommands, type RefEntry } from './latexref';
  import { notify } from './toast.svelte';

  type Props = { open: boolean; source: string };
  let { open = $bindable(), source }: Props = $props();

  let query = $state('');
  let input = $state<HTMLInputElement>();

  $effect(() => {
    if (open) {
      query = '';
      tick().then(() => input?.focus());
    }
  });

  let own = $derived(open ? definedCommands(source) : []);
  const filter = (list: RefEntry[]) =>
    query.trim() ? list.filter((e) => score(query, `${e.name} ${e.usage} ${e.info}`) >= 0) : list;
  let sections = $derived([
    {
      title: 'Formavium-Befehle',
      note: 'Nur im LaTeX eines Moduls (Modul-Werkstatt). Sie machen eine Stelle zum Eingabefeld, das der Anwender im visuellen Editor ausfüllt.',
      list: filter(formaviumCommands),
    },
    {
      title: 'Befehle dieses Dokuments',
      note: own.length
        ? 'Definiert im Vorspann der Vorlage oder von den Modulen mitgebracht. Ein Kommentar direkt über der Definition erscheint hier als Erklärung.'
        : 'Das offene Dokument definiert keine eigenen Befehle.',
      list: filter(own),
    },
    {
      title: 'Häufige LaTeX-Befehle',
      note: 'Eine kleine Auswahl. Die vollständige Referenz der LaTeX-Befehle folgt.',
      list: filter(latexCommands),
    },
  ]);

  async function copy(e: RefEntry) {
    try {
      await navigator.clipboard.writeText(e.usage);
      notify('info', `${e.usage} kopiert`);
    } catch {
      notify('info', e.usage);
    }
  }
</script>

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (open = false)}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="ref-title"
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault();
          open = false;
        }
        e.stopPropagation();
      }}
    >
      <header>
        <h2 id="ref-title">LaTeX-Befehle</h2>
        <input bind:this={input} bind:value={query} placeholder="Befehl suchen …" aria-label="Befehl suchen" spellcheck="false" />
      </header>
      <div class="scroll">
        {#each sections as sec}
          <section>
            <h3>{sec.title}</h3>
            <p class="note">{sec.note}</p>
            {#if sec.list.length}
              <dl>
                {#each sec.list as e (e.group + e.name)}
                  <dt>
                    <button title="Kopieren" onclick={() => copy(e)}><code>{e.usage}</code></button>
                  </dt>
                  <dd>{e.info}</dd>
                {/each}
              </dl>
            {/if}
          </section>
        {/each}
        <section class="tip">
          <h3>Tipps</h3>
          <ul>
            <li>Im Quelltext schlägt die Vervollständigung Befehle vor, sobald du <code>\</code> tippst. Strg+Leertaste öffnet sie jederzeit.</li>
            <li>Eigene Befehle für viele Dokumente gehören in den Vorspann eines Moduls (Modul-Werkstatt → Vorspann), definiert mit <code>\providecommand</code>. Dann funktioniert das Modul in jedem Dokument.</li>
            <li>Offline stehen nur LaTeX-Pakete zur Verfügung, die Formavium mitbringt.</li>
          </ul>
        </section>
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
    display: flex;
    flex-direction: column;
    width: min(820px, 100%);
    max-height: 86vh;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    outline: none;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 20px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  header input {
    flex: 1;
    font: inherit;
    color: var(--text);
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 6px 10px;
  }
  .scroll {
    overflow: auto;
    padding: 6px 20px 18px;
  }
  h3 {
    margin: 16px 0 2px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .note {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-2);
  }
  dl {
    display: grid;
    grid-template-columns: minmax(180px, max-content) 1fr;
    gap: 4px 16px;
    margin: 0;
    font-size: 13px;
  }
  dt button {
    font: inherit;
    background: none;
    border: none;
    padding: 0;
    cursor: copy;
    text-align: left;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--accent-strong);
    background: var(--accent-soft);
    border-radius: 4px;
    padding: 1px 5px;
  }
  dd {
    margin: 0;
    color: var(--text-2);
  }
  .tip ul {
    margin: 4px 0 0;
    padding-left: 18px;
    font-size: 12.5px;
    color: var(--text-2);
    line-height: 1.6;
  }
</style>
