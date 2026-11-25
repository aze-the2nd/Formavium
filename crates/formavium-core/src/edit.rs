//! Bearbeitung über fachliche Befehle und identifizierbare Revisionen.
//!
//! Jede bestätigte Bearbeitung erzeugt eine neue Revision. Rückgängig und
//! Wiederholen erzeugen ebenfalls neue Revisionsnummern, damit Ansichten und
//! Renderergebnisse eindeutig einem Stand zugeordnet bleiben.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::BTreeMap;

use crate::model::{
    find_element, Align, Column, ElementKind, FieldKind, Id, Part, PartContent, Project, Row, Text,
    UnitPlacement, Value,
};

pub type Revision = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    SetPreamble {
        source: String,
    },
    SetLatex {
        part: Id,
        source: String,
    },
    SetValue {
        part: Id,
        path: String,
        value: Value,
    },
    ClearValue {
        part: Id,
        path: String,
    },
    /// Spalte einer Tabelle in dieser Instanz verschieben. Kopf, Werte und
    /// Einheit wandern gemeinsam, weil sie an der Spaltenkennung hängen.
    MoveColumn {
        part: Id,
        table: String,
        from: usize,
        to: usize,
    },
    /// Zelle einer Tabelle setzen (`path` ist das Tabellenfeld).
    SetCell {
        part: Id,
        path: String,
        row: Id,
        column: Id,
        value: String,
    },
    /// Leere Zeile an `index` einfügen.
    InsertRow {
        part: Id,
        path: String,
        index: usize,
    },
    RemoveRow {
        part: Id,
        path: String,
        row: Id,
    },
    MoveRow {
        part: Id,
        path: String,
        row: Id,
        to: usize,
    },
    /// Spalte nur in dieser Verwendung ergänzen (`table` ist das
    /// Tabellenelement). Die Moduldefinition bleibt unverändert.
    InsertColumn {
        part: Id,
        table: String,
        index: usize,
        header: String,
    },
    /// Spalte in dieser Verwendung entfernen, samt ihrer Zellwerte.
    RemoveColumn {
        part: Id,
        table: String,
        column: Id,
    },
    /// Spaltenkopf ändern: ein Kopf aus einem Feld setzt dessen Wert, ein
    /// fester Kopf wird für diese Verwendung überschrieben.
    SetColumnHeader {
        part: Id,
        table: String,
        column: Id,
        header: String,
    },
    InsertPart {
        index: usize,
        content: PartContent,
    },
    RemovePart {
        part: Id,
    },
    MovePart {
        part: Id,
        to: usize,
    },
    /// Ersetzt aufeinanderfolgende Teile durch einen freien LaTeX-Teil.
    /// Module verlieren dabei ihre visuelle Zuordnung.
    ReplacePartsWithLatex {
        parts: Vec<Id>,
        source: String,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum EditError {
    #[error("Bearbeitung beruht auf Revision {base}, aktuell ist {current}")]
    StaleRevision { base: Revision, current: Revision },
    #[error("Dokumentteil {0} nicht gefunden")]
    UnknownPart(Id),
    #[error("Dokumentteil {0} hat nicht den erwarteten Typ")]
    WrongPartKind(Id),
    #[error("Feld „{0}“ existiert in diesem Modul nicht")]
    UnknownField(String),
    #[error("Feld „{path}“ erwartet {expected:?}, erhalten {got:?}")]
    WrongValueKind {
        path: String,
        expected: FieldKind,
        got: FieldKind,
    },
    #[error("Tabelle „{0}“ existiert in diesem Modul nicht")]
    UnknownTable(String),
    #[error("Zeile {0} existiert in dieser Tabelle nicht")]
    UnknownRow(Id),
    #[error("Spalte {0} existiert in dieser Tabelle nicht")]
    UnknownColumn(Id),
    #[error("Die letzte Spalte einer Tabelle lässt sich nicht entfernen")]
    LastColumn,
    #[error("Position {0} liegt außerhalb des gültigen Bereichs")]
    OutOfRange(usize),
    #[error("Moduldefinition {0} fehlt im Dokument")]
    MissingModule(String),
    #[error("Nichts zum Rückgängigmachen oder Wiederholen")]
    NothingToUndo,
}

/// Ein geöffnetes Dokument mit Revisionen und Verlauf.
#[derive(Debug)]
pub struct Workspace {
    project: Project,
    revision: Revision,
    undo: Vec<Project>,
    redo: Vec<Project>,
    saved_revision: Option<Revision>,
}

const UNDO_LIMIT: usize = 200;

impl Workspace {
    pub fn new(project: Project) -> Self {
        Workspace {
            project,
            revision: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            saved_revision: None,
        }
    }

    /// Ein frisch geladenes Dokument gilt als gespeichert.
    pub fn opened(project: Project) -> Self {
        let mut ws = Self::new(project);
        ws.saved_revision = Some(ws.revision);
        ws
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        self.saved_revision != Some(self.revision)
    }

    pub fn mark_saved(&mut self, revision: Revision) {
        self.saved_revision = Some(revision);
    }

    /// Wendet einen Befehl an. Liefert die neue Revision und, beim Einfügen,
    /// die Kennung des neuen Teils.
    pub fn apply(
        &mut self,
        base: Revision,
        cmd: Command,
    ) -> Result<(Revision, Option<Id>), EditError> {
        if base != self.revision {
            return Err(EditError::StaleRevision {
                base,
                current: self.revision,
            });
        }
        let mut next = self.project.clone();
        let created = apply_to(&mut next, cmd)?;
        self.push_history(next);
        Ok((self.revision, created))
    }

    fn push_history(&mut self, next: Project) {
        let prev = std::mem::replace(&mut self.project, next);
        self.undo.push(prev);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
    }

    pub fn undo(&mut self) -> Result<Revision, EditError> {
        let prev = self.undo.pop().ok_or(EditError::NothingToUndo)?;
        self.redo.push(std::mem::replace(&mut self.project, prev));
        self.revision += 1;
        Ok(self.revision)
    }

    pub fn redo(&mut self) -> Result<Revision, EditError> {
        let next = self.redo.pop().ok_or(EditError::NothingToUndo)?;
        self.undo.push(std::mem::replace(&mut self.project, next));
        self.revision += 1;
        Ok(self.revision)
    }
}

fn apply_to(p: &mut Project, cmd: Command) -> Result<Option<Id>, EditError> {
    match cmd {
        Command::SetPreamble { source } => p.preamble = source,
        Command::SetLatex { part, source } => {
            let pt = p
                .parts
                .iter_mut()
                .find(|x| x.id == part)
                .ok_or_else(|| EditError::UnknownPart(part.clone()))?;
            match &mut pt.content {
                PartContent::Latex { source: s } => *s = source,
                PartContent::Module(_) => return Err(EditError::WrongPartKind(part)),
            }
        }
        Command::SetValue { part, path, value } => {
            let inst = instance(p, &part)?;
            let (def, field) = p
                .scope_module(&inst.module, &path)
                .ok_or_else(|| EditError::UnknownField(path.clone()))?;
            let fd = def
                .field(field)
                .ok_or_else(|| EditError::UnknownField(path.clone()))?;
            if fd.kind != value.kind() {
                return Err(EditError::WrongValueKind {
                    path,
                    expected: fd.kind,
                    got: value.kind(),
                });
            }
            p.instance_mut(&part)
                .expect("geprüft")
                .values
                .insert(path, value);
        }
        Command::ClearValue { part, path } => {
            instance(p, &part)?;
            p.instance_mut(&part).expect("geprüft").values.remove(&path);
        }
        Command::MoveColumn {
            part,
            table,
            from,
            to,
        } => {
            let inst = instance(p, &part)?;
            let (def, el_id) = p
                .scope_module(&inst.module, &table)
                .ok_or_else(|| EditError::UnknownTable(table.clone()))?;
            let columns = match find_element(&def.body, el_id).map(|e| &e.kind) {
                Some(ElementKind::Table { columns, .. }) => columns,
                _ => return Err(EditError::UnknownTable(table)),
            };
            let mut order: Vec<Id> = crate::latex::ordered_columns(inst, &table, columns)
                .into_iter()
                .map(|c| c.id.clone())
                .collect();
            if from >= order.len() {
                return Err(EditError::OutOfRange(from));
            }
            if to >= order.len() {
                return Err(EditError::OutOfRange(to));
            }
            let col = order.remove(from);
            order.insert(to, col);
            p.instance_mut(&part)
                .expect("geprüft")
                .column_order
                .insert(table, order);
        }
        Command::SetCell {
            part,
            path,
            row,
            column,
            value,
        } => {
            let mut rows = table_rows(p, &part, &path)?;
            let r = rows
                .iter_mut()
                .find(|r| r.id == row)
                .ok_or(EditError::UnknownRow(row))?;
            r.cells.insert(column, value);
            set_rows(p, &part, path, rows);
        }
        Command::InsertRow { part, path, index } => {
            let mut rows = table_rows(p, &part, &path)?;
            if index > rows.len() {
                return Err(EditError::OutOfRange(index));
            }
            let id = loop {
                let id = p.fresh_id("r");
                if rows.iter().all(|r| r.id != id) {
                    break id;
                }
            };
            rows.insert(
                index,
                Row {
                    id: id.clone(),
                    cells: BTreeMap::new(),
                },
            );
            set_rows(p, &part, path, rows);
            return Ok(Some(id));
        }
        Command::RemoveRow { part, path, row } => {
            let mut rows = table_rows(p, &part, &path)?;
            let i = rows
                .iter()
                .position(|r| r.id == row)
                .ok_or(EditError::UnknownRow(row))?;
            rows.remove(i);
            set_rows(p, &part, path, rows);
        }
        Command::MoveRow {
            part,
            path,
            row,
            to,
        } => {
            let mut rows = table_rows(p, &part, &path)?;
            let i = rows
                .iter()
                .position(|r| r.id == row)
                .ok_or(EditError::UnknownRow(row))?;
            if to >= rows.len() {
                return Err(EditError::OutOfRange(to));
            }
            let r = rows.remove(i);
            rows.insert(to, r);
            set_rows(p, &part, path, rows);
        }
        Command::InsertColumn {
            part,
            table,
            index,
            header,
        } => {
            let (mut columns, _) = local_columns(p, &part, &table)?;
            if index > columns.len() {
                return Err(EditError::OutOfRange(index));
            }
            let id = loop {
                let id = p.fresh_id("c");
                if columns.iter().all(|c| c.id != id) {
                    break id;
                }
            };
            columns.insert(
                index,
                Column {
                    id: id.clone(),
                    header: Text::Literal(header),
                    unit: None,
                    unit_placement: UnitPlacement::default(),
                    align: Align::default(),
                },
            );
            set_columns(p, &part, table, columns);
            return Ok(Some(id));
        }
        Command::RemoveColumn {
            part,
            table,
            column,
        } => {
            let (mut columns, rows_path) = local_columns(p, &part, &table)?;
            let i = columns
                .iter()
                .position(|c| c.id == column)
                .ok_or_else(|| EditError::UnknownColumn(column.clone()))?;
            if columns.len() == 1 {
                return Err(EditError::LastColumn);
            }
            columns.remove(i);
            set_columns(p, &part, table, columns);
            // Werte der entfernten Spalte nicht unsichtbar im Dokument behalten.
            let inst = p.instance_mut(&part).expect("geprüft");
            if let Some(Value::Table(rows)) = inst.values.get_mut(&rows_path) {
                for r in rows {
                    r.cells.remove(&column);
                }
            }
        }
        Command::SetColumnHeader {
            part,
            table,
            column,
            header,
        } => {
            let (mut columns, _) = local_columns(p, &part, &table)?;
            let col = columns
                .iter_mut()
                .find(|c| c.id == column)
                .ok_or(EditError::UnknownColumn(column))?;
            match &col.header {
                Text::Field(field) => {
                    let path = format!("{}{field}", scope_prefix(&table));
                    p.instance_mut(&part)
                        .expect("geprüft")
                        .values
                        .insert(path, Value::Text(header));
                }
                Text::Literal(_) => {
                    col.header = Text::Literal(header);
                    set_columns(p, &part, table, columns);
                }
            }
        }
        Command::InsertPart { index, content } => {
            if index > p.parts.len() {
                return Err(EditError::OutOfRange(index));
            }
            if let PartContent::Module(inst) = &content {
                if p.module(&inst.module).is_none() {
                    return Err(EditError::MissingModule(inst.module.to_string()));
                }
            }
            let id = p.fresh_id("p");
            p.parts.insert(
                index,
                Part {
                    id: id.clone(),
                    content,
                },
            );
            return Ok(Some(id));
        }
        Command::RemovePart { part } => {
            let i = p.part_index(&part).ok_or(EditError::UnknownPart(part))?;
            p.parts.remove(i);
        }
        Command::MovePart { part, to } => {
            let i = p.part_index(&part).ok_or(EditError::UnknownPart(part))?;
            if to >= p.parts.len() {
                return Err(EditError::OutOfRange(to));
            }
            let pt = p.parts.remove(i);
            p.parts.insert(to, pt);
        }
        Command::ReplacePartsWithLatex { parts, source } => {
            let first = parts.first().ok_or(EditError::OutOfRange(0))?;
            let start = p
                .part_index(first)
                .ok_or_else(|| EditError::UnknownPart(first.clone()))?;
            for (k, id) in parts.iter().enumerate() {
                if p.parts.get(start + k).map(|x| &x.id) != Some(id) {
                    return Err(EditError::UnknownPart(id.clone()));
                }
            }
            let id = p.fresh_id("p");
            p.parts.splice(
                start..start + parts.len(),
                [Part {
                    id: id.clone(),
                    content: PartContent::Latex { source },
                }],
            );
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// Pfadpräfix eines Elements: `a/b/tabelle` → `a/b/`.
fn scope_prefix(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..=i])
}

/// Aktuelle Zeilen eines Tabellenfeldes (Wert der Instanz oder Standard).
fn table_rows(p: &Project, part: &Id, path: &str) -> Result<Vec<Row>, EditError> {
    let inst = instance(p, part)?;
    let kind = p
        .scope_module(&inst.module, path)
        .and_then(|(def, field)| def.field(field))
        .map(|f| f.kind)
        .ok_or_else(|| EditError::UnknownField(path.to_owned()))?;
    if kind != FieldKind::Table {
        return Err(EditError::WrongValueKind {
            path: path.to_owned(),
            expected: kind,
            got: FieldKind::Table,
        });
    }
    Ok(match p.effective_value(inst, path) {
        Some(Value::Table(rows)) => rows.clone(),
        _ => Vec::new(),
    })
}

fn set_rows(p: &mut Project, part: &Id, path: String, rows: Vec<Row>) {
    p.instance_mut(part)
        .expect("geprüft")
        .values
        .insert(path, Value::Table(rows));
}

/// Effektive Spalten einer Tabelle in Anzeigereihenfolge und der Pfad ihres
/// Tabellenfeldes.
fn local_columns(p: &Project, part: &Id, table: &str) -> Result<(Vec<Column>, String), EditError> {
    let inst = instance(p, part)?;
    let (def, el_id) = p
        .scope_module(&inst.module, table)
        .ok_or_else(|| EditError::UnknownTable(table.to_owned()))?;
    match find_element(&def.body, el_id).map(|e| &e.kind) {
        Some(ElementKind::Table { columns, rows, .. }) => Ok((
            crate::latex::ordered_columns(inst, table, columns)
                .into_iter()
                .cloned()
                .collect(),
            format!("{}{rows}", scope_prefix(table)),
        )),
        _ => Err(EditError::UnknownTable(table.to_owned())),
    }
}

/// Speichert die lokale Spaltenliste. Sie steht bereits in
/// Anzeigereihenfolge, eine gesonderte Reihenfolge entfällt.
fn set_columns(p: &mut Project, part: &Id, table: String, columns: Vec<Column>) {
    let inst = p.instance_mut(part).expect("geprüft");
    inst.column_order.remove(&table);
    inst.columns.insert(table, columns);
}

fn instance<'a>(p: &'a Project, part: &Id) -> Result<&'a crate::model::ModuleInstance, EditError> {
    match p.part(part) {
        None => Err(EditError::UnknownPart(part.clone())),
        Some(pt) => match &pt.content {
            PartContent::Module(m) => Ok(m),
            PartContent::Latex { .. } => Err(EditError::WrongPartKind(part.clone())),
        },
    }
}
