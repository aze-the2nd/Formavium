// Vorschau für LaTeX in Tabellenzellen: ein kleiner, sicherer Ausschnitt
// (Indizes, Exponenten, häufige Zeichen, fett, kursiv, Farbe). Das PDF
// bleibt maßgeblich; die Vorschau soll nur lesbar machen, was dort steht.

const symbols: Record<string, string> = {
  Omega: 'Ω',
  omega: 'ω',
  mu: 'µ',
  textmu: 'µ',
  pm: '±',
  textpm: '±',
  times: '×',
  cdot: '·',
  ldots: '…',
  dots: '…',
  degree: '°',
  textdegree: '°',
  circ: '°',
  alpha: 'α',
  beta: 'β',
  gamma: 'γ',
  delta: 'δ',
  Delta: 'Δ',
  eta: 'η',
  phi: 'φ',
  pi: 'π',
  sigma: 'σ',
  tau: 'τ',
  theta: 'θ',
  lambda: 'λ',
  varepsilon: 'ε',
  epsilon: 'ε',
  infty: '∞',
  approx: '≈',
  le: '≤',
  leq: '≤',
  ge: '≥',
  geq: '≥',
  neq: '≠',
  rightarrow: '→',
  to: '→',
  Rightarrow: '⇒',
  textohm: 'Ω',
  euro: '€',
  '%': '%',
  '&': '&amp;',
  _: '_',
  '#': '#',
  $: '$',
};

const escapeHtml = (s: string) =>
  s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

const colors: Record<string, string> = {
  red: '#d33',
  green: '#2a8',
  blue: '#36c',
  orange: '#e80',
  gray: '#888',
  besser: '#538135',
  schlechter: '#C45911',
};

/** HTML für die Anzeige einer LaTeX-Zelle. */
export function texPreview(source: string): string {
  let s = escapeHtml(source);
  for (let i = 0; i < 3; i++) {
    s = s
      .replace(/\\(?:tief|textsubscript)\{([^{}]*)\}/g, '<sub>$1</sub>')
      .replace(/\\(?:hoch|textsuperscript)\{([^{}]*)\}/g, '<sup>$1</sup>')
      .replace(/_\{([^{}]*)\}/g, '<sub>$1</sub>')
      .replace(/\^\{([^{}]*)\}/g, '<sup>$1</sup>')
      .replace(/_([A-Za-z0-9])/g, '<sub>$1</sub>')
      .replace(/\^([A-Za-z0-9])/g, '<sup>$1</sup>')
      .replace(/\\(?:textbf|mathbf)\{([^{}]*)\}/g, '<b>$1</b>')
      .replace(/\\(?:textit|emph|mathit)\{([^{}]*)\}/g, '<i>$1</i>')
      .replace(/\\(?:mathrm|text|textrm|mbox|textsf|texttt|underline)\{([^{}]*)\}/g, '$1')
      .replace(/\\textcolor\{([^{}]*)\}\{([^{}]*)\}/g, (_, c: string, t: string) => {
        const color = colors[c] ?? (/^#?[0-9a-fA-F]{6}$/.test(c) ? `#${c.replace('#', '')}` : null);
        return color ? `<span style="color:${color}">${t}</span>` : t;
      });
  }
  return s
    .replace(/\\([A-Za-z]+|[%&_#$])/g, (all, name: string) => symbols[name] ?? all)
    .replace(/\\[,;:! ]/g, ' ')
    .replace(/\\\\/g, '<br>')
    .replace(/~/g, '&nbsp;')
    .replace(/\$/g, '')
    .replace(/[{}]/g, '');
}
