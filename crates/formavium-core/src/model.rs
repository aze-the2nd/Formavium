//! Dokumentmodell.
//!
//! Ein [`Project`] enthält geordnete Dokumentteile (freies LaTeX oder
//! Modulinstanzen) und die Moduldefinitionen in den verwendeten Versionen.
//! Das Dokument trägt damit alle benötigten Modulstände mit und lässt sich
//! ohne Bibliothek öffnen.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Aktuelle Schemaversion der `.fvm`-Datei.
pub const SCHEMA_VERSION: u32 = 1;
/// Kennzeichen im Dateikopf.
pub const FORMAT_TAG: &str = "formavium";

/// Stabile Kennung für Teile, Elemente, Felder, Spalten und Zeilen.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    pub fn new(s: impl Into<String>) -> Self {
        Id(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Id(s.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub schema: u32,
    pub title: String,
    /// Freies LaTeX vor `\begin{document}`, einschließlich `\documentclass`.
    pub preamble: String,
    pub parts: Vec<Part>,
    /// Moduldefinitionen in den von diesem Dokument verwendeten Versionen.
    #[serde(default)]
    pub modules: Vec<ModuleDef>,
    /// Zähler für neue Kennungen. Kennungen werden nie wiederverwendet.
    pub next_id: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: Id,
    pub content: PartContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartContent {
    /// Frei geschriebener LaTeX-Quelltext, unverändert ausgegeben.
    Latex { source: String },
    /// Eine Platzierung eines Moduls.
    Module(ModuleInstance),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModuleRef {
    pub id: Id,
    pub version: u32,
}

impl fmt::Display for ModuleRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}

/// Eine Platzierung eines Moduls mit eigenen Werten und lokalen Anpassungen.
///
/// Feldpfade: `feld` für Felder des Moduls selbst, `element/feld` für Felder
/// eines verschachtelten Untermoduls (beliebig tief).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleInstance {
    pub module: ModuleRef,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
    /// Lokale Spaltenreihenfolge je Tabelle (Pfad zum Tabellenelement).
    #[serde(default)]
    pub column_order: BTreeMap<String, Vec<Id>>,
    /// Lokal angepasste Spalten je Tabelle (Pfad zum Tabellenelement). Ein
    /// Eintrag ersetzt für diese Verwendung die Spalten der Definition:
    /// ergänzte, entfernte oder umbenannte Spalten. Die Kennungen
    /// unveränderter Spalten bleiben die der Definition.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub columns: BTreeMap<String, Vec<Column>>,
    /// Von der Vorlage als erforderlich gekennzeichnet.
    #[serde(default)]
    pub required: bool,
}

impl ModuleInstance {
    pub fn new(module: ModuleRef) -> Self {
        ModuleInstance {
            module,
            values: BTreeMap::new(),
            column_order: BTreeMap::new(),
            columns: BTreeMap::new(),
            required: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleDef {
    pub id: Id,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    pub body: Vec<Element>,
}

impl ModuleDef {
    pub fn reference(&self) -> ModuleRef {
        ModuleRef {
            id: self.id.clone(),
            version: self.version,
        }
    }

    pub fn field(&self, id: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.id.as_str() == id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDef {
    pub id: Id,
    pub label: String,
    pub kind: FieldKind,
    /// Bewusst gesetzter Standardwert der Vorlage.
    #[serde(default)]
    pub default: Option<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Number,
    Image,
    Table,
}

/// Wert eines Eingabefeldes. Text und Zahlen sind Inhalte, kein LaTeX.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    Text(String),
    /// Zahl in der eingegebenen Schreibweise (z. B. mit Dezimalkomma).
    Number(String),
    /// Relativer Pfad zu einer Bilddatei neben der Arbeitsdatei.
    Image(String),
    Table(Vec<Row>),
}

impl Value {
    pub fn kind(&self) -> FieldKind {
        match self {
            Value::Text(_) => FieldKind::Text,
            Value::Number(_) => FieldKind::Number,
            Value::Image(_) => FieldKind::Image,
            Value::Table(_) => FieldKind::Table,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub id: Id,
    /// Zellwerte je Spaltenkennung.
    pub cells: BTreeMap<Id, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: Id,
    #[serde(flatten)]
    pub kind: ElementKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElementKind {
    Heading {
        level: u8,
        text: Text,
    },
    Paragraph {
        text: Text,
    },
    /// Tabelle, deren Zeilen aus einem Tabellenfeld stammen.
    Table {
        columns: Vec<Column>,
        rows: Id,
        #[serde(default)]
        caption: Option<Text>,
    },
    Image {
        /// Bildfeld mit dem relativen Pfad.
        path: Id,
        /// Breite als Anteil der verfügbaren Zeilenbreite.
        width: f32,
    },
    /// Horizontale Anordnung.
    Row {
        children: Vec<Element>,
    },
    /// Verschachteltes Untermodul.
    Module {
        module: ModuleRef,
    },
    /// Vom Modulautor geschriebener LaTeX-Quelltext.
    Latex {
        source: String,
    },
}

/// Text eines Elements: fest vorgegeben oder aus einem Eingabefeld.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Text {
    Literal(String),
    Field(Id),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub id: Id,
    pub header: Text,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub unit_placement: UnitPlacement,
    #[serde(default)]
    pub align: Align,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitPlacement {
    #[default]
    Header,
    Value,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

impl Project {
    pub fn new(title: impl Into<String>, preamble: impl Into<String>) -> Self {
        Project {
            format: FORMAT_TAG.to_owned(),
            schema: SCHEMA_VERSION,
            title: title.into(),
            preamble: preamble.into(),
            parts: Vec::new(),
            modules: Vec::new(),
            next_id: 1,
        }
    }

    /// Erzeugt eine neue, im Projekt eindeutige Kennung.
    pub fn fresh_id(&mut self, prefix: &str) -> Id {
        let id = Id(format!("{prefix}{}", self.next_id));
        self.next_id += 1;
        id
    }

    pub fn module(&self, r: &ModuleRef) -> Option<&ModuleDef> {
        self.modules
            .iter()
            .find(|m| m.id == r.id && m.version == r.version)
    }

    /// Fügt eine Moduldefinition hinzu, falls diese Version noch fehlt.
    pub fn add_module(&mut self, def: ModuleDef) {
        if self.module(&def.reference()).is_none() {
            self.modules.push(def);
        }
    }

    pub fn part_index(&self, id: &Id) -> Option<usize> {
        self.parts.iter().position(|p| &p.id == id)
    }

    pub fn part(&self, id: &Id) -> Option<&Part> {
        self.parts.iter().find(|p| &p.id == id)
    }

    pub fn instance(&self, id: &Id) -> Option<&ModuleInstance> {
        match &self.part(id)?.content {
            PartContent::Module(m) => Some(m),
            PartContent::Latex { .. } => None,
        }
    }

    pub fn instance_mut(&mut self, id: &Id) -> Option<&mut ModuleInstance> {
        match &mut self.parts.iter_mut().find(|p| &p.id == id)?.content {
            PartContent::Module(m) => Some(m),
            PartContent::Latex { .. } => None,
        }
    }

    /// Löst einen Pfad `a/b/feld` innerhalb einer Instanz auf: liefert das
    /// Modul, in dessen Gültigkeitsbereich das letzte Segment liegt.
    pub fn scope_module<'a, 's>(
        &'a self,
        root: &ModuleRef,
        path: &'s str,
    ) -> Option<(&'a ModuleDef, &'s str)> {
        let mut def = self.module(root)?;
        let mut segments: Vec<&str> = path.split('/').collect();
        let last = segments.pop()?;
        for seg in segments {
            let el = find_element(&def.body, seg)?;
            match &el.kind {
                ElementKind::Module { module } => def = self.module(module)?,
                _ => return None,
            }
        }
        Some((def, last))
    }

    /// Wert eines Feldes einer Instanz, sonst Standardwert der Definition.
    pub fn effective_value<'a>(
        &'a self,
        inst: &'a ModuleInstance,
        path: &str,
    ) -> Option<&'a Value> {
        if let Some(v) = inst.values.get(path) {
            return Some(v);
        }
        let (def, field) = self.scope_module(&inst.module, path)?;
        def.field(field)?.default.as_ref()
    }
}

/// Sucht ein Element im Modulkörper, auch in horizontalen Anordnungen, aber
/// nicht in Untermodulen (diese bilden einen eigenen Pfadbereich).
pub fn find_element<'a>(body: &'a [Element], id: &str) -> Option<&'a Element> {
    for el in body {
        if el.id.as_str() == id {
            return Some(el);
        }
        if let ElementKind::Row { children } = &el.kind {
            if let Some(found) = find_element(children, id) {
                return Some(found);
            }
        }
    }
    None
}
