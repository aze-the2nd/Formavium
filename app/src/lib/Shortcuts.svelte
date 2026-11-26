<script lang="ts">
  // Übersicht der Tastenkürzel (F1).
  type Props = { open: boolean };
  let { open = $bindable() }: Props = $props();

  let box = $state<HTMLDivElement>();
  $effect(() => {
    if (open) box?.focus();
  });

  const groups: { title: string; keys: [string, string][] }[] = [
    {
      title: 'Dokument',
      keys: [
        ['Strg+K', 'Befehlspalette: jeder Befehl, jedes Modul, jede Vorlage'],
        ['Strg+N', 'Neues Dokument aus einer Vorlage'],
        ['Strg+O', 'Dokument öffnen'],
        ['Strg+S', 'Speichern'],
        ['Strg+P', 'Als PDF speichern'],
        ['Strg+Umschalt+E', 'LaTeX exportieren'],
        ['Strg+W', 'Dokument schließen'],
        ['Strg+Z / Strg+Y', 'Rückgängig / Wiederholen'],
        ['Strg+Umschalt+F', 'Feedback senden'],
      ],
    },
    {
      title: 'Module',
      keys: [
        ['Strg+M', 'Modul aus der Bibliothek einfügen'],
        ['Doppelklick auf den Modulkopf', 'Modul-Werkstatt öffnen'],
        ['Esc in der Werkstatt', 'Werkstatt schließen'],
        ['Rechtsklick in der Auswahl', 'Eigenen Eintrag löschen'],
      ],
    },
    {
      title: 'Liste',
      keys: [
        ['Eingabe', 'Nächster Punkt'],
        ['Rücktaste im leeren Punkt', 'Punkt entfernen'],
        ['Alt+↑ / Alt+↓', 'Punkt verschieben'],
      ],
    },
    {
      title: 'Ansichten',
      keys: [
        ['Strg+1 / 2 / 3', 'Quelltext / Visuell / PDF ein- und ausblenden'],
        ['Griff am Kachelkopf ziehen', 'Ansichten umordnen'],
        ['Trennlinie ziehen', 'Breite ändern, Doppelklick gleicht aus'],
      ],
    },
    {
      title: 'Abschnitte',
      keys: [
        ['Strg+↑ / Strg+↓', 'Abschnitt verschieben'],
        ['Esc', 'Feld verlassen, Abschnitt fokussieren'],
        ['↑ / ↓', 'Zum vorigen / nächsten Abschnitt'],
        ['Eingabe', 'Ins erste Feld des Abschnitts'],
        ['Entf', 'Fokussierten Abschnitt entfernen'],
        ['Rechtsklick oder Menütaste', 'Aktionen an der Stelle'],
      ],
    },
    {
      title: 'Tabelle',
      keys: [
        ['Pfeiltasten', 'Zelle wählen'],
        ['Umschalt+Pfeil oder Ziehen', 'Bereich markieren'],
        ['Tippen', 'Zelle überschreiben'],
        ['Eingabe / F2 / Doppelklick', 'Zelle bearbeiten'],
        ['Eingabe beim Bearbeiten', 'Übernehmen, eine Zeile tiefer, am Ende neue Zeile'],
        ['Tab / Umschalt+Tab', 'Nächste / vorige Zelle'],
        ['Entf', 'Markierte Zellen leeren'],
        ['Strg+C / Strg+X / Strg+V', 'Bereich kopieren, ausschneiden, einsetzen (auch aus Excel)'],
        ['Strg+Eingabe', 'Zeile darunter einfügen'],
        ['Strg+Umschalt+Eingabe', 'Zeile darüber einfügen'],
        ['Strg+Umschalt+Rück', 'Zeilen löschen'],
        ['Alt+↑ / Alt+↓', 'Zeile verschieben'],
        ['Alt+← / Alt+→', 'Spalte verschieben'],
        ['Esc', 'Tabelle verlassen, Abschnitt fokussieren'],
        ['Rechtsklick', 'Zellen verbinden, Spalte als LaTeX, Ausrichtung'],
      ],
    },
    {
      title: 'PDF und Quelltext',
      keys: [
        ['Klick ins PDF', 'Zur Stelle im Editor springen'],
        ['Strg+Mausrad im PDF', 'Zoomen'],
        ['\\ im Quelltext', 'LaTeX-Befehle vorschlagen (Strg+Leertaste jederzeit)'],
        ['Strg+Umschalt+L', 'LaTeX-Befehle nachschlagen'],
      ],
    },
    {
      title: 'Bild',
      keys: [
        ['Eingabe', 'Bild wählen'],
        ['Strg+V', 'Bild aus der Zwischenablage einfügen'],
        ['Entf', 'Bild entfernen'],
      ],
    },
  ];
</script>

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (open = false)}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="keys-title"
      tabindex="-1"
      bind:this={box}
      onkeydown={(e) => {
        if (e.key === 'Escape' || e.key === 'F1') {
          e.preventDefault();
          e.stopPropagation();
          open = false;
        }
      }}
    >
      <h2 id="keys-title">Tastenkürzel</h2>
      <div class="groups">
        {#each groups as g}
          <section>
            <h3>{g.title}</h3>
            <dl>
              {#each g.keys as [key, what]}
                <dt>{key}</dt>
                <dd>{what}</dd>
              {/each}
            </dl>
          </section>
        {/each}
      </div>
      <div class="buttons">
        <button class="ghost small" onclick={() => (open = false)}>Schließen</button>
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
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 20px 24px 16px;
    width: min(860px, 100%);
    max-height: 100%;
    overflow: auto;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    outline: none;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 16px;
  }
  .groups {
    columns: 2 340px;
    column-gap: 32px;
  }
  section {
    break-inside: avoid;
    margin-bottom: 14px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 14px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    font-weight: 600;
    white-space: nowrap;
  }
  dd {
    margin: 0;
    color: var(--text-2);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
  }
</style>
