//! Serialisierbare Sichten für die Oberfläche.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use formavium_core::catalog::{content_kinds, ContentKind};
use formavium_core::latex::{byte_to_utf16, ordered_columns, segments, Segment};
use formavium_core::{
    Align, Element, ElementKind, Generated, ModuleDef, ModuleInstance, PartContent, Project,
    Revision, Text, UnitPlacement, Value, Workspace, LIST_COLUMN,
};
use formavium_render::{Preview, PreviewStatus, Severity};
use serde::Serialize;

#[derive(Serialize)]
pub struct DocView {
    pub path: String,
    pub title: String,
    pub revision: Revision,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Reiner LaTeX-Quelltext dieser Revision.
    pub source: String,
    pub parts: Vec<PartView>,
    pub notices: Vec<NoticeView>,
    pub modules: Vec<ModuleSummary>,
}

#[derive(Serialize)]
pub struct NoticeView {
    pub message: String,
    pub place: String,
    pub part: Option<String>,
}

#[derive(Serialize)]
pub struct ModuleSummary {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub kinds: Vec<ContentKind>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartView {
    Latex {
        id: String,
        /// Position im Quelltext (UTF-16) für Sprünge.
        offset: usize,
        line: u32,
        source: String,
    },
    Module {
        id: String,
        offset: usize,
        line: u32,
        name: String,
        module: String,
        module_id: String,
        version: u32,
        description: String,
        required: bool,
        /// Verhältnis zur Bibliothek.
        status: ModuleStatus,
        body: Vec<ElView>,
    },
}

/// Stand eines Moduls im Dokument gegenüber der Bibliothek.
#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq)]
pub struct ModuleStatus {
    /// Im Dokument geändert oder nie in der Bibliothek gespeichert.
    pub local: bool,
    /// Neuere Version in der Bibliothek.
    pub newer: Option<u32>,
}

#[derive(Serialize)]
pub struct ColView {
    pub id: String,
    pub header: String,
    pub unit: Option<String>,
    pub unit_placement: UnitPlacement,
    pub align: Align,
    pub width: Option<f32>,
}

#[derive(Serialize)]
pub struct ItemView {
    pub id: String,
    pub text: String,
}

/// Eingabefeld innerhalb eines LaTeX-Elements.
#[derive(Serialize)]
pub struct LatexFieldView {
    pub path: String,
    pub label: String,
    /// `text`, `raw` (LaTeX des Anwenders) oder `image`.
    pub kind: &'static str,
    pub value: String,
    pub numeric: bool,
}

#[derive(Serialize)]
pub struct RowView {
    pub id: String,
    pub cells: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElView {
    Heading {
        level: u8,
        text: String,
        field: Option<String>,
        numeric: bool,
    },
    Paragraph {
        text: String,
        field: Option<String>,
        numeric: bool,
    },
    Table {
        /// Pfad des Tabellenelements (für Spaltenverschiebung).
        path: String,
        /// Pfad des Tabellenfeldes (für Werte).
        rows_path: String,
        caption: Option<String>,
        columns: Vec<ColView>,
        rows: Vec<RowView>,
    },
    Image {
        field: String,
        value: Option<String>,
        /// Breite als Anteil der verfügbaren Zeilenbreite.
        width: f32,
    },
    Row {
        children: Vec<ElView>,
    },
    Module {
        name: String,
        children: Vec<ElView>,
    },
    List {
        /// Pfad des Tabellenfeldes mit den Punkten.
        path: String,
        ordered: bool,
        items: Vec<ItemView>,
    },
    Latex {
        source: String,
        /// Eingabefelder des Quelltexts, jedes einmal.
        fields: Vec<LatexFieldView>,
    },
}

pub fn doc_view(
    path: &Path,
    ws: &Workspace,
    g: &Generated,
    status: &dyn Fn(&ModuleDef) -> ModuleStatus,
) -> DocView {
    let p = ws.project();
    let parts = p
        .parts
        .iter()
        .map(|part| {
            let start = g.part_range(&part.id).map_or(0, |r| r.start);
            let offset = byte_to_utf16(&g.text, start);
            let line = g.line_of(start);
            match &part.content {
                PartContent::Latex { source } => PartView::Latex {
                    id: part.id.to_string(),
                    offset,
                    line,
                    source: source.clone(),
                },
                PartContent::Module(inst) => {
                    let def = p.module(&inst.module);
                    PartView::Module {
                        id: part.id.to_string(),
                        offset,
                        line,
                        name: def.map_or_else(|| inst.module.to_string(), |d| d.name.clone()),
                        module: inst.module.to_string(),
                        module_id: inst.module.id.to_string(),
                        version: inst.module.version,
                        description: def.map_or_else(String::new, |d| d.description.clone()),
                        required: inst.required,
                        status: def.map(status).unwrap_or_default(),
                        body: def.map_or_else(Vec::new, |d| elements(p, inst, "", &d.body, 0)),
                    }
                }
            }
        })
        .collect();
    let notices = g
        .notices
        .iter()
        .map(|n| NoticeView {
            message: n.message.clone(),
            place: n.origin.to_string(),
            part: n.origin.part().map(ToString::to_string),
        })
        .collect();
    let modules = p
        .modules
        .iter()
        .map(|d| ModuleSummary {
            id: d.id.to_string(),
            version: d.version,
            name: d.name.clone(),
            kinds: content_kinds(d, &|r| p.module(r))
                .map(|k| k.into_iter().collect())
                .unwrap_or_default(),
        })
        .collect();
    DocView {
        path: path.display().to_string(),
        title: p.title.clone(),
        revision: ws.revision(),
        dirty: ws.is_dirty(),
        can_undo: ws.can_undo(),
        can_redo: ws.can_redo(),
        source: g.text.clone(),
        parts,
        notices,
        modules,
    }
}

fn text_of(p: &Project, inst: &ModuleInstance, scope: &str, t: &Text) -> (String, Option<String>) {
    match t {
        Text::Literal(s) => (s.clone(), None),
        Text::Field(id) => {
            let path = format!("{scope}{id}");
            let v = match p.effective_value(inst, &path) {
                Some(Value::Text(s) | Value::Number(s)) => s.clone(),
                _ => String::new(),
            };
            (v, Some(path))
        }
    }
}

fn is_numeric(p: &Project, inst: &ModuleInstance, path: Option<&str>) -> bool {
    path.and_then(|path| p.scope_module(&inst.module, path))
        .and_then(|(def, f)| def.field(f))
        .is_some_and(|f| f.kind == formavium_core::FieldKind::Number)
}

fn elements(
    p: &Project,
    inst: &ModuleInstance,
    scope: &str,
    body: &[Element],
    depth: usize,
) -> Vec<ElView> {
    if depth > 16 {
        return Vec::new();
    }
    body.iter()
        .map(|el| match &el.kind {
            ElementKind::Heading { level, text } => {
                let (text, field) = text_of(p, inst, scope, text);
                let numeric = is_numeric(p, inst, field.as_deref());
                ElView::Heading {
                    level: *level,
                    text,
                    field,
                    numeric,
                }
            }
            ElementKind::Paragraph { text } => {
                let (text, field) = text_of(p, inst, scope, text);
                let numeric = is_numeric(p, inst, field.as_deref());
                ElView::Paragraph {
                    text,
                    field,
                    numeric,
                }
            }
            ElementKind::Table {
                columns,
                rows,
                caption,
            } => {
                let path = format!("{scope}{}", el.id);
                let rows_path = format!("{scope}{rows}");
                let cols = ordered_columns(inst, &path, columns);
                let data = match p.effective_value(inst, &rows_path) {
                    Some(Value::Table(r)) => r.clone(),
                    _ => Vec::new(),
                };
                ElView::Table {
                    caption: caption.as_ref().map(|c| text_of(p, inst, scope, c).0),
                    columns: cols
                        .iter()
                        .map(|c| ColView {
                            id: c.id.to_string(),
                            header: text_of(p, inst, scope, &c.header).0,
                            unit: c.unit.clone(),
                            unit_placement: c.unit_placement,
                            align: c.align,
                            width: c.width,
                        })
                        .collect(),
                    rows: data
                        .iter()
                        .map(|r| RowView {
                            id: r.id.to_string(),
                            cells: cols
                                .iter()
                                .map(|c| r.cells.get(&c.id).cloned().unwrap_or_default())
                                .collect(),
                        })
                        .collect(),
                    path,
                    rows_path,
                }
            }
            ElementKind::Image { path, width } => {
                let field = format!("{scope}{path}");
                let value = match p.effective_value(inst, &field) {
                    Some(Value::Image(s)) => Some(s.clone()),
                    _ => None,
                };
                ElView::Image {
                    field,
                    value,
                    width: width.clamp(0.05, 1.0),
                }
            }
            ElementKind::Row { children } => ElView::Row {
                children: elements(p, inst, scope, children, depth),
            },
            ElementKind::Module { module } => {
                let sub = format!("{scope}{}/", el.id);
                match p.module(module) {
                    Some(d) => ElView::Module {
                        name: d.name.clone(),
                        children: elements(p, inst, &sub, &d.body, depth + 1),
                    },
                    None => ElView::Module {
                        name: module.to_string(),
                        children: Vec::new(),
                    },
                }
            }
            ElementKind::List { items, ordered } => {
                let path = format!("{scope}{items}");
                let rows = match p.effective_value(inst, &path) {
                    Some(Value::Table(r)) => r.clone(),
                    _ => Vec::new(),
                };
                ElView::List {
                    ordered: *ordered,
                    items: rows
                        .iter()
                        .map(|r| ItemView {
                            id: r.id.to_string(),
                            text: r
                                .cells
                                .get(&LIST_COLUMN.into())
                                .cloned()
                                .unwrap_or_default(),
                        })
                        .collect(),
                    path,
                }
            }
            ElementKind::Latex { source } => {
                let mut fields: Vec<LatexFieldView> = Vec::new();
                for segment in segments(source) {
                    let (id, kind) = match segment {
                        Segment::Text(_) => continue,
                        Segment::Field(id) => (id, "text"),
                        Segment::Raw(id) => (id, "raw"),
                        Segment::Image { field, .. } => (field, "image"),
                    };
                    let path = format!("{scope}{id}");
                    if fields.iter().any(|f| f.path == path) {
                        continue;
                    }
                    let def = p
                        .scope_module(&inst.module, &path)
                        .and_then(|(def, f)| def.field(f));
                    let value = match p.effective_value(inst, &path) {
                        Some(Value::Text(s) | Value::Number(s) | Value::Image(s)) => s.clone(),
                        _ => String::new(),
                    };
                    fields.push(LatexFieldView {
                        label: def.map_or_else(|| id.to_owned(), |f| f.label.clone()),
                        numeric: def.is_some_and(|f| f.kind == formavium_core::FieldKind::Number),
                        path,
                        kind,
                        value,
                    });
                }
                ElView::Latex {
                    source: source.clone(),
                    fields,
                }
            }
        })
        .collect()
}

#[derive(Serialize)]
pub struct DiagView {
    pub severity: &'static str,
    pub message: String,
    pub line: Option<u32>,
    pub place: Option<String>,
    pub part: Option<String>,
}

#[derive(Serialize)]
pub struct PreviewView {
    /// `empty`, `current`, `stale` oder `failed`.
    pub status: &'static str,
    pub shown_revision: Option<Revision>,
    pub failed_revision: Option<Revision>,
    /// Eine neuere Revision wird gerade kompiliert oder wartet.
    pub pending: bool,
    pub sha256: Option<String>,
    pub diagnostics: Vec<DiagView>,
    /// Diagnosen beziehen sich auf die aktuelle Revision (Zeilen stimmen).
    pub diagnostics_current: bool,
    pub duration_ms: Option<u128>,
    pub warnings: usize,
}

pub(crate) fn preview_view(
    preview: &Preview,
    current: Revision,
    requested: Revision,
    history: &VecDeque<(Revision, Arc<Generated>)>,
    last: Option<(Revision, u128, usize)>,
) -> PreviewView {
    let (status, shown_revision, failed_revision) = match preview.status() {
        PreviewStatus::Empty => ("empty", None, None),
        PreviewStatus::Current { revision } => ("current", Some(revision), None),
        PreviewStatus::Stale { shown, failed } => ("stale", Some(shown), Some(failed)),
        PreviewStatus::Failed { failed } => ("failed", None, Some(failed)),
    };
    let newest = shown_revision.max(failed_revision).unwrap_or(0);
    let generated =
        failed_revision.and_then(|f| history.iter().find(|(r, _)| *r == f).map(|(_, g)| g));
    let diagnostics = preview
        .failure()
        .map(|f| {
            f.diagnostics
                .iter()
                .map(|d| {
                    let origin = match (d.file.as_deref(), d.line, generated) {
                        (Some("main.tex"), Some(line), Some(g)) => g.origin_at_line(line),
                        _ => None,
                    };
                    DiagView {
                        severity: match d.severity {
                            Severity::Error => "error",
                            Severity::Warning => "warning",
                        },
                        message: d.message.clone(),
                        line: d.line,
                        place: origin.map(ToString::to_string),
                        part: origin.and_then(|o| o.part()).map(ToString::to_string),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    PreviewView {
        status,
        shown_revision,
        failed_revision,
        pending: requested > newest,
        sha256: preview.shown().map(|s| s.pdf.sha256.clone()),
        diagnostics,
        diagnostics_current: failed_revision == Some(current),
        duration_ms: last.map(|l| l.1),
        warnings: last.map_or(0, |l| l.2),
    }
}
