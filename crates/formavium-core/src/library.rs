//! Bibliothek: wiederverwendbare Module und Vorlagen.
//!
//! Die Bibliothek ist lokal. Sie besteht aus den mitgelieferten, neutralen
//! Bausteinen und einem Ordner des Anwenders:
//!
//! ```text
//! Bibliothek/
//!   module/<kennung>@<version>.json     ein Modul je Datei und Version
//!   vorlagen/<kennung>/vorlage.fvm      eine Vorlage ist ein Dokument
//!   vorlagen/<kennung>/bilder/…         mit seinen Bildern
//! ```
//!
//! Ein Dokument trägt die verwendeten Modulstände selbst mit. Die Bibliothek
//! wird nur beim Einfügen, Veröffentlichen und für Hinweise auf neuere
//! Versionen gelesen.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::builtin;
use crate::io as fvm;
use crate::model::{
    find_element, Element, ElementKind, ModuleDef, ModuleRef, PartContent, Project,
};

const MODULE_FORMAT: &str = "formavium-module";
const MODULE_SCHEMA: u32 = 1;
const TEMPLATE_FILE: &str = "vorlage.fvm";

#[derive(Serialize, Deserialize)]
struct ModuleFile {
    format: String,
    schema: u32,
    module: ModuleDef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Mit der Anwendung geliefert, nicht änderbar.
    Builtin,
    /// Aus dem Bibliotheksordner des Anwenders.
    User,
}

#[derive(Clone, Debug)]
pub struct ModuleEntry {
    pub def: ModuleDef,
    pub source: Source,
}

#[derive(Clone, Debug)]
pub struct TemplateEntry {
    pub id: String,
    pub source: Source,
    pub project: Project,
    /// Ordner der Vorlage mit ihren Bildern (nur eigene Vorlagen).
    dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct Library {
    user: Option<PathBuf>,
}

/// Kennung aus einem Namen: Kleinbuchstaben, Ziffern und Bindestriche.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        let mapped = match c {
            'ä' | 'Ä' => "ae",
            'ö' | 'Ö' => "oe",
            'ü' | 'Ü' => "ue",
            'ß' => "ss",
            c if c.is_ascii_alphanumeric() => {
                out.push(c.to_ascii_lowercase());
                continue;
            }
            _ => "-",
        };
        if mapped == "-" && (out.is_empty() || out.ends_with('-')) {
            continue;
        }
        out.push_str(mapped);
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "unbenannt".into()
    } else {
        out
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl Library {
    /// Bibliothek mit dem Ordner des Anwenders. Ohne Ordner gibt es nur die
    /// mitgelieferten Bausteine.
    pub fn new(user: Option<PathBuf>) -> Self {
        Library { user }
    }

    pub fn user_dir(&self) -> Option<&Path> {
        self.user.as_deref()
    }

    fn writable(&self) -> io::Result<&Path> {
        self.user
            .as_deref()
            .ok_or_else(|| invalid("Kein Bibliotheksordner eingerichtet"))
    }

    // --- Module ---

    /// Alle Module in allen Versionen, mitgelieferte zuerst.
    pub fn modules(&self) -> Vec<ModuleEntry> {
        let mut all: Vec<ModuleEntry> = builtin::modules()
            .into_iter()
            .map(|def| ModuleEntry {
                def,
                source: Source::Builtin,
            })
            .collect();
        if let Some(dir) = &self.user {
            let mut own: Vec<ModuleDef> = fs::read_dir(dir.join("module"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| read_module(&e.path()).ok())
                .collect();
            own.sort_by(|a, b| (&a.name, &a.id, a.version).cmp(&(&b.name, &b.id, b.version)));
            // Eigene Module dürfen mitgelieferte Kennungen nicht überdecken.
            own.retain(|d| all.iter().all(|b| b.def.reference() != d.reference()));
            all.extend(own.into_iter().map(|def| ModuleEntry {
                def,
                source: Source::User,
            }));
        }
        all
    }

    pub fn module(&self, r: &ModuleRef) -> Option<ModuleDef> {
        self.modules()
            .into_iter()
            .map(|e| e.def)
            .find(|d| d.reference() == *r)
    }

    /// Höchste Version eines Moduls in der Bibliothek.
    pub fn latest_version(&self, id: &str) -> Option<u32> {
        self.modules()
            .iter()
            .filter(|e| e.def.id.as_str() == id)
            .map(|e| e.def.version)
            .max()
    }

    /// Das Modul samt aller Untermodule, die es (auch verschachtelt) nutzt.
    /// Fehlt ein Untermodul in `known` und in der Bibliothek, ist das ein Fehler.
    pub fn closure(&self, def: &ModuleDef, known: &[ModuleDef]) -> io::Result<Vec<ModuleDef>> {
        let all = self.modules();
        let mut out: Vec<ModuleDef> = vec![def.clone()];
        let mut i = 0;
        while i < out.len() {
            let mut refs = Vec::new();
            collect_refs(&out[i].body, &mut refs);
            for r in refs {
                if out.iter().any(|d| d.reference() == r) {
                    continue;
                }
                let found = known
                    .iter()
                    .find(|d| d.reference() == r)
                    .or_else(|| all.iter().map(|e| &e.def).find(|d| d.reference() == r))
                    .ok_or_else(|| invalid(format!("Untermodul {r} fehlt in der Bibliothek")))?;
                out.push(found.clone());
            }
            i += 1;
        }
        Ok(out)
    }

    /// Speichert ein Modul im Ordner des Anwenders. Gibt es die Kennung schon
    /// mit anderem Inhalt, entsteht die nächste Version. Liefert den
    /// gespeicherten Stand.
    pub fn save_module(&self, def: &ModuleDef) -> io::Result<ModuleDef> {
        let dir = self.writable()?.join("module");
        validate_module(def).map_err(invalid)?;
        let existing: Vec<ModuleDef> = self
            .modules()
            .into_iter()
            .map(|e| e.def)
            .filter(|d| d.id == def.id)
            .collect();
        // Unveränderter Inhalt: keine neue Version.
        if let Some(same) = existing.iter().find(|d| same_content(d, def)) {
            return Ok(same.clone());
        }
        let mut saved = def.clone();
        saved.version = existing
            .iter()
            .map(|d| d.version)
            .max()
            .map_or(1, |v| v + 1);
        fs::create_dir_all(&dir)?;
        let file = ModuleFile {
            format: MODULE_FORMAT.into(),
            schema: MODULE_SCHEMA,
            module: saved.clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| invalid(e.to_string()))?;
        fvm::write_atomic(
            &dir.join(format!("{}@{}.json", saved.id, saved.version)),
            json.as_bytes(),
        )
        .map_err(|e| invalid(e.to_string()))?;
        Ok(saved)
    }

    pub fn delete_module(&self, r: &ModuleRef) -> io::Result<()> {
        let path = self
            .writable()?
            .join("module")
            .join(format!("{}@{}.json", r.id, r.version));
        fs::remove_file(path)
    }

    // --- Vorlagen ---

    pub fn templates(&self) -> Vec<TemplateEntry> {
        let mut all: Vec<TemplateEntry> = builtin::templates()
            .into_iter()
            .map(|(id, project)| TemplateEntry {
                id: id.to_owned(),
                source: Source::Builtin,
                project,
                dir: None,
            })
            .collect();
        if let Some(dir) = &self.user {
            let mut own: Vec<TemplateEntry> = fs::read_dir(dir.join("vorlagen"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let (project, _) = fvm::load(&e.path().join(TEMPLATE_FILE)).ok()?;
                    Some(TemplateEntry {
                        id: e.file_name().to_string_lossy().into_owned(),
                        source: Source::User,
                        project,
                        dir: Some(e.path()),
                    })
                })
                .collect();
            own.sort_by(|a, b| a.project.title.cmp(&b.project.title));
            own.retain(|t| all.iter().all(|b| b.id != t.id));
            all.extend(own);
        }
        all
    }

    pub fn template(&self, id: &str) -> Option<TemplateEntry> {
        self.templates().into_iter().find(|t| t.id == id)
    }

    /// Speichert ein Dokument als Vorlage, mit den Bildern aus `resources`.
    /// `keep_values = false` leert die Eingaben, damit keine fremden Altdaten
    /// in neue Dokumente gelangen. Liefert die Kennung der Vorlage.
    pub fn save_template(
        &self,
        project: &Project,
        project_dir: &Path,
        resources: &BTreeSet<String>,
        keep_values: bool,
    ) -> io::Result<String> {
        let root = self.writable()?.join("vorlagen");
        let id = slug(&project.title);
        if builtin::templates().iter().any(|(b, _)| *b == id) {
            return Err(invalid(format!(
                "„{}“ heißt wie eine mitgelieferte Vorlage. Bitte einen anderen Titel wählen.",
                project.title
            )));
        }
        let dir = root.join(&id);
        let mut template = project.clone();
        if !keep_values {
            for part in &mut template.parts {
                if let PartContent::Module(inst) = &mut part.content {
                    inst.values.clear();
                }
            }
        }
        fs::create_dir_all(&dir)?;
        let mut kept = BTreeSet::new();
        for rel in resources {
            // Bilder geleerter Bildfelder werden nicht mitkopiert.
            if !keep_values && !used_outside_values(project, rel) {
                continue;
            }
            copy_resource(project_dir, &dir, rel)?;
            kept.insert(rel.clone());
        }
        fvm::save(&dir.join(TEMPLATE_FILE), &template, None).map_err(|e| invalid(e.to_string()))?;
        Ok(id)
    }

    pub fn delete_template(&self, id: &str) -> io::Result<()> {
        if id.is_empty() || id.contains(['/', '\\', '.']) {
            return Err(invalid("Ungültige Vorlagenkennung"));
        }
        fs::remove_dir_all(self.writable()?.join("vorlagen").join(id))
    }
}

impl TemplateEntry {
    /// Legt aus der Vorlage ein neues Dokument an: Arbeitsdatei `target` und
    /// die Bilder der Vorlage daneben.
    pub fn instantiate(&self, target: &Path, resources: &BTreeSet<String>) -> io::Result<()> {
        let target_dir = target.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(target_dir)?;
        for rel in resources {
            match &self.dir {
                Some(dir) => {
                    // Fehlende Bilder verhindern das Anlegen nicht.
                    if dir.join(rel).is_file() {
                        copy_resource(dir, target_dir, rel)?;
                    }
                }
                None => {
                    if let Some(bytes) = builtin::resource(rel) {
                        let dst = target_dir.join(rel);
                        if let Some(parent) = dst.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(dst, bytes)?;
                    }
                }
            }
        }
        fvm::save(target, &self.project, None).map_err(|e| invalid(e.to_string()))?;
        Ok(())
    }
}

fn read_module(path: &Path) -> io::Result<ModuleDef> {
    let file: ModuleFile =
        serde_json::from_slice(&fs::read(path)?).map_err(|e| invalid(e.to_string()))?;
    if file.format != MODULE_FORMAT || file.schema > MODULE_SCHEMA {
        return Err(invalid("Keine lesbare Moduldatei"));
    }
    validate_module(&file.module).map_err(invalid)?;
    Ok(file.module)
}

fn copy_resource(from_dir: &Path, to_dir: &Path, rel: &str) -> io::Result<()> {
    if !crate::latex::is_safe_resource_path(rel) {
        return Err(invalid(format!("Unzulässiger Bildpfad „{rel}“")));
    }
    let dst = to_dir.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from_dir.join(rel), dst)?;
    Ok(())
}

/// Ob ein Bild auch außerhalb der Eingabewerte gebraucht wird (Vorspann,
/// freies LaTeX, Standardwerte der Module).
fn used_outside_values(project: &Project, rel: &str) -> bool {
    let mut stripped = project.clone();
    for part in &mut stripped.parts {
        if let PartContent::Module(inst) = &mut part.content {
            inst.values.clear();
        }
    }
    crate::generate(&stripped).resources.contains(rel)
}

fn collect_refs(body: &[Element], out: &mut Vec<ModuleRef>) {
    for el in body {
        match &el.kind {
            ElementKind::Module { module } => out.push(module.clone()),
            ElementKind::Row { children } => collect_refs(children, out),
            _ => {}
        }
    }
}

/// Inhaltlicher Vergleich ohne Versionsnummer.
fn same_content(a: &ModuleDef, b: &ModuleDef) -> bool {
    let mut b = b.clone();
    b.version = a.version;
    *a == b
}

/// Prüft eine Moduldefinition, bevor sie gespeichert oder in ein Dokument
/// übernommen wird. Liefert eine Meldung für den Modulautor.
pub fn validate_module(def: &ModuleDef) -> Result<(), String> {
    if def.name.trim().is_empty() {
        return Err("Das Modul braucht einen Namen".into());
    }
    if def.id.as_str().is_empty() || def.id.as_str() != slug(def.id.as_str()) {
        return Err(format!("Ungültige Modulkennung „{}“", def.id));
    }
    let mut fields = BTreeSet::new();
    for f in &def.fields {
        if f.id.as_str().is_empty() || f.id.as_str().contains('/') {
            return Err(format!("Ungültige Feldkennung „{}“", f.id));
        }
        if !fields.insert(f.id.as_str()) {
            return Err(format!("Feld „{}“ ist doppelt vorhanden", f.id));
        }
        if let Some(default) = &f.default {
            if default.kind() != f.kind {
                return Err(format!("Standardwert von „{}“ hat die falsche Art", f.id));
            }
        }
    }
    let mut elements = BTreeSet::new();
    validate_body(def, &def.body, &mut elements)
}

fn validate_body<'a>(
    def: &'a ModuleDef,
    body: &'a [Element],
    seen: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    use crate::model::{FieldKind, Text};
    let field_of =
        |id: &crate::model::Id, kinds: &[FieldKind], what: &str| match def.field(id.as_str()) {
            None => Err(format!("{what} verweist auf das unbekannte Feld „{id}“")),
            Some(f) if !kinds.contains(&f.kind) => Err(format!(
                "{what} kann das Feld „{id}“ dieser Art nicht verwenden"
            )),
            Some(_) => Ok(()),
        };
    let text = |t: &Text, what: &str| match t {
        Text::Literal(_) => Ok(()),
        Text::Field(id) => field_of(id, &[FieldKind::Text, FieldKind::Number], what),
    };
    for el in body {
        if el.id.as_str().is_empty() || el.id.as_str().contains('/') {
            return Err(format!("Ungültige Elementkennung „{}“", el.id));
        }
        if !seen.insert(el.id.as_str()) {
            return Err(format!("Element „{}“ ist doppelt vorhanden", el.id));
        }
        match &el.kind {
            ElementKind::Heading { text: t, .. } => text(t, "Die Überschrift")?,
            ElementKind::Paragraph { text: t } => text(t, "Der Text")?,
            ElementKind::Table {
                columns,
                rows,
                caption,
            } => {
                field_of(rows, &[FieldKind::Table], "Die Tabelle")?;
                if columns.is_empty() {
                    return Err("Eine Tabelle braucht mindestens eine Spalte".into());
                }
                let mut ids = BTreeSet::new();
                for c in columns {
                    if !ids.insert(c.id.as_str()) {
                        return Err(format!("Spalte „{}“ ist doppelt vorhanden", c.id));
                    }
                    text(&c.header, "Der Spaltenkopf")?;
                }
                if let Some(c) = caption {
                    text(c, "Der Tabellentitel")?;
                }
            }
            ElementKind::Image { path, .. } => field_of(path, &[FieldKind::Image], "Das Bild")?,
            ElementKind::List { items, .. } => field_of(items, &[FieldKind::Table], "Die Liste")?,
            ElementKind::Row { children } => validate_body(def, children, seen)?,
            ElementKind::Module { module } => {
                if module.id == def.id {
                    return Err("Ein Modul kann sich nicht selbst enthalten".into());
                }
            }
            ElementKind::Latex { source } => {
                for id in crate::latex::field_refs(source) {
                    if def.field(id).is_none() {
                        return Err(format!("\\feld{{{id}}} verweist auf ein unbekanntes Feld"));
                    }
                }
            }
        }
    }
    // Der Suchpfad über Kennungen muss jedes Element finden.
    debug_assert!(body
        .iter()
        .all(|e| find_element(body, e.id.as_str()).is_some()));
    Ok(())
}

impl Library {
    /// Dateien des eigenen Ordners, die sich nicht verwenden lassen, je mit
    /// dem Grund. Die Bibliothek überspringt sie sonst stillschweigend.
    pub fn problems(&self) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        let Some(dir) = &self.user else {
            return out;
        };
        for entry in fs::read_dir(dir.join("module"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_some_and(|x| x == "json") {
                if let Err(e) = read_module(&path) {
                    out.push((path, e.to_string()));
                }
            }
        }
        for entry in fs::read_dir(dir.join("vorlagen"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path().join(TEMPLATE_FILE);
            if !entry.path().is_dir() {
                continue;
            }
            match fvm::load(&path) {
                Err(e) => out.push((path, e.to_string())),
                Ok((project, _)) => {
                    for def in &project.modules {
                        if let Err(e) = validate_module(def) {
                            out.push((path.clone(), format!("Modul {}: {e}", def.id)));
                        }
                    }
                    for notice in crate::generate(&project).notices {
                        out.push((path.clone(), notice.message));
                    }
                }
            }
        }
        out
    }
}
