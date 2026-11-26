<script lang="ts">
  // Erste Schritte: kurze Anleitungen mit direktem Einstieg in die Aktion.
  type Action = 'template' | 'module' | 'save-template' | 'free' | 'library' | 'ref';
  type Props = { open: boolean; hasDoc: boolean; onAction: (a: Action) => void };
  let { open = $bindable(), hasDoc, onAction }: Props = $props();

  const guides: { title: string; steps: string[]; action: Action; label: string; needsDoc?: boolean }[] = [
    {
      title: 'Ein Dokument ausfüllen',
      steps: [
        'Datei → Neu aus Vorlage (Strg+N) und eine Vorlage wählen.',
        'Im visuellen Editor direkt in Überschriften, Texte und Tabellen tippen. Bilder anklicken oder hineinziehen.',
        'Ein Klick im PDF springt zur passenden Stelle im Editor.',
        'Datei → Als PDF speichern (Strg+P).',
      ],
      action: 'template',
      label: 'Vorlage wählen',
    },
    {
      title: 'Ein eigenes Modul bauen',
      steps: [
        'Einfügen → Neues Modul erstellen. Das Modul entsteht im Dokument, rechts öffnet sich die Modul-Werkstatt.',
        'Bausteine hinzufügen: Überschrift, Text, Liste, Tabelle, Bild oder LaTeX. Jede Änderung siehst du sofort im PDF.',
        'Was der Anwender später ausfüllen soll, wird ein Eingabefeld: bei Überschrift und Text „Eingabefeld“ einschalten, im LaTeX Text markieren und „zum Eingabefeld machen“.',
        '„In Bibliothek speichern“: Ab dann steht das Modul unter Strg+M in jedem Dokument bereit.',
      ],
      action: 'module',
      label: 'Neues Modul erstellen',
      needsDoc: true,
    },
    {
      title: 'Vorhandenes LaTeX zum Modul machen',
      steps: [
        'LaTeX als freien Abschnitt einfügen (Rechtsklick → LaTeX-Abschnitt) und hineinkopieren.',
        'Rechtsklick auf den Abschnitt → „In Modul umwandeln“.',
        'In der Werkstatt Texte markieren und zu Eingabefeldern machen.',
        'Eigene Befehle des LaTeX in den Vorspann des Moduls, dann funktioniert es überall.',
      ],
      action: 'ref',
      label: 'LaTeX-Befehle ansehen',
    },
    {
      title: 'Eine Vorlage anlegen',
      steps: [
        'Ein Dokument so aufbauen, wie neue Dokumente beginnen sollen: Seitenlayout, Module, Reihenfolge.',
        'Datei → Als Vorlage speichern. Auf Wunsch werden die Eingaben geleert, damit keine alten Inhalte in neue Dokumente gelangen.',
        'Die Vorlage erscheint unter Strg+N. Sie liegt mit ihren Bildern im Bibliotheksordner und lässt sich weitergeben.',
      ],
      action: 'save-template',
      label: 'Dieses Dokument als Vorlage',
      needsDoc: true,
    },
    {
      title: 'Frei in LaTeX schreiben (Nerd-Modus)',
      steps: [
        'Ein freies LaTeX-Dokument ist reiner Quelltext mit PDF-Vorschau, ohne Module und ohne Schutz.',
        'Neu: Strg+K → „Neues freies LaTeX-Dokument“. Aus einem geführten Dokument: Datei → Als freies LaTeX weiterführen (eine Kopie).',
        'Die Vervollständigung kennt LaTeX-Befehle und die Befehle deiner Vorlagen.',
      ],
      action: 'free',
      label: 'Freies Dokument anlegen',
    },
  ];
</script>

{#if open}
  <div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && (open = false)}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="guide-title"
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
        <h2 id="guide-title">Erste Schritte</h2>
        <button class="ghost small" onclick={() => (open = false)}>Schließen</button>
      </header>
      <div class="grid">
        {#each guides as g, i}
          <article>
            <h3><span class="n">{i + 1}</span>{g.title}</h3>
            <ol>
              {#each g.steps as step}<li>{step}</li>{/each}
            </ol>
            <button class="primary small" disabled={g.needsDoc && !hasDoc} onclick={() => onAction(g.action)}>{g.label}</button>
            {#if g.needsDoc && !hasDoc}<span class="need">Erst ein Dokument öffnen</span>{/if}
          </article>
        {/each}
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
    width: min(980px, 100%);
    max-height: 90vh;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 18px 22px 20px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
  }
  article {
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 12px 14px 14px;
    background: var(--panel-2);
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 6px;
    font-size: 14px;
  }
  .n {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--accent);
    color: white;
    font-size: 11px;
  }
  ol {
    margin: 0 0 10px;
    padding-left: 18px;
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-2);
  }
  .need {
    margin-left: 8px;
    font-size: 11px;
    color: var(--muted);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
