// Nachschlagewerk der LaTeX-Befehle: Formavium-Befehle, Befehle, die das
// offene Dokument selbst definiert (Vorlage, Module), und häufige
// LaTeX-Befehle. Dient der Hilfe und der Vervollständigung im Quelltext.

export type RefEntry = {
  name: string;
  /** Schreibweise mit Platzhaltern, z. B. `\feld{kennung}`. */
  usage: string;
  /** Einfügetext; `#` markiert die Stelle der Einfügemarke. */
  insert: string;
  info: string;
  group: 'formavium' | 'dokument' | 'latex';
};

export const formaviumCommands: RefEntry[] = [
  {
    name: 'feld',
    usage: '\\feld{kennung}',
    insert: '\\feld{#}',
    info: 'Eingabefeld für Text. Der Anwender füllt es im visuellen Editor aus; Sonderzeichen wie & oder % werden automatisch maskiert. Nur in Modulen.',
    group: 'formavium',
  },
  {
    name: 'rohfeld',
    usage: '\\rohfeld{kennung}',
    insert: '\\rohfeld{#}',
    info: 'Eingabefeld für LaTeX: Der Wert wird unverändert übernommen, etwa für Formeln, Indizes oder Farben. Nur in Modulen.',
    group: 'formavium',
  },
  {
    name: 'bildfeld',
    usage: '\\bildfeld[optionen]{kennung}',
    insert: '\\bildfeld[width=\\linewidth]{#}',
    info: 'Bildfeld: wird zu \\includegraphics[optionen]{pfad}. Ohne gewähltes Bild steht ein Platzhalterkasten. Nur in Modulen.',
    group: 'formavium',
  },
];

export const latexCommands: RefEntry[] = [
  ['section', '\\section{Titel}', '\\section{#}', 'Nummerierte Überschrift, Ebene 1'],
  ['subsection', '\\subsection{Titel}', '\\subsection{#}', 'Überschrift, Ebene 2'],
  ['subsubsection', '\\subsubsection{Titel}', '\\subsubsection{#}', 'Überschrift, Ebene 3'],
  ['paragraph', '\\paragraph{Titel}', '\\paragraph{#}', 'Fettes Stichwort am Absatzanfang'],
  ['textbf', '\\textbf{Text}', '\\textbf{#}', 'Fett'],
  ['textit', '\\textit{Text}', '\\textit{#}', 'Kursiv'],
  ['emph', '\\emph{Text}', '\\emph{#}', 'Hervorgehoben (meist kursiv)'],
  ['underline', '\\underline{Text}', '\\underline{#}', 'Unterstrichen'],
  ['texttt', '\\texttt{Text}', '\\texttt{#}', 'Schreibmaschinenschrift'],
  ['textcolor', '\\textcolor{farbe}{Text}', '\\textcolor{#}{}', 'Farbiger Text (Paket xcolor)'],
  ['textsubscript', '\\textsubscript{x}', '\\textsubscript{#}', 'Tiefgestellt im Fließtext'],
  ['textsuperscript', '\\textsuperscript{x}', '\\textsuperscript{#}', 'Hochgestellt im Fließtext'],
  ['small', '\\small', '\\small', 'Kleinere Schrift ab hier'],
  ['large', '\\large', '\\large', 'Größere Schrift ab hier'],
  ['par', '\\par', '\\par', 'Absatzende'],
  ['newline', '\\newline', '\\newline', 'Zeilenumbruch im Absatz'],
  ['newpage', '\\newpage', '\\newpage', 'Neue Seite'],
  ['vspace', '\\vspace{Länge}', '\\vspace{#}', 'Senkrechter Abstand, z. B. 5mm'],
  ['hspace', '\\hspace{Länge}', '\\hspace{#}', 'Waagerechter Abstand'],
  ['hfill', '\\hfill', '\\hfill', 'Schiebt Folgendes an den rechten Rand'],
  ['centering', '\\centering', '\\centering', 'Zentriert den Rest der Umgebung'],
  ['includegraphics', '\\includegraphics[width=…]{bild}', '\\includegraphics[width=\\linewidth]{#}', 'Bild einbinden (Paket graphicx). Bilder liegen im Ordner bilder/ neben dem Dokument.'],
  ['caption', '\\caption{Text}', '\\caption{#}', 'Bild- oder Tabellenunterschrift'],
  ['label', '\\label{name}', '\\label{#}', 'Sprungmarke für Verweise'],
  ['ref', '\\ref{name}', '\\ref{#}', 'Verweis auf eine Sprungmarke'],
  ['pageref', '\\pageref{name}', '\\pageref{#}', 'Seitenzahl einer Sprungmarke'],
  ['footnote', '\\footnote{Text}', '\\footnote{#}', 'Fußnote'],
  ['url', '\\url{adresse}', '\\url{#}', 'Internetadresse (Paket hyperref)'],
  ['href', '\\href{adresse}{Text}', '\\href{#}{}', 'Verweis mit Text (Paket hyperref)'],
  ['hline', '\\hline', '\\hline', 'Waagerechte Linie in einer Tabelle'],
  ['cline', '\\cline{a-b}', '\\cline{#}', 'Linie über die Spalten a bis b'],
  ['multicolumn', '\\multicolumn{n}{|l|}{Text}', '\\multicolumn{#}{|l|}{}', 'Zelle über n Spalten'],
  ['multirow', '\\multirow{n}{*}{Text}', '\\multirow{#}{*}{}', 'Zelle über n Zeilen (Paket multirow)'],
  ['item', '\\item', '\\item ', 'Punkt einer Liste'],
  ['frac', '\\frac{a}{b}', '\\frac{#}{}', 'Bruch (Mathematik)'],
  ['sqrt', '\\sqrt{x}', '\\sqrt{#}', 'Wurzel (Mathematik)'],
  ['cdot', '\\cdot', '\\cdot', 'Malpunkt (Mathematik)'],
  ['times', '\\times', '\\times', 'Malkreuz (Mathematik)'],
  ['pm', '\\pm', '\\pm', 'Plusminus (Mathematik)'],
  ['Omega', '\\Omega', '\\Omega', 'Ohm-Zeichen (Mathematik)'],
  ['mu', '\\mu', '\\mu', 'Mikro (Mathematik)'],
  ['degree', '^\\circ', '^\\circ', 'Grad (Mathematik); im Text: \\textdegree'],
  ['textdegree', '\\textdegree', '\\textdegree', 'Gradzeichen im Text'],
  ['usepackage', '\\usepackage{paket}', '\\usepackage{#}', 'Paket laden (nur im Vorspann). Offline gehen nur Pakete, die Formavium mitbringt.'],
  ['newcommand', '\\newcommand{\\name}[n]{…}', '\\newcommand{\\#}{}', 'Eigenen Befehl definieren'],
  ['providecommand', '\\providecommand{\\name}{…}', '\\providecommand{\\#}{}', 'Befehl definieren, falls er noch fehlt (gut für Module)'],
  ['renewcommand', '\\renewcommand{\\name}{…}', '\\renewcommand{\\#}{}', 'Vorhandenen Befehl neu definieren'],
  ['setlength', '\\setlength{\\länge}{wert}', '\\setlength{\\#}{}', 'Länge setzen, z. B. \\parindent'],
  ['begin', '\\begin{umgebung} … \\end{umgebung}', '\\begin{#}\n\n\\end{}', 'Umgebung beginnen'],
].map(([name, usage, insert, info]) => ({ name, usage, insert, info, group: 'latex' as const }));

export const environments = [
  ['itemize', 'Aufzählung'],
  ['enumerate', 'Nummerierte Liste'],
  ['tabular', 'Tabelle'],
  ['center', 'Zentriert'],
  ['figure', 'Gleitende Abbildung'],
  ['table', 'Gleitende Tabelle'],
  ['minipage', 'Kasten mit fester Breite'],
  ['equation', 'Nummerierte Formel'],
  ['align', 'Mehrzeilige Formel (amsmath)'],
  ['quote', 'Eingerücktes Zitat'],
  ['description', 'Liste mit Stichworten'],
];

/** Befehle, die ein Quelltext selbst definiert (Vorlage, Module). */
export function definedCommands(source: string): RefEntry[] {
  const out = new Map<string, RefEntry>();
  const re = /\\(newcommand|renewcommand|providecommand|DeclareRobustCommand)\*?\s*\{?\\([A-Za-z@]+)\}?\s*(?:\[(\d)\])?(?:\[[^\]]*\])?/g;
  for (let m = re.exec(source); m; m = re.exec(source)) {
    const name = m[2];
    if (name.includes('@') || out.has(name)) continue;
    const n = Number(m[3] ?? 0);
    const args = Array.from({ length: n }, (_, i) => `{${['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i'][i]}}`).join('');
    // Kommentar direkt über der Definition als Erklärung.
    const before = source.slice(0, m.index).split('\n');
    const comment = before.length > 1 && /^\s*%/.test(before[before.length - 2] ?? '') ? before[before.length - 2].replace(/^\s*%+\s*/, '') : '';
    out.set(name, {
      name,
      usage: `\\${name}${args}`,
      insert: n ? `\\${name}{#}${'{}'.repeat(n - 1)}` : `\\${name}`,
      info: comment || (n ? `Eigener Befehl mit ${n} ${n === 1 ? 'Angabe' : 'Angaben'}` : 'Eigener Befehl'),
      group: 'dokument',
    });
  }
  return [...out.values()].sort((a, b) => a.name.localeCompare(b.name));
}
