<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Annotation, EditorState, RangeSetBuilder, StateEffect, StateField } from '@codemirror/state';
  import {
    Decoration,
    type DecorationSet,
    EditorView,
    drawSelection,
    highlightActiveLine,
    highlightActiveLineGutter,
    keymap,
    lineNumbers,
  } from '@codemirror/view';
  import { defaultKeymap, indentWithTab } from '@codemirror/commands';
  import { HighlightStyle, StreamLanguage, bracketMatching, syntaxHighlighting } from '@codemirror/language';
  import { tags as t } from '@lezer/highlight';
  import { stex } from '@codemirror/legacy-modes/mode/stex';
  import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
  import { diff, type Doc } from './api';
  import { autocompletion, completionKeymap, snippet, type Completion, type CompletionContext } from '@codemirror/autocomplete';
  import { definedCommands, environments, latexCommands } from './latexref';

  type Props = {
    doc: Doc;
    errorLines: number[];
    onSourceEdit: (change: { base: number; from: number; to: number; insert: string }) => Promise<Doc>;
    onUndo: () => void;
    onRedo: () => void;
    /** Vom Anwender ausgelöstes Scrollen (für die Synchronisierung). */
    onScroll?: () => void;
  };
  let { doc, errorLines, onSourceEdit, onUndo, onRedo, onScroll }: Props = $props();

  let host: HTMLDivElement;

  // Vervollständigung: Befehle des Dokuments (Vorlage, Module) zuerst,
  // dann häufige LaTeX-Befehle und Umgebungen.
  let docCommands = $derived(definedCommands(doc.source));
  const toApply = (insert: string) => snippet(insert.replace('#', '${}').replace(/\{\}/g, '{${}}'));
  function complete(ctx: CompletionContext) {
    const env = ctx.matchBefore(/\\begin\{[A-Za-z*]*/);
    if (env) {
      return {
        from: env.from + 7,
        options: environments.map(
          ([name, info]): Completion => ({
            label: name,
            detail: info,
            type: 'keyword',
            apply: snippet(`${name}}\n\t\${}\n\\end{${name}}`),
          }),
        ),
      };
    }
    const word = ctx.matchBefore(/\\[A-Za-z]*/);
    if (!word || (word.from === word.to && !ctx.explicit)) return null;
    const options: Completion[] = [
      ...docCommands.map((c) => ({ label: `\\${c.name}`, detail: c.usage, info: c.info, type: 'function', boost: 2, apply: toApply(c.insert) })),
      ...latexCommands.map((c) => ({ label: `\\${c.name}`, detail: c.usage, info: c.info, type: 'keyword', apply: toApply(c.insert) })),
    ];
    return { from: word.from, options, validFor: /^\\[A-Za-z]*$/ };
  }
  let view: EditorView | undefined;

  // Stand des Kerns, auf dem der Editor zuletzt abgeglichen wurde.
  let synced = '';
  let syncedRev = -1;
  let inflight = false;

  const remote = Annotation.define<boolean>();

  // Farben über CSS-Variablen, damit helles und dunkles Theme lesbar bleiben.
  const latexHighlight = HighlightStyle.define([
    { tag: [t.tagName, t.keyword], color: 'var(--hl-command)' },
    { tag: [t.bracket, t.squareBracket, t.brace], color: 'var(--hl-bracket)' },
    { tag: t.comment, color: 'var(--hl-comment)', fontStyle: 'italic' },
    { tag: [t.atom, t.string, t.special(t.string)], color: 'var(--hl-math)' },
    { tag: t.number, color: 'var(--hl-number)' },
    { tag: t.variableName, color: 'var(--hl-env)' },
  ]);

  // Fehlerzeilen hervorheben.
  const setErrors = StateEffect.define<number[]>();
  const errorField = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setErrors)) {
          const b = new RangeSetBuilder<Decoration>();
          const lines = [...new Set(e.value)].sort((x, y) => x - y);
          for (const n of lines) {
            if (n >= 1 && n <= tr.state.doc.lines) {
              const l = tr.state.doc.line(n);
              b.add(l.from, l.from, Decoration.line({ class: 'cm-error-line' }));
            }
          }
          deco = b.finish();
        }
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  /** Editorinhalt auf den Kernstand bringen, mit minimaler Änderung. */
  function replaceWith(text: string) {
    if (!view) return;
    const d = diff(view.state.doc.toString(), text);
    if (d) view.dispatch({ changes: d, annotations: remote.of(true) });
  }

  async function flush() {
    if (!view || inflight) return;
    const current = view.state.doc.toString();
    const d = diff(synced, current);
    if (!d) return;
    inflight = true;
    let result: Doc;
    try {
      result = await onSourceEdit({ base: syncedRev, ...d });
    } catch {
      result = doc;
    }
    synced = result.source;
    syncedRev = result.revision;
    inflight = false;
    if (view.state.doc.toString() === current) {
      // Keine weitere Eingabe: kanonischen Stand übernehmen (z. B. Maskierung).
      replaceWith(synced);
    } else {
      // Weitergetippt: Rest auf Basis des neuen Stands senden.
      flush();
    }
  }

  // Änderungen aus anderen Ansichten, Rückgängig oder Neuladen übernehmen.
  $effect(() => {
    const { revision, source } = doc;
    if (!view || inflight || revision === syncedRev) return;
    synced = source;
    syncedRev = revision;
    replaceWith(source);
  });

  $effect(() => {
    const lines = errorLines;
    view?.dispatch({ effects: setErrors.of(lines) });
  });

  export function jumpTo(offset: number) {
    if (!view) return;
    const pos = Math.min(offset, view.state.doc.length);
    view.dispatch({
      selection: { anchor: pos },
      effects: EditorView.scrollIntoView(pos, { y: 'start', yMargin: 40 }),
    });
    view.focus();
  }

  /** Oberste sichtbare Zeile (für das synchrone Scrollen). */
  export function topLine(): number {
    if (!view) return 1;
    const block = view.lineBlockAtHeight(view.scrollDOM.scrollTop + 4);
    return view.state.doc.lineAt(block.from).number;
  }

  /** Zeile an den oberen Rand scrollen, ohne den Fokus zu nehmen. */
  export function scrollToLine(line: number) {
    if (!view) return;
    const n = Math.max(1, Math.min(Math.round(line), view.state.doc.lines));
    view.dispatch({ effects: EditorView.scrollIntoView(view.state.doc.line(n).from, { y: 'start', yMargin: 4 }) });
  }

  export function jumpToLine(line: number) {
    if (!view) return;
    const n = Math.max(1, Math.min(line, view.state.doc.lines));
    jumpTo(view.state.doc.line(n).from);
  }

  onMount(() => {
    synced = doc.source;
    syncedRev = doc.revision;
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: doc.source,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightActiveLine(),
          drawSelection(),
          bracketMatching(),
          highlightSelectionMatches(),
          StreamLanguage.define(stex),
          syntaxHighlighting(latexHighlight),
          EditorView.lineWrapping,
          errorField,
          autocompletion({ override: [complete], icons: false, activateOnTyping: true }),
          keymap.of([
            ...completionKeymap,
            // Rückgängig wirkt auf das Dokument, nicht nur auf den Editor.
            { key: 'Mod-z', run: () => (onUndo(), true), preventDefault: true },
            { key: 'Mod-y', run: () => (onRedo(), true), preventDefault: true },
            { key: 'Mod-Shift-z', run: () => (onRedo(), true), preventDefault: true },
            ...searchKeymap,
            ...defaultKeymap,
            indentWithTab,
          ]),
          EditorView.updateListener.of((u) => {
            if (u.docChanged && !u.transactions.some((t) => t.annotation(remote))) flush();
          }),
          EditorView.domEventHandlers({ scroll: () => void onScroll?.() }),
        ],
      }),
    });
  });

  onDestroy(() => view?.destroy());
</script>

<div class="source" bind:this={host}></div>

<style>
  .source {
    height: 100%;
    overflow: hidden;
  }
  .source :global(.cm-editor) {
    height: 100%;
    font-size: 13px;
  }
  .source :global(.cm-scroller) {
    font-family: var(--mono);
    line-height: 1.55;
  }
  .source :global(.cm-gutters) {
    background: var(--panel-2);
    border-right: 1px solid var(--line);
    color: var(--muted);
  }
  .source :global(.cm-activeLine),
  .source :global(.cm-activeLineGutter) {
    background: var(--accent-soft);
  }
  .source :global(.cm-error-line) {
    background: var(--danger-soft);
    box-shadow: inset 3px 0 0 var(--danger);
  }
</style>
