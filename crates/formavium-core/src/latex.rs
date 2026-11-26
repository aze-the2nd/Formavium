//! Erzeugung des reinen LaTeX-Quelltexts mit Positionszuordnung.
//!
//! Der erzeugte Text enthält keine Formavium-Metadaten. Die Zuordnung von
//! Bytebereichen zu Dokumentteilen und Feldern wird getrennt geliefert.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::model::{
    Align, Column, Element, ElementKind, Id, ModuleInstance, ModuleRef, PartContent, Project, Row,
    TableLatex, Text, UnitPlacement, Value, LIST_COLUMN,
};

/// Herkunft eines Bereichs im erzeugten Quelltext.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    Preamble,
    /// Freier LaTeX-Teil des Dokuments.
    Latex {
        part: Id,
    },
    /// Struktur einer Modulinstanz (vom Modul erzeugt).
    Module {
        part: Id,
        module: ModuleRef,
    },
    /// Wert eines Eingabefeldes einer Instanz.
    Field {
        part: Id,
        path: String,
    },
    /// Zelle eines Tabellenfeldes.
    Cell {
        part: Id,
        path: String,
        row: Id,
        column: Id,
        /// Stelle der Zelle in ihrer gesetzten Zeile (0 = erste Spalte).
        index: usize,
    },
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Origin::Preamble => write!(f, "Präambel"),
            Origin::Latex { part } => write!(f, "freier LaTeX-Teil {part}"),
            Origin::Module { part, module } => write!(f, "Modul {module} in Teil {part}"),
            Origin::Field { part, path } => write!(f, "Feld „{path}“ in Teil {part}"),
            Origin::Cell {
                part,
                path,
                row,
                column,
                ..
            } => {
                write!(
                    f,
                    "Tabelle „{path}“, Zeile {row}, Spalte {column} in Teil {part}"
                )
            }
        }
    }
}

impl Origin {
    /// Dokumentteil, zu dem der Bereich gehört.
    pub fn part(&self) -> Option<&Id> {
        match self {
            Origin::Preamble => None,
            Origin::Latex { part }
            | Origin::Module { part, .. }
            | Origin::Field { part, .. }
            | Origin::Cell { part, .. } => Some(part),
        }
    }
}

/// Gesamter Textbereich eines Dokumentteils einschließlich Zeilenumbrüchen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartRange {
    pub part: Id,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// Bytebereich im UTF-8-Text.
    pub start: usize,
    pub end: usize,
    pub origin: Origin,
}

/// Hinweise, die beim Erzeugen auffallen (z. B. fehlende Pflichtangaben).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub origin: Origin,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct Generated {
    pub text: String,
    pub spans: Vec<Span>,
    pub notices: Vec<Notice>,
    /// Relative Pfade aller referenzierten Bilddateien.
    pub resources: BTreeSet<String>,
    /// Bereiche der Dokumentteile in Dokumentreihenfolge.
    pub part_ranges: Vec<PartRange>,
}

impl Generated {
    /// Innerste Herkunft an einer Byteposition.
    pub fn origin_at(&self, offset: usize) -> Option<&Origin> {
        self.spans
            .iter()
            .filter(|s| s.start <= offset && offset < s.end)
            .min_by_key(|s| s.end - s.start)
            .map(|s| &s.origin)
    }

    /// Herkunft einer (1-basierten) Zeile, wie sie TeX-Diagnosen melden.
    pub fn origin_at_line(&self, line: u32) -> Option<&Origin> {
        let start = line_start(&self.text, line)?;
        let end = self.text[start..]
            .find('\n')
            .map_or(self.text.len(), |i| start + i);
        // Erste nicht leere Stelle der Zeile bevorzugen.
        let pos = self.text[start..end]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map_or(start, |(i, _)| start + i);
        self.origin_at(pos)
    }

    /// 1-basierte Zeile einer Byteposition.
    pub fn line_of(&self, offset: usize) -> u32 {
        let offset = offset.min(self.text.len());
        self.text.as_bytes()[..offset]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u32
            + 1
    }

    /// Bereich eines Dokumentteils.
    pub fn part_range(&self, part: &Id) -> Option<&PartRange> {
        self.part_ranges.iter().find(|r| &r.part == part)
    }
}

/// Byteposition des Zeilenanfangs (Zeilen ab 1).
pub fn line_start(text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return None;
    }
    if line == 1 {
        return Some(0);
    }
    text.match_indices('\n')
        .nth(line as usize - 2)
        .map(|(i, _)| i + 1)
}

/// Maskiert Inhalte für den Textkontext von LaTeX.
///
/// Eingegebene Werte sind Inhalt und nie Quelltext (Sicherheitsanforderung
/// „Datenfelder und freies LaTeX“).
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '$' => out.push_str("\\$"),
            '&' => out.push_str("\\&"),
            '#' => out.push_str("\\#"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            '^' => out.push_str("\\textasciicircum{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '\r' => {}
            '\n' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Umkehrung von [`escape_text`] für im Quelltext bearbeitete Feldwerte.
///
/// Bekannte Maskierungen werden zurückgeführt. Alles andere, auch ein
/// unmaskiertes Sonderzeichen, gilt als eingegebener Inhalt.
pub fn unescape_text(s: &str) -> String {
    const SEQ: [(&str, char); 10] = [
        ("\\textbackslash{}", '\\'),
        ("\\textasciicircum{}", '^'),
        ("\\textasciitilde{}", '~'),
        ("\\{", '{'),
        ("\\}", '}'),
        ("\\$", '$'),
        ("\\&", '&'),
        ("\\#", '#'),
        ("\\%", '%'),
        ("\\_", '_'),
    ];
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while let Some(c) = rest.chars().next() {
        if c == '\\' {
            for (seq, ch) in SEQ {
                if let Some(r) = rest.strip_prefix(seq) {
                    out.push(ch);
                    rest = r;
                    continue 'outer;
                }
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Wandelt eine UTF-16-Position (Editor) in eine UTF-8-Byteposition (Kern).
/// `None`, wenn die Position außerhalb liegt oder ein Ersatzpaar teilt.
pub fn utf16_to_byte(s: &str, utf16: usize) -> Option<usize> {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units == utf16 {
            return Some(i);
        }
        units += c.len_utf16();
        if units > utf16 {
            return None;
        }
    }
    (units == utf16).then_some(s.len())
}

/// Wandelt eine UTF-8-Byteposition in eine UTF-16-Position.
pub fn byte_to_utf16(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().map(char::len_utf16).sum()
}

/// Maskiert einen Absatztext; Leerzeilen bleiben Absatzwechsel.
fn escape_paragraphs(s: &str) -> String {
    let normalized = s.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .map(|p| escape_text(p.trim()))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Ein Bildpfad ist zulässig, wenn er relativ ist, im Projekt bleibt und nur
/// Zeichen enthält, die in `\includegraphics` keine Maskierung brauchen.
pub fn is_safe_resource_path(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains(':')
        && p.split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
}

struct Writer<'p> {
    project: &'p Project,
    out: Generated,
}

impl<'p> Writer<'p> {
    fn push(&mut self, s: &str) {
        self.out.text.push_str(s);
    }

    fn spanned(&mut self, origin: Origin, f: impl FnOnce(&mut Self)) {
        let start = self.out.text.len();
        f(self);
        let end = self.out.text.len();
        // Leere Bereiche bleiben erhalten: In ein leeres Feld kann man tippen.
        self.out.spans.push(Span { start, end, origin });
    }

    fn notice(&mut self, origin: Origin, message: impl Into<String>) {
        self.out.notices.push(Notice {
            origin,
            message: message.into(),
        });
    }
}

/// Erzeugt das vollständige `main.tex` eines Projekts.
pub fn generate(project: &Project) -> Generated {
    let mut w = Writer {
        project,
        out: Generated::default(),
    };

    // Freies Dokument: Die Datei ist genau der Text des Anwenders.
    if project.free {
        w.spanned(Origin::Preamble, |w| {
            w.push(&project.preamble);
            if !project.preamble.ends_with('\n') {
                w.push("\n");
            }
        });
        scan_graphics(&project.preamble, &mut w.out.resources);
        return w.out;
    }

    w.spanned(Origin::Preamble, |w| {
        w.push(project.preamble.trim_end());
        w.push("\n");
    });
    // Bilder aus freiem LaTeX (etwa ein Logo in der Kopfzeile).
    scan_graphics(&project.preamble, &mut w.out.resources);
    let packages = required_packages(project);
    for pkg in packages {
        if !project.preamble.contains(pkg) {
            let _ = writeln!(w.out.text, "\\usepackage{{{pkg}}}");
        }
    }
    // Vorspann der Module, jeder Text nur einmal.
    let mut seen = BTreeSet::new();
    for def in &project.modules {
        let pre = def.preamble.trim();
        if !pre.is_empty() && seen.insert(pre) {
            w.push(pre);
            w.push("\n");
            scan_graphics(pre, &mut w.out.resources);
        }
    }
    w.push("\\begin{document}\n");

    for part in &project.parts {
        let part_start = w.out.text.len();
        match &part.content {
            PartContent::Latex { source } => {
                w.spanned(
                    Origin::Latex {
                        part: part.id.clone(),
                    },
                    |w| {
                        // Freier Quelltext bleibt exakt erhalten, nur ein
                        // abschließender Zeilenumbruch wird bei Bedarf ergänzt.
                        w.push(source);
                        if !source.ends_with('\n') {
                            w.push("\n");
                        }
                        scan_graphics(source, &mut w.out.resources);
                    },
                );
            }
            PartContent::Module(inst) => {
                let origin = Origin::Module {
                    part: part.id.clone(),
                    module: inst.module.clone(),
                };
                w.spanned(origin.clone(), |w| match project.module(&inst.module) {
                    Some(def) => {
                        let ctx = Ctx {
                            part: &part.id,
                            inst,
                            scope: String::new(),
                        };
                        emit_body(w, &ctx, &def.body);
                    }
                    None => {
                        w.notice(
                            origin.clone(),
                            format!("Moduldefinition {} fehlt im Dokument", inst.module),
                        );
                        w.push("% Modul nicht verfügbar\n");
                    }
                });
                w.push("\n");
            }
        }
        w.out.part_ranges.push(PartRange {
            part: part.id.clone(),
            start: part_start,
            end: w.out.text.len(),
        });
    }
    w.push("\\end{document}\n");
    w.out
}

fn required_packages(project: &Project) -> Vec<&'static str> {
    let mut graphicx = false;
    // Spalten fester Breite brauchen `array` für die Ausrichtung.
    let mut array = false;
    for def in &project.modules {
        visit(&def.body, &mut |k| match k {
            ElementKind::Image { .. } => graphicx = true,
            ElementKind::Latex { source } => {
                graphicx |= segments(source)
                    .iter()
                    .any(|s| matches!(s, Segment::Image { .. }));
            }
            ElementKind::Table { columns, .. } => {
                array |= columns.iter().any(|c| c.width.is_some());
            }
            _ => {}
        });
    }
    for part in &project.parts {
        if let PartContent::Module(inst) = &part.content {
            array |= inst.columns.values().flatten().any(|c| c.width.is_some());
        }
    }
    // Verbundene Zellen in Werten der Dokumente oder Vorgaben der Module.
    let merged =
        |v: &Value| matches!(v, Value::Table(rows) if rows.iter().any(|r| !r.merged.is_empty()));
    let multirow = project.parts.iter().any(|part| {
        matches!(&part.content, PartContent::Module(inst) if inst.values.values().any(merged))
    }) || project
        .modules
        .iter()
        .flat_map(|d| &d.fields)
        .any(|f| f.default.as_ref().is_some_and(merged));
    let mut v = Vec::new();
    if graphicx {
        v.push("graphicx");
    }
    if array {
        v.push("array");
    }
    if multirow {
        v.push("multirow");
    }
    v
}

fn visit(body: &[Element], f: &mut impl FnMut(&ElementKind)) {
    for el in body {
        f(&el.kind);
        if let ElementKind::Row { children } = &el.kind {
            visit(children, f);
        }
    }
}

struct Ctx<'a> {
    part: &'a Id,
    inst: &'a ModuleInstance,
    /// Pfadpräfix für verschachtelte Module, leer oder mit `/` endend.
    scope: String,
}

impl Ctx<'_> {
    fn path(&self, id: &Id) -> String {
        format!("{}{}", self.scope, id)
    }
    fn field_origin(&self, id: &Id) -> Origin {
        Origin::Field {
            part: self.part.clone(),
            path: self.path(id),
        }
    }
}

const MAX_DEPTH: usize = 16;

fn emit_body(w: &mut Writer, ctx: &Ctx, body: &[Element]) {
    for el in body {
        emit_element(w, ctx, el);
    }
}

fn emit_text(w: &mut Writer, ctx: &Ctx, text: &Text, paragraphs: bool) {
    match text {
        Text::Literal(s) => {
            let escaped = if paragraphs {
                escape_paragraphs(s)
            } else {
                escape_text(s)
            };
            w.push(&escaped);
        }
        Text::Field(id) => {
            let path = ctx.path(id);
            let value = match w.project.effective_value(ctx.inst, &path) {
                Some(Value::Text(s)) | Some(Value::Number(s)) => s.clone(),
                _ => String::new(),
            };
            if value.trim().is_empty() && ctx.inst.required {
                w.notice(
                    ctx.field_origin(id),
                    format!("Pflichtangabe „{path}“ ist leer"),
                );
            }
            let escaped = if paragraphs {
                escape_paragraphs(&value)
            } else {
                escape_text(&value)
            };
            w.spanned(ctx.field_origin(id), |w| w.push(&escaped));
        }
    }
}

fn emit_element(w: &mut Writer, ctx: &Ctx, el: &Element) {
    match &el.kind {
        ElementKind::Heading { level, text } => {
            let cmd = match level {
                0 | 1 => "section",
                2 => "subsection",
                _ => "subsubsection",
            };
            let _ = write!(w.out.text, "\\{cmd}{{");
            emit_text(w, ctx, text, false);
            w.push("}\n");
        }
        ElementKind::Paragraph { text } => {
            emit_text(w, ctx, text, true);
            w.push("\n\n");
        }
        ElementKind::Table {
            columns,
            rows,
            caption,
            latex,
        } => match latex {
            Some(template) => emit_table_template(w, ctx, el, columns, rows, template),
            None => emit_table(w, ctx, el, columns, rows, caption.as_ref()),
        },
        ElementKind::Image { path, width } => {
            let field_path = ctx.path(path);
            let value = match w.project.effective_value(ctx.inst, &field_path) {
                Some(Value::Image(p)) => Some(p.clone()),
                _ => None,
            };
            let width = width.clamp(0.05, 1.0);
            w.spanned(ctx.field_origin(path), |w| match value {
                Some(p) if is_safe_resource_path(&p) => {
                    let _ = write!(
                        w.out.text,
                        "\\includegraphics[width={width:.3}\\linewidth]{{{p}}}"
                    );
                    w.out.resources.insert(p);
                }
                Some(p) => {
                    w.notice(
                        ctx.field_origin(path),
                        format!("Bildpfad „{p}“ ist nicht zulässig"),
                    );
                    placeholder(w, width, "Bildpfad unzulässig");
                }
                None => {
                    if ctx.inst.required {
                        w.notice(
                            ctx.field_origin(path),
                            format!("Pflichtbild „{field_path}“ fehlt"),
                        );
                    }
                    placeholder(w, width, "Bild");
                }
            });
            w.push("\n");
        }
        ElementKind::Row { children } => {
            let n = children.len().max(1) as f32;
            let width = (0.98 / n) - 0.02 * (n - 1.0) / n;
            w.push("\\par\\noindent\n");
            for (i, child) in children.iter().enumerate() {
                if i > 0 {
                    w.push("\\hfill\n");
                }
                let _ = writeln!(
                    w.out.text,
                    "\\begin{{minipage}}[t]{{{width:.3}\\linewidth}}\\vspace{{0pt}}"
                );
                emit_element(w, ctx, child);
                w.push("\\end{minipage}\n");
            }
            w.push("\\par\n");
        }
        ElementKind::Module { module } => {
            let depth = ctx.scope.matches('/').count();
            match w.project.module(module) {
                Some(def) if depth < MAX_DEPTH => {
                    let sub = Ctx {
                        part: ctx.part,
                        inst: ctx.inst,
                        scope: format!("{}{}/", ctx.scope, el.id),
                    };
                    emit_body(w, &sub, &def.body);
                }
                Some(_) => {
                    let o = Origin::Module {
                        part: ctx.part.clone(),
                        module: module.clone(),
                    };
                    w.notice(o, "Modulverschachtelung zu tief oder zyklisch");
                }
                None => {
                    let o = Origin::Module {
                        part: ctx.part.clone(),
                        module: module.clone(),
                    };
                    w.notice(o, format!("Untermodul {module} fehlt im Dokument"));
                }
            }
        }
        ElementKind::List { items, ordered } => {
            let path = ctx.path(items);
            let data: Vec<Row> = match w.project.effective_value(ctx.inst, &path) {
                Some(Value::Table(r)) => r.clone(),
                _ => Vec::new(),
            };
            let env = if *ordered { "enumerate" } else { "itemize" };
            let column = Id::from(LIST_COLUMN);
            w.spanned(ctx.field_origin(items), |w| {
                // Eine Liste ohne Punkte wäre in LaTeX ein Fehler.
                if data.is_empty() {
                    return;
                }
                let _ = writeln!(w.out.text, "\\begin{{{env}}}");
                for row in &data {
                    w.push("\\item ");
                    let cell = row.cells.get(&column).map(String::as_str).unwrap_or("");
                    let origin = Origin::Cell {
                        part: ctx.part.clone(),
                        path: path.clone(),
                        row: row.id.clone(),
                        column: column.clone(),
                        index: 0,
                    };
                    w.spanned(origin, |w| w.push(&escape_text(cell)));
                    w.push("\n");
                }
                let _ = writeln!(w.out.text, "\\end{{{env}}}");
            });
            if data.is_empty() && ctx.inst.required {
                w.notice(ctx.field_origin(items), format!("Liste „{path}“ ist leer"));
            }
        }
        ElementKind::Latex { source } => {
            for segment in segments(source.trim_end_matches(['\n', '\r'])) {
                match segment {
                    Segment::Text(text) => {
                        scan_graphics(text, &mut w.out.resources);
                        w.push(text);
                    }
                    Segment::Field(id) => {
                        let id = Id::from(id);
                        let value = field_text(w, ctx, &id);
                        if value.trim().is_empty() && ctx.inst.required {
                            let path = ctx.path(&id);
                            w.notice(
                                ctx.field_origin(&id),
                                format!("Pflichtangabe „{path}“ ist leer"),
                            );
                        }
                        w.spanned(ctx.field_origin(&id), |w| w.push(&escape_text(&value)));
                    }
                    // LaTeX des Anwenders unverändert. Ohne Feldbereich:
                    // Im Quelltext lässt es sich nicht eindeutig zurückführen.
                    Segment::Raw(id) => {
                        let value = field_text(w, ctx, &Id::from(id));
                        w.push(value.trim());
                    }
                    Segment::Image { options, field } => {
                        let id = Id::from(field);
                        let value = match w.project.effective_value(ctx.inst, &ctx.path(&id)) {
                            Some(Value::Image(p)) => Some(p.clone()),
                            _ => None,
                        };
                        w.spanned(ctx.field_origin(&id), |w| match value {
                            Some(p) if is_safe_resource_path(&p) => {
                                let _ = write!(w.out.text, "\\includegraphics{options}{{{p}}}");
                                w.out.resources.insert(p);
                            }
                            _ => placeholder(w, 0.6, "Bild"),
                        });
                    }
                }
            }
            w.push("\n");
        }
    }
}

/// Text- oder Zahlwert eines Feldes, sonst leer.
fn field_text(w: &Writer, ctx: &Ctx, id: &Id) -> String {
    match w.project.effective_value(ctx.inst, &ctx.path(id)) {
        Some(Value::Text(s)) | Some(Value::Number(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Bestandteil eines LaTeX-Elements: fester Quelltext oder ein Eingabefeld.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    Text(&'a str),
    /// `\feld{kennung}`: Text des Anwenders, maskiert.
    Field(&'a str),
    /// `\rohfeld{kennung}`: LaTeX des Anwenders, unverändert.
    Raw(&'a str),
    /// `\bildfeld[optionen]{kennung}`: Bild, `options` mit eckigen Klammern.
    Image {
        options: &'a str,
        field: &'a str,
    },
}

/// Zerlegt den Quelltext eines LaTeX-Elements an seinen Feldmarken.
pub fn segments(source: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while let Some(found) = source[i..].find('\\') {
        let at = i + found;
        let rest = &source[at..];
        let parsed = ["\\feld", "\\rohfeld", "\\bildfeld"]
            .iter()
            .find_map(|name| {
                let after = rest.strip_prefix(name)?;
                let (options, after) = match (*name == "\\bildfeld", after.strip_prefix('[')) {
                    (true, Some(inner)) => {
                        let close = inner.find(']')?;
                        (&after[..close + 2], &inner[close + 1..])
                    }
                    _ => ("", after),
                };
                let inner = after.strip_prefix('{')?;
                let close = inner.find('}')?;
                let id = &inner[..close];
                if id.is_empty() || id.contains(['{', '\\', ' ', '\n']) {
                    return None;
                }
                let len = rest.len() - inner.len() + close + 1;
                let segment = match *name {
                    "\\feld" => Segment::Field(id),
                    "\\rohfeld" => Segment::Raw(id),
                    _ => Segment::Image { options, field: id },
                };
                Some((segment, len))
            });
        match parsed {
            Some((segment, len)) => {
                if at > text_start {
                    out.push(Segment::Text(&source[text_start..at]));
                }
                out.push(segment);
                i = at + len;
                text_start = i;
            }
            None => i = at + 1,
        }
    }
    if text_start < source.len() {
        out.push(Segment::Text(&source[text_start..]));
    }
    out
}

/// Kennungen aller Felder, die ein LaTeX-Element verwendet.
pub fn field_refs(source: &str) -> Vec<&str> {
    segments(source)
        .into_iter()
        .filter_map(|s| match s {
            Segment::Field(id) | Segment::Raw(id) | Segment::Image { field: id, .. } => Some(id),
            Segment::Text(_) => None,
        })
        .collect()
}

/// Merkt Bilder vor, die freies LaTeX mit `\includegraphics` einbindet,
/// damit sie beim Rendern und im Export bereitstehen.
fn scan_graphics(text: &str, resources: &mut BTreeSet<String>) {
    let mut rest = text;
    while let Some(at) = rest.find("\\includegraphics") {
        rest = &rest[at + "\\includegraphics".len()..];
        let mut after = rest.trim_start();
        if let Some(inner) = after.strip_prefix('[') {
            match inner.find(']') {
                Some(close) => after = inner[close + 1..].trim_start(),
                None => continue,
            }
        }
        let Some(inner) = after.strip_prefix('{') else {
            continue;
        };
        let Some(close) = inner.find('}') else {
            continue;
        };
        let path = inner[..close].trim();
        let known = [".png", ".jpg", ".jpeg", ".pdf"]
            .iter()
            .any(|ext| path.to_ascii_lowercase().ends_with(ext));
        if known && is_safe_resource_path(path) {
            resources.insert(path.to_owned());
        }
    }
}

fn placeholder(w: &mut Writer, width: f32, label: &str) {
    let _ = write!(
        w.out.text,
        "\\fbox{{\\parbox[c][0.4\\linewidth][c]{{\\dimexpr{width:.3}\\linewidth-2\\fboxsep-2\\fboxrule\\relax}}{{\\centering {}}}}}",
        escape_text(label)
    );
}

/// Spalte der Tabelle, deren Zeilen aus dem Tabellenfeld `rows_path`
/// einer Verwendung stammen. Listen haben keine Spalten.
pub fn table_column<'p>(
    project: &'p Project,
    part: &Id,
    rows_path: &str,
    column: &Id,
) -> Option<&'p Column> {
    let inst = project.instance(part)?;
    let (def, field) = project.scope_module(&inst.module, rows_path)?;
    fn find<'a>(body: &'a [Element], field: &str) -> Option<&'a Element> {
        body.iter().find_map(|el| match &el.kind {
            ElementKind::Table { rows, .. } if rows.as_str() == field => Some(el),
            ElementKind::Row { children } => find(children, field),
            _ => None,
        })
    }
    let el = find(&def.body, field)?;
    let ElementKind::Table { columns, .. } = &el.kind else {
        return None;
    };
    let scope = &rows_path[..rows_path.len() - field.len()];
    let table_path = format!("{scope}{}", el.id);
    ordered_columns(inst, &table_path, columns)
        .into_iter()
        .find(|c| &c.id == column)
}

/// Effektive Spalten einer Tabelle in einer Instanz: lokal angepasste
/// Spalten ersetzen die der Definition, darauf wirkt die lokale Reihenfolge.
pub fn ordered_columns<'c>(
    inst: &'c ModuleInstance,
    table_path: &str,
    columns: &'c [Column],
) -> Vec<&'c Column> {
    let columns = inst.columns.get(table_path).map_or(columns, Vec::as_slice);
    match inst.column_order.get(table_path) {
        Some(order) => {
            let mut v: Vec<&Column> = order
                .iter()
                .filter_map(|id| columns.iter().find(|c| &c.id == id))
                .collect();
            // Später in der Definition ergänzte Spalten erscheinen am Ende.
            for c in columns {
                if !order.contains(&c.id) {
                    v.push(c);
                }
            }
            v
        }
        None => columns.iter().collect(),
    }
}

fn emit_table(
    w: &mut Writer,
    ctx: &Ctx,
    el: &Element,
    columns: &[Column],
    rows: &Id,
    caption: Option<&Text>,
) {
    let table_path = ctx.path(&el.id);
    let cols = ordered_columns(ctx.inst, &table_path, columns);
    let rows_path = ctx.path(rows);
    let data: Vec<Row> = match w.project.effective_value(ctx.inst, &rows_path) {
        Some(Value::Table(r)) => r.clone(),
        _ => Vec::new(),
    };

    w.push("\\begin{center}\n");
    if let Some(caption) = caption {
        w.push("\\textbf{");
        emit_text(w, ctx, caption, false);
        w.push("}\\par\\smallskip\n");
    }
    // Standard ist ein regelmäßiges Gitter: Linien um jede Zelle.
    let mut spec = String::from("|");
    for c in &cols {
        match c.width {
            // Feste Breite: Der Text bricht in der Zelle um.
            Some(width) => {
                let flush = match c.align {
                    Align::Left => "raggedright",
                    Align::Center => "centering",
                    Align::Right => "raggedleft",
                };
                let width = width.clamp(0.03, 1.0);
                let _ = write!(
                    spec,
                    ">{{\\{flush}\\arraybackslash}}p{{{width:.3}\\linewidth}}"
                );
            }
            None => spec.push(match c.align {
                Align::Left => 'l',
                Align::Center => 'c',
                Align::Right => 'r',
            }),
        }
        spec.push('|');
    }
    let _ = writeln!(w.out.text, "\\begin{{tabular}}{{{spec}}}\n\\hline");
    for (i, c) in cols.iter().enumerate() {
        if i > 0 {
            w.push(" & ");
        }
        emit_text(w, ctx, &c.header, false);
        if let (Some(unit), UnitPlacement::Header) = (&c.unit, c.unit_placement) {
            let _ = write!(w.out.text, " [{}]", escape_text(unit));
        }
    }
    w.push(" \\\\\n\\hline\n");
    w.spanned(ctx.field_origin(rows), |w| {
        for (ri, row) in data.iter().enumerate() {
            for (i, c) in cols.iter().enumerate() {
                if i > 0 {
                    w.push(" & ");
                }
                // Verbundene Zelle: Der Inhalt steht in der obersten Zelle.
                if ri > 0 && row.merged.contains(&c.id) {
                    continue;
                }
                let span = 1 + data[ri + 1..]
                    .iter()
                    .take_while(|r| r.merged.contains(&c.id))
                    .count();
                if span > 1 {
                    let width = if c.width.is_some() { "=" } else { "*" };
                    let _ = write!(w.out.text, "\\multirow{{{span}}}{{{width}}}{{");
                }
                let cell = row.cells.get(&c.id).map(String::as_str).unwrap_or("");
                let origin = Origin::Cell {
                    part: ctx.part.clone(),
                    path: rows_path.clone(),
                    row: row.id.clone(),
                    column: c.id.clone(),
                    index: i,
                };
                let content = if c.latex {
                    cell.to_owned()
                } else {
                    escape_text(cell)
                };
                w.spanned(origin, |w| w.push(&content));
                if let (Some(unit), UnitPlacement::Value) = (&c.unit, c.unit_placement) {
                    if !cell.trim().is_empty() {
                        let _ = write!(w.out.text, "~{}", escape_text(unit));
                    }
                }
                if span > 1 {
                    w.push("}");
                }
            }
            w.push(" \\\\\n");
            // Unter verbundenen Zellen keine Linie.
            let next = data.get(ri + 1);
            let open: Vec<usize> = (0..cols.len())
                .filter(|&i| next.is_none_or(|n| !n.merged.contains(&cols[i].id)))
                .collect();
            if open.len() == cols.len() {
                w.push("\\hline\n");
            } else {
                let mut i = 0;
                while i < open.len() {
                    let start = open[i];
                    while i + 1 < open.len() && open[i + 1] == open[i] + 1 {
                        i += 1;
                    }
                    let _ = writeln!(w.out.text, "\\cline{{{}-{}}}", start + 1, open[i] + 1);
                    i += 1;
                }
            }
        }
    });
    if data.is_empty() && ctx.inst.required {
        w.notice(
            ctx.field_origin(rows),
            format!("Tabelle „{rows_path}“ ist leer"),
        );
    }
    w.push("\\end{tabular}\n\\end{center}\n");
}

/// Tabelle im eigenen Satz des Modulautors: Anfang, je Zeile die Vorlage
/// mit `#1`…`#9` für die Zellen, Ende. Jede Zeile steht auf einer eigenen
/// Quelltextzeile, damit SyncTeX die Zellen zuordnen kann.
fn emit_table_template(
    w: &mut Writer,
    ctx: &Ctx,
    el: &Element,
    columns: &[Column],
    rows: &Id,
    template: &TableLatex,
) {
    let table_path = ctx.path(&el.id);
    let cols = ordered_columns(ctx.inst, &table_path, columns);
    let rows_path = ctx.path(rows);
    let data: Vec<Row> = match w.project.effective_value(ctx.inst, &rows_path) {
        Some(Value::Table(r)) => r.clone(),
        _ => Vec::new(),
    };
    let begin = template.begin.trim_end_matches(['\n', '\r']);
    if !begin.is_empty() {
        scan_graphics(begin, &mut w.out.resources);
        w.push(begin);
        w.push("\n");
    }
    let row_template = template.row.replace(['\n', '\r'], " ");
    w.spanned(ctx.field_origin(rows), |w| {
        for (ri, row) in data.iter().enumerate() {
            let mut rest = row_template.as_str();
            while let Some(at) = rest.find('#') {
                w.push(&rest[..at]);
                let after = &rest[at + 1..];
                match after.chars().next() {
                    Some('#') => {
                        w.push("#");
                        rest = &after[1..];
                    }
                    Some(d @ '1'..='9') => {
                        let i = d as usize - '1' as usize;
                        rest = &after[1..];
                        let Some(c) = cols.get(i) else { continue };
                        // Fortsetzung einer verbundenen Zelle: leer.
                        if ri > 0 && row.merged.contains(&c.id) {
                            continue;
                        }
                        let cell = row.cells.get(&c.id).map(String::as_str).unwrap_or("");
                        let origin = Origin::Cell {
                            part: ctx.part.clone(),
                            path: rows_path.clone(),
                            row: row.id.clone(),
                            column: c.id.clone(),
                            index: i,
                        };
                        let mut content = if c.latex {
                            cell.to_owned()
                        } else {
                            escape_text(cell)
                        };
                        if let (Some(unit), UnitPlacement::Value) = (&c.unit, c.unit_placement) {
                            if !cell.trim().is_empty() {
                                let _ = write!(content, "~{}", escape_text(unit));
                            }
                        }
                        w.spanned(origin, |w| w.push(&content));
                    }
                    _ => {
                        w.push("#");
                        rest = after;
                    }
                }
            }
            w.push(rest);
            w.push("\n");
        }
    });
    if data.is_empty() && ctx.inst.required {
        w.notice(
            ctx.field_origin(rows),
            format!("Tabelle „{rows_path}“ ist leer"),
        );
    }
    let end = template.end.trim_matches(['\n', '\r']);
    if !end.is_empty() {
        w.push(end);
        w.push("\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_characters() {
        assert_eq!(
            escape_text("A & B 50% $x_1#^~\\{}"),
            "A \\& B 50\\% \\$x\\_1\\#\\textasciicircum{}\\textasciitilde{}\\textbackslash{}\\{\\}"
        );
    }

    #[test]
    fn keeps_unicode() {
        assert_eq!(escape_text("Größe µm – Ø"), "Größe µm – Ø");
    }

    #[test]
    fn resource_paths() {
        assert!(is_safe_resource_path("bilder/foto-1.png"));
        assert!(!is_safe_resource_path("../geheim.png"));
        assert!(!is_safe_resource_path("/etc/passwd"));
        assert!(!is_safe_resource_path("C:/x.png"));
        assert!(!is_safe_resource_path("a b.png"));
        assert!(!is_safe_resource_path("x}\\input{y.png"));
    }

    #[test]
    fn unescape_roundtrip() {
        for s in ["A & B 50% $x_1#^~\\{}", "Größe µm", "a\\b", ""] {
            assert_eq!(unescape_text(&escape_text(s)), s);
        }
        assert_eq!(unescape_text("roh & offen"), "roh & offen");
        assert_eq!(unescape_text("\\emph{x}"), "\\emph{x}");
    }

    #[test]
    fn utf16_conversion() {
        let s = "aä😀b";
        assert_eq!(utf16_to_byte(s, 0), Some(0));
        assert_eq!(utf16_to_byte(s, 1), Some(1));
        assert_eq!(utf16_to_byte(s, 2), Some(3));
        assert_eq!(utf16_to_byte(s, 3), None);
        assert_eq!(utf16_to_byte(s, 4), Some(7));
        assert_eq!(utf16_to_byte(s, 5), Some(8));
        assert_eq!(utf16_to_byte(s, 6), None);
        for b in [0, 1, 3, 7, 8] {
            assert_eq!(utf16_to_byte(s, byte_to_utf16(s, b)), Some(b));
        }
    }

    #[test]
    fn line_lookup() {
        assert_eq!(line_start("a\nb\nc", 1), Some(0));
        assert_eq!(line_start("a\nb\nc", 3), Some(4));
        assert_eq!(line_start("a\nb\nc", 4), None);
    }
}
