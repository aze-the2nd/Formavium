//! Abbildung von Änderungen im reinen LaTeX-Quelltext auf das Dokument.
//!
//! Eine Änderung innerhalb eines eindeutig zuordenbaren Bereichs (freier
//! LaTeX-Teil, Präambel, Text- oder Zahlenfeld, Tabellenzelle) aktualisiert
//! genau diesen Bereich. Berührt sie die vom Modul erzeugte Struktur, wird
//! der betroffene Abschnitt nur nach Bestätigung zu freiem LaTeX. Vom Kern
//! erzeugte Rahmenzeilen (`\begin{document}` usw.) sind nicht bearbeitbar.

use crate::edit::Command;
use crate::latex::{unescape_text, Generated, Origin, Span};
use crate::model::{FieldKind, Id, ModuleInstance, PartContent, Project, Value};

/// Eine Änderung im Quelltext, in Bytepositionen des erzeugten Texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceChange {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourcePlan {
    /// Eindeutig zuordenbar: Befehl direkt anwenden.
    Apply(Command),
    /// Module würden ihre visuelle Zuordnung verlieren. Erst nach
    /// Bestätigung anwenden; Rückgängig bleibt möglich.
    Convert { modules: Vec<Id>, command: Command },
    /// Bereich wird von Formavium erzeugt.
    Reject(String),
}

pub fn plan_source_change(
    project: &Project,
    generated: &Generated,
    change: &SourceChange,
) -> SourcePlan {
    let text = &generated.text;
    let SourceChange { from, to, insert } = change;
    let (from, to) = (*from, *to);
    if from > to || to > text.len() || !text.is_char_boundary(from) || !text.is_char_boundary(to) {
        return SourcePlan::Reject("Ungültiger Änderungsbereich".into());
    }

    if let Some(cmd) = editable_span(project, generated, from, to)
        .and_then(|s| map_span(project, generated, s, from, to, insert))
    {
        return SourcePlan::Apply(cmd);
    }

    // Auf Teilebene: zusammenhängende Teile in freies LaTeX umwandeln.
    let ranges = &generated.part_ranges;
    let affected: Vec<usize> = ranges
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            if from == to {
                r.start <= from && from < r.end
            } else {
                r.start < to && from < r.end
            }
        })
        .map(|(i, _)| i)
        .collect();
    let (Some(&first), Some(&last)) = (affected.first(), affected.last()) else {
        return SourcePlan::Reject(
            "Dieser Bereich wird von Formavium erzeugt (Pakete, \\begin{document}, \\end{document})".into(),
        );
    };
    let (start, end) = (ranges[first].start, ranges[last].end);
    if from < start || to > end {
        return SourcePlan::Reject(
            "Die Änderung reicht in einen von Formavium erzeugten Bereich".into(),
        );
    }
    let parts: Vec<Id> = ranges[first..=last]
        .iter()
        .map(|r| r.part.clone())
        .collect();

    // Einfügen in der Leerzeile hinter einem Modul: neuen freien Teil anlegen.
    if from == to && affected.len() == 1 {
        let part = &parts[0];
        let module_end = generated
            .spans
            .iter()
            .find(|s| matches!(&s.origin, Origin::Module { part: p, .. } if p == part))
            .map(|s| s.end);
        if module_end.is_some_and(|e| from >= e) {
            let index = project.part_index(part).map_or(0, |i| i + 1);
            return SourcePlan::Apply(Command::InsertPart {
                index,
                content: PartContent::Latex {
                    source: insert.clone(),
                },
            });
        }
    }

    let source = format!("{}{}{}", &text[start..from], insert, &text[to..end]);
    // Unverändert übernehmen: erzeugt wieder exakt denselben Text.
    let modules: Vec<Id> = parts
        .iter()
        .filter(|id| {
            matches!(
                project.part(id).map(|p| &p.content),
                Some(PartContent::Module(_))
            )
        })
        .cloned()
        .collect();
    let command = Command::ReplacePartsWithLatex { parts, source };
    if modules.is_empty() {
        SourcePlan::Apply(command)
    } else {
        SourcePlan::Convert { modules, command }
    }
}

fn is_editable(project: &Project, origin: &Origin) -> bool {
    match origin {
        Origin::Preamble | Origin::Latex { .. } | Origin::Cell { .. } => true,
        Origin::Field { part, path } => field_kind(project, part, path)
            .is_some_and(|k| matches!(k, FieldKind::Text | FieldKind::Number)),
        Origin::Module { .. } => false,
    }
}

fn field_kind(project: &Project, part: &Id, path: &str) -> Option<FieldKind> {
    let inst = project.instance(part)?;
    let (def, field) = project.scope_module(&inst.module, path)?;
    Some(def.field(field)?.kind)
}

/// Kleinster bearbeitbarer Bereich, der die Änderung vollständig enthält.
fn editable_span<'g>(
    project: &Project,
    g: &'g Generated,
    from: usize,
    to: usize,
) -> Option<&'g Span> {
    let innermost = g
        .spans
        .iter()
        .filter(|s| s.start <= from && to <= s.end)
        .min_by_key(|s| s.end - s.start)?;
    if is_editable(project, &innermost.origin) {
        return Some(innermost);
    }
    // Einfügen an einer Grenze: einen angrenzenden bearbeitbaren Bereich wählen.
    if from == to {
        return g
            .spans
            .iter()
            .filter(|s| s.start <= from && from <= s.end && is_editable(project, &s.origin))
            .min_by_key(|s| s.end - s.start);
    }
    None
}

fn map_span(
    project: &Project,
    g: &Generated,
    span: &Span,
    from: usize,
    to: usize,
    insert: &str,
) -> Option<Command> {
    let new_text = format!(
        "{}{}{}",
        &g.text[span.start..from],
        insert,
        &g.text[to..span.end]
    );
    let strip_nl = |s: String| s.strip_suffix('\n').map(str::to_owned).unwrap_or(s);
    Some(match &span.origin {
        Origin::Preamble => Command::SetPreamble {
            source: strip_nl(new_text),
        },
        Origin::Latex { part } => Command::SetLatex {
            part: part.clone(),
            source: new_text,
        },
        Origin::Field { part, path } => {
            // Ein Zeilenumbruch gehört nicht in ein einzeiliges Feld.
            if new_text.contains('\n') {
                return None;
            }
            let raw = unescape_text(&new_text);
            let value = match field_kind(project, part, path)? {
                FieldKind::Number => Value::Number(raw),
                _ => Value::Text(raw),
            };
            Command::SetValue {
                part: part.clone(),
                path: path.clone(),
                value,
            }
        }
        Origin::Cell {
            part,
            path,
            row,
            column,
            ..
        } => {
            // Zeilenumbrüche gehören zur Tabellenstruktur. Ein getipptes `&`
            // ist wie in jedem Feld Inhalt und wird beim Erzeugen maskiert.
            if new_text.contains('\n') {
                return None;
            }
            let inst: &ModuleInstance = project.instance(part)?;
            let mut rows = match project.effective_value(inst, path)? {
                Value::Table(r) => r.clone(),
                _ => return None,
            };
            let r = rows.iter_mut().find(|r| &r.id == row)?;
            // LaTeX-Spalten nehmen den Text, wie er steht.
            let latex =
                crate::latex::table_column(project, part, path, column).is_some_and(|c| c.latex);
            let value = if latex {
                new_text
            } else {
                unescape_text(&new_text)
            };
            r.cells.insert(column.clone(), value);
            Command::SetValue {
                part: part.clone(),
                path: path.clone(),
                value: Value::Table(rows),
            }
        }
        Origin::Module { .. } => return None,
    })
}
