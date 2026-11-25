//! Sicht der Bibliothek für die Modulauswahl.

use formavium_core::catalog::{content_kinds, ContentKind};
use formavium_core::library::{ModuleEntry, Source, TemplateEntry};
use formavium_core::{Element, ElementKind, ModuleDef, ModuleRef, PartContent, Project};
use serde::Serialize;

/// Aufbau in Kurzform, aus dem die Oberfläche eine Miniatur zeichnet.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outline {
    Heading,
    Paragraph,
    List,
    Table { cols: usize },
    Image,
    Latex,
    Module,
    Row { children: Vec<Outline> },
}

#[derive(Serialize)]
pub struct LibModuleView {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub kinds: Vec<ContentKind>,
    pub source: Source,
    /// Neueste Version dieser Kennung.
    pub latest: bool,
    pub outline: Vec<Outline>,
}

#[derive(Serialize)]
pub struct LibTemplateView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: Source,
    pub outline: Vec<Outline>,
}

#[derive(Serialize)]
pub struct LibraryView {
    /// Ordner der eigenen Bibliothek.
    pub dir: Option<String>,
    pub modules: Vec<LibModuleView>,
    pub templates: Vec<LibTemplateView>,
}

fn outline(
    body: &[Element],
    lookup: &dyn Fn(&ModuleRef) -> Option<ModuleDef>,
    depth: usize,
) -> Vec<Outline> {
    let mut out = Vec::new();
    for el in body {
        match &el.kind {
            ElementKind::Heading { .. } => out.push(Outline::Heading),
            ElementKind::Paragraph { .. } => out.push(Outline::Paragraph),
            ElementKind::List { .. } => out.push(Outline::List),
            ElementKind::Table { columns, .. } => out.push(Outline::Table {
                cols: columns.len(),
            }),
            ElementKind::Image { .. } => out.push(Outline::Image),
            ElementKind::Latex { .. } => out.push(Outline::Latex),
            ElementKind::Row { children } => out.push(Outline::Row {
                children: outline(children, lookup, depth),
            }),
            // Untermodule mit ihrem Aufbau zeigen, nicht als leere Kachel.
            ElementKind::Module { module } => match lookup(module) {
                Some(def) if depth < 4 => out.extend(outline(&def.body, lookup, depth + 1)),
                _ => out.push(Outline::Module),
            },
        }
    }
    out
}

pub fn library_view(
    dir: Option<&std::path::Path>,
    modules: &[ModuleEntry],
    templates: &[TemplateEntry],
) -> LibraryView {
    let lookup = |r: &ModuleRef| {
        modules
            .iter()
            .map(|e| &e.def)
            .find(|d| d.reference() == *r)
            .cloned()
    };
    let module_views = modules
        .iter()
        .map(|e| LibModuleView {
            id: e.def.id.to_string(),
            version: e.def.version,
            name: e.def.name.clone(),
            description: e.def.description.clone(),
            kinds: content_kinds(&e.def, &|r| {
                modules.iter().map(|m| &m.def).find(|d| d.reference() == *r)
            })
            .map(|k| k.into_iter().collect())
            .unwrap_or_default(),
            source: e.source,
            latest: modules
                .iter()
                .filter(|o| o.def.id == e.def.id)
                .all(|o| o.def.version <= e.def.version),
            outline: outline(&e.def.body, &lookup, 0),
        })
        .collect();
    let template_views = templates
        .iter()
        .map(|t| LibTemplateView {
            id: t.id.clone(),
            name: t.project.title.clone(),
            description: t.project.description.clone(),
            source: t.source,
            outline: project_outline(&t.project),
        })
        .collect();
    LibraryView {
        dir: dir.map(|d| d.display().to_string()),
        modules: module_views,
        templates: template_views,
    }
}

/// Aufbau eines ganzen Dokuments: freie Teile als LaTeX, Module mit Inhalt.
fn project_outline(p: &Project) -> Vec<Outline> {
    let lookup = |r: &ModuleRef| p.module(r).cloned();
    let mut out = Vec::new();
    for part in &p.parts {
        match &part.content {
            PartContent::Latex { .. } => out.push(Outline::Latex),
            PartContent::Module(inst) => match p.module(&inst.module) {
                Some(def) => out.extend(outline(&def.body, &lookup, 0)),
                None => out.push(Outline::Module),
            },
        }
    }
    out
}
