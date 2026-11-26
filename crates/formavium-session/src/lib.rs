//! Sitzung eines geöffneten Dokuments.
//!
//! Die Oberfläche spricht ausschließlich über [`Session::call`] (JSON hinein,
//! JSON heraus) und [`Session::pdf`] mit dem Kern. Die Tauri-Hülle und die
//! Dev-Bridge für den Browser nutzen dieselbe Schnittstelle.

mod libview;
mod sync;
mod texcache;
mod views;

pub use libview::*;
pub use texcache::install_seed_cache;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use base64::Engine as _;
use formavium_core::io::{self, FileStamp};
use formavium_core::latex::{byte_to_utf16, is_safe_resource_path, utf16_to_byte};
use formavium_core::library::{slug, Library, ModuleEntry};
use formavium_core::source_edit::{plan_source_change, SourceChange, SourcePlan};
use formavium_core::{
    export, generate, Command, Element, ElementKind, FieldDef, FieldKind, Generated, Id, ModuleDef,
    ModuleRef, PartContent, Revision, Text, Value, Workspace,
};
use formavium_render::{
    locate_tectonic, Engine, Preview, RenderOutcome, RenderRequest, Renderer, TectonicEngine,
};
use serde::Deserialize;
use serde_json::{json, Value as Json};

pub use views::*;

/// Gemeinsam genutzte Engine für alle Dokumente einer Sitzung.
#[derive(Clone)]
pub struct SharedEngine(pub Arc<dyn Engine>);

impl Engine for SharedEngine {
    fn compile(&self, request: &RenderRequest) -> RenderOutcome {
        self.0.compile(request)
    }
}

struct OpenDoc {
    path: PathBuf,
    stamp: Option<FileStamp>,
    ws: Workspace,
    generated: Arc<Generated>,
    /// Erzeugte Fassungen der zuletzt angeforderten Revisionen, um
    /// Diagnosen einer älteren Revision ihrer Stelle zuzuordnen.
    history: VecDeque<(Revision, Arc<Generated>)>,
    renderer: Renderer,
    results: Receiver<RenderOutcome>,
    preview: Preview,
    requested: Revision,
    last_outcome: Option<OutcomeInfo>,
}

#[derive(Clone, Debug)]
struct OutcomeInfo {
    revision: Revision,
    duration_ms: u128,
    warnings: usize,
}

const HISTORY: usize = 16;

pub struct Session {
    engine: SharedEngine,
    doc: Option<OpenDoc>,
    library: Library,
    /// Module der Bibliothek, zuletzt gelesener Stand. Dient dem Vergleich
    /// mit den Modulen des Dokuments, ohne bei jeder Eingabe Dateien zu lesen.
    library_modules: Vec<ModuleEntry>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new(Arc::new(TectonicEngine {
            executable: locate_tectonic(),
            ..Default::default()
        }))
    }
}

type R<T> = Result<T, String>;

fn arg<T: for<'de> Deserialize<'de>>(args: &Json, key: &str) -> R<T> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Json::Null))
        .map_err(|e| format!("Parameter „{key}“: {e}"))
}

fn project_dir(path: &Path) -> PathBuf {
    path.parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Ein neues Dokument bekommt einen eigenen Ordner mit seinem Namen, weil
/// Bilder neben der Arbeitsdatei liegen: `…/x/anleitung.fvm` wird zu
/// `…/x/anleitung/anleitung.fvm`. Heißt der gewählte Ordner schon so,
/// bleibt der Pfad unverändert.
fn own_folder(path: &Path) -> PathBuf {
    let (Some(stem), Some(file)) = (path.file_stem(), path.file_name()) else {
        return path.to_owned();
    };
    let dir = project_dir(path);
    let same = dir.file_name().is_some_and(|d| {
        d.to_string_lossy().to_lowercase() == stem.to_string_lossy().to_lowercase()
    });
    if same {
        path.to_owned()
    } else {
        dir.join(stem).join(file)
    }
}

impl Session {
    pub fn new(engine: Arc<dyn Engine>) -> Self {
        let library = Library::new(None);
        Session {
            engine: SharedEngine(engine),
            doc: None,
            library_modules: library.modules(),
            library,
        }
    }

    /// Legt den Ordner der eigenen Bibliothek fest (Module und Vorlagen des
    /// Anwenders). Ohne Ordner gibt es nur die mitgelieferten Bausteine.
    pub fn set_library_dir(&mut self, dir: Option<PathBuf>) {
        self.library = Library::new(dir);
        self.library_modules = self.library.modules();
    }

    pub fn library_dir(&self) -> Option<&Path> {
        self.library.user_dir()
    }

    /// Einziger Einstieg für Befehle der Oberfläche.
    pub fn call(&mut self, name: &str, args: &Json) -> R<Json> {
        let value = match name {
            "new_sample" => {
                let path: PathBuf = arg(args, "path")?;
                self.new_document(&path, "montageanleitung")?;
                self.doc_json()?
            }
            "new_document" => {
                let path: PathBuf = arg(args, "path")?;
                let template: String = arg(args, "template")?;
                self.new_document(&path, &template)?;
                self.doc_json()?
            }
            // Dokument als freies LaTeX weiterführen: neue Datei, Original bleibt.
            "to_free_copy" => {
                let path: PathBuf = arg(args, "path")?;
                self.save_free_copy(&path)?;
                self.doc_json()?
            }
            "close" => {
                self.doc = None;
                Json::Null
            }
            // --- Bibliothek ---
            "library" => {
                self.library_modules = self.library.modules();
                serde_json::to_value(library_view(
                    self.library.user_dir(),
                    &self.library_modules,
                    &self.library.templates(),
                ))
                .map_err(|e| e.to_string())?
            }
            "insert_module" => {
                let base: Revision = arg(args, "base")?;
                let index: usize = arg(args, "index")?;
                let module: ModuleRef = arg(args, "module")?;
                let def = self
                    .library
                    .module(&module)
                    .ok_or_else(|| format!("Modul {module} fehlt in der Bibliothek"))?;
                let requires = self.requires(&def)?;
                self.apply(
                    base,
                    Command::InsertModule {
                        index,
                        def,
                        requires,
                    },
                )?;
                self.doc_json()?
            }
            // Neues, leeres Modul an der Stelle anlegen, zum Ausbauen in der Werkstatt.
            "new_module" => {
                let base: Revision = arg(args, "base")?;
                let index: usize = arg(args, "index")?;
                let name: String = arg(args, "name")?;
                let def = self.blank_module(&name)?;
                let module = def.reference();
                self.apply(
                    base,
                    Command::InsertModule {
                        index,
                        def,
                        requires: Vec::new(),
                    },
                )?;
                json!({ "doc": self.doc_json()?, "module": module })
            }
            // Freien LaTeX-Abschnitt in ein Modul verwandeln.
            "part_to_module" => {
                let base: Revision = arg(args, "base")?;
                let part: Id = arg(args, "part")?;
                let name: String = arg(args, "name")?;
                let source = match self.doc_ref()?.ws.project().part(&part).map(|p| &p.content) {
                    Some(PartContent::Latex { source }) => source.clone(),
                    _ => return Err("Nur freie LaTeX-Abschnitte lassen sich umwandeln".into()),
                };
                let mut def = self.blank_module(&name)?;
                def.fields.clear();
                def.body = vec![Element {
                    id: Id::from("inhalt"),
                    kind: ElementKind::Latex {
                        source: source.trim_end().to_owned(),
                    },
                }];
                let module = def.reference();
                self.apply(base, Command::ConvertPartToModule { part, def })?;
                json!({ "doc": self.doc_json()?, "module": module })
            }
            "module_def" => {
                let module: ModuleRef = arg(args, "module")?;
                let def = self
                    .doc_ref()?
                    .ws
                    .project()
                    .module(&module)
                    .ok_or_else(|| format!("Modul {module} fehlt im Dokument"))?;
                serde_json::to_value(def).map_err(|e| e.to_string())?
            }
            // Stand des Dokuments in die eigene Bibliothek übernehmen.
            "publish_module" => {
                let base: Revision = arg(args, "base")?;
                let module: ModuleRef = arg(args, "module")?;
                let project = self.doc_ref()?.ws.project();
                let def = project
                    .module(&module)
                    .cloned()
                    .ok_or_else(|| format!("Modul {module} fehlt im Dokument"))?;
                // Untermodule müssen ebenfalls in der Bibliothek stehen.
                for sub in self
                    .library
                    .closure(&def, &project.modules)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .skip(1)
                {
                    if self.library.module(&sub.reference()).is_none() {
                        self.library.save_module(&sub).map_err(|e| e.to_string())?;
                    }
                }
                let saved = self.library.save_module(&def).map_err(|e| e.to_string())?;
                self.library_modules = self.library.modules();
                if saved.version != module.version {
                    self.apply(
                        base,
                        Command::UseModuleVersion {
                            id: module.id,
                            from: module.version,
                            def: saved.clone(),
                            requires: Vec::new(),
                        },
                    )?;
                }
                json!({ "doc": self.doc_json()?, "module": saved.reference() })
            }
            // Alle Verwendungen im Dokument auf die neueste Bibliotheksversion heben.
            "upgrade_module" => {
                let base: Revision = arg(args, "base")?;
                let module: ModuleRef = arg(args, "module")?;
                let latest = self
                    .library
                    .latest_version(module.id.as_str())
                    .ok_or_else(|| format!("{} steht nicht in der Bibliothek", module.id))?;
                let def = self
                    .library
                    .module(&ModuleRef {
                        id: module.id.clone(),
                        version: latest,
                    })
                    .ok_or("Bibliotheksstand nicht lesbar")?;
                let requires = self.requires(&def)?;
                self.apply(
                    base,
                    Command::UseModuleVersion {
                        id: module.id,
                        from: module.version,
                        def,
                        requires,
                    },
                )?;
                self.doc_json()?
            }
            "save_template" => {
                let title: String = arg(args, "title")?;
                let description: String = arg(args, "description")?;
                let keep_values: bool = arg(args, "keep_values")?;
                let doc = self.doc_ref()?;
                let mut project = doc.ws.project().clone();
                project.title = title.trim().to_owned();
                project.description = description.trim().to_owned();
                if project.title.is_empty() {
                    return Err("Die Vorlage braucht einen Namen".into());
                }
                let id = self
                    .library
                    .save_template(
                        &project,
                        &project_dir(&doc.path),
                        &doc.generated.resources,
                        keep_values,
                    )
                    .map_err(|e| e.to_string())?;
                json!({ "id": id })
            }
            "delete_library_module" => {
                let module: ModuleRef = arg(args, "module")?;
                self.library
                    .delete_module(&module)
                    .map_err(|e| e.to_string())?;
                self.library_modules = self.library.modules();
                Json::Null
            }
            "delete_template" => {
                let id: String = arg(args, "id")?;
                self.library
                    .delete_template(&id)
                    .map_err(|e| e.to_string())?;
                Json::Null
            }
            "open" => {
                let path: PathBuf = arg(args, "path")?;
                self.open(&path)?;
                self.doc_json()?
            }
            "state" => match self.doc {
                Some(_) => self.doc_json()?,
                None => Json::Null,
            },
            "edit" => {
                let base: Revision = arg(args, "base")?;
                let command: Command = arg(args, "command")?;
                self.apply(base, command)?;
                self.doc_json()?
            }
            "source_edit" => self.source_edit(args)?,
            "undo" => {
                self.doc_mut()?.ws.undo().map_err(|e| e.to_string())?;
                self.refresh();
                self.doc_json()?
            }
            "redo" => {
                self.doc_mut()?.ws.redo().map_err(|e| e.to_string())?;
                self.refresh();
                self.doc_json()?
            }
            "save" => {
                self.save()?;
                self.doc_json()?
            }
            "preview" => serde_json::to_value(self.preview()?).map_err(|e| e.to_string())?,
            // --- Zuordnung PDF ↔ Dokument (SyncTeX des angezeigten PDFs) ---
            "sync" => match self.shown_sync() {
                Some((rev, g, map)) => {
                    let mut out = sync::overview(&g, &map);
                    out["revision"] = json!(rev);
                    out["current"] = json!(rev == self.doc_ref()?.ws.revision());
                    out
                }
                None => Json::Null,
            },
            "locate" => {
                let page: u32 = arg(args, "page")?;
                let x: f32 = arg(args, "x")?;
                let y: f32 = arg(args, "y")?;
                match self.shown_sync() {
                    Some((rev, g, map)) => match sync::locate(&g, &map, page, x, y) {
                        Some(mut hit) => {
                            hit["current"] = json!(rev == self.doc_ref()?.ws.revision());
                            hit
                        }
                        None => Json::Null,
                    },
                    None => Json::Null,
                }
            }
            "highlight" => {
                let targets: Vec<sync::Target> = arg(args, "targets")?;
                match self.shown_sync() {
                    Some((_, g, map)) => json!(sync::highlight(&g, &map, &targets)),
                    None => json!([]),
                }
            }
            "save_pdf" => {
                let path: PathBuf = arg(args, "path")?;
                let doc = self.doc_ref()?;
                let shown = doc.preview.save_pdf(&path).map_err(|e| e.to_string())?;
                json!({ "revision": shown.revision, "sha256": shown.pdf.sha256, "path": path })
            }
            "export" => {
                let dir: PathBuf = arg(args, "dir")?;
                let doc = self.doc_ref()?;
                let g = export::export_latex(doc.ws.project(), &project_dir(&doc.path), &dir)
                    .map_err(|e| e.to_string())?;
                json!({ "dir": dir, "resources": g.resources.len() })
            }
            "import_image" => {
                let source: PathBuf = arg(args, "source")?;
                let name = source
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let bytes = std::fs::read(&source).map_err(|e| e.to_string())?;
                json!({ "path": self.import_image(&name, &bytes)? })
            }
            // Bild aus der Oberfläche (Ablegen, Einfügen, Dateiauswahl).
            "import_image_data" => {
                let name: String = arg(args, "name")?;
                let data: String = arg(args, "data")?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data.as_bytes())
                    .map_err(|e| format!("Bilddaten unlesbar: {e}"))?;
                json!({ "path": self.import_image(&name, &bytes)? })
            }
            other => return Err(format!("Unbekannter Befehl „{other}“")),
        };
        Ok(value)
    }

    /// PDF-Bytes des angezeigten Stands.
    pub fn pdf(&mut self) -> Option<(Revision, Arc<Vec<u8>>)> {
        let doc = self.doc.as_mut()?;
        drain(doc);
        doc.preview
            .shown()
            .map(|s| (s.revision, Arc::clone(&s.pdf.bytes)))
    }

    fn doc_ref(&self) -> R<&OpenDoc> {
        self.doc
            .as_ref()
            .ok_or_else(|| "Kein Dokument geöffnet".to_owned())
    }

    fn doc_mut(&mut self) -> R<&mut OpenDoc> {
        self.doc
            .as_mut()
            .ok_or_else(|| "Kein Dokument geöffnet".to_owned())
    }

    /// Datei eines Dokuments neben der Arbeitsdatei (Bild für die Anzeige).
    /// Nur relative Pfade innerhalb des Projektordners.
    pub fn resource(&self, rel: &str) -> Option<(&'static str, Vec<u8>)> {
        let doc = self.doc.as_ref()?;
        if !is_safe_resource_path(rel) {
            return None;
        }
        let mime = match rel.rsplit('.').next()?.to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "pdf" => "application/pdf",
            _ => return None,
        };
        let bytes = std::fs::read(project_dir(&doc.path).join(rel)).ok()?;
        Some((mime, bytes))
    }

    /// Legt ein Dokument aus einer Vorlage an (eigener Ordner, Bilder der
    /// Vorlage daneben) und öffnet es.
    fn new_document(&mut self, path: &Path, template: &str) -> R<()> {
        let path = &own_folder(path);
        if path.exists() {
            return Err(format!("{} existiert bereits", path.display()));
        }
        let entry = self
            .library
            .template(template)
            .ok_or_else(|| format!("Vorlage „{template}“ fehlt in der Bibliothek"))?;
        let resources = generate(&entry.project).resources;
        entry
            .instantiate(path, &resources)
            .map_err(|e| e.to_string())?;
        self.open(path)
    }

    fn save_free_copy(&mut self, path: &Path) -> R<()> {
        let path = &own_folder(path);
        if path.exists() {
            return Err(format!("{} existiert bereits", path.display()));
        }
        let doc = self.doc_ref()?;
        let mut free = doc.ws.project().clone();
        free.preamble = doc.generated.text.clone();
        free.parts.clear();
        free.modules.clear();
        free.free = true;
        free.title = format!("{} (LaTeX)", free.title);
        let from = project_dir(&doc.path);
        let to = project_dir(path);
        for rel in &doc.generated.resources {
            let src = from.join(rel);
            if src.is_file() {
                let dst = to.join(rel);
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
            }
        }
        std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
        io::save(path, &free, None).map_err(|e| e.to_string())?;
        self.open(path)
    }

    /// Untermodule, die mit einem Bibliotheksmodul ins Dokument müssen.
    fn requires(&self, def: &ModuleDef) -> R<Vec<ModuleDef>> {
        let known = self
            .doc_ref()
            .map(|d| d.ws.project().modules.clone())
            .unwrap_or_default();
        Ok(self
            .library
            .closure(def, &known)
            .map_err(|e| e.to_string())?
            .into_iter()
            .skip(1)
            .collect())
    }

    /// Neues Modul mit einer Überschrift und einem Text. Die Kennung ist im
    /// Dokument und in der Bibliothek noch frei.
    fn blank_module(&self, name: &str) -> R<ModuleDef> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Das Modul braucht einen Namen".into());
        }
        let project = self.doc_ref()?.ws.project();
        let taken = |id: &str| {
            project.modules.iter().any(|m| m.id.as_str() == id)
                || self.library_modules.iter().any(|e| e.def.id.as_str() == id)
        };
        let stem = slug(name);
        let id = (1..)
            .map(|n| match n {
                1 => stem.clone(),
                n => format!("{stem}-{n}"),
            })
            .find(|id| !taken(id))
            .expect("freie Kennung");
        let field = |id: &str, label: &str, default: &str| FieldDef {
            id: Id::from(id),
            label: label.into(),
            kind: FieldKind::Text,
            default: Some(Value::Text(default.into())),
        };
        Ok(ModuleDef {
            id: Id(id),
            version: 1,
            name: name.to_owned(),
            description: String::new(),
            preamble: String::new(),
            fields: vec![field("titel", "Titel", name), field("text", "Text", "Text")],
            body: vec![
                Element {
                    id: Id::from("kopf"),
                    kind: ElementKind::Heading {
                        level: 1,
                        text: Text::Field(Id::from("titel")),
                    },
                },
                Element {
                    id: Id::from("absatz"),
                    kind: ElementKind::Paragraph {
                        text: Text::Field(Id::from("text")),
                    },
                },
            ],
        })
    }

    pub fn open(&mut self, path: &Path) -> R<()> {
        let (project, stamp) = io::load(path).map_err(|e| e.to_string())?;
        let (renderer, results) = Renderer::start(self.engine.clone());
        let generated = Arc::new(generate(&project));
        let mut doc = OpenDoc {
            path: path.to_owned(),
            stamp: Some(stamp),
            ws: Workspace::opened(project),
            generated,
            history: VecDeque::new(),
            renderer,
            results,
            preview: Preview::default(),
            requested: 0,
            last_outcome: None,
        };
        request_render(&mut doc);
        self.doc = Some(doc);
        Ok(())
    }

    fn apply(&mut self, base: Revision, command: Command) -> R<()> {
        self.doc_mut()?
            .ws
            .apply(base, command)
            .map_err(|e| e.to_string())?;
        self.refresh();
        Ok(())
    }

    fn refresh(&mut self) {
        if let Some(doc) = self.doc.as_mut() {
            doc.generated = Arc::new(generate(doc.ws.project()));
            request_render(doc);
        }
    }

    fn source_edit(&mut self, args: &Json) -> R<Json> {
        let base: Revision = arg(args, "base")?;
        let from16: usize = arg(args, "from")?;
        let to16: usize = arg(args, "to")?;
        let insert: String = arg(args, "insert")?;
        let allow_convert: bool = arg::<Option<bool>>(args, "allow_convert")?.unwrap_or(false);
        let doc = self.doc_ref()?;
        if base != doc.ws.revision() {
            return Ok(json!({
                "outcome": "stale",
                "message": "Der Quelltext beruht auf einem älteren Stand",
                "doc": self.doc_json()?,
            }));
        }
        let text = &doc.generated.text;
        let (Some(from), Some(to)) = (utf16_to_byte(text, from16), utf16_to_byte(text, to16))
        else {
            return Err("Position außerhalb des Quelltexts".into());
        };
        let change = SourceChange { from, to, insert };
        match plan_source_change(doc.ws.project(), &doc.generated, &change) {
            SourcePlan::Apply(cmd) => {
                self.apply(base, cmd)?;
                Ok(json!({ "outcome": "applied", "doc": self.doc_json()? }))
            }
            SourcePlan::Convert { modules, command } if allow_convert => {
                let _ = modules;
                self.apply(base, command)?;
                Ok(json!({ "outcome": "applied", "converted": true, "doc": self.doc_json()? }))
            }
            SourcePlan::Convert { modules, .. } => {
                let project = doc.ws.project();
                let names: Vec<String> = modules
                    .iter()
                    .filter_map(|id| project.instance(id))
                    .map(|i| {
                        project
                            .module(&i.module)
                            .map_or(i.module.to_string(), |d| d.name.clone())
                    })
                    .collect();
                Ok(json!({
                    "outcome": "needs_conversion",
                    "modules": modules,
                    "message": format!(
                        "Diese Änderung betrifft die Struktur von {}. Der Abschnitt wird zu freiem LaTeX und verliert seine visuelle Bearbeitung. Rückgängig ist möglich.",
                        names.join(", ")
                    ),
                }))
            }
            SourcePlan::Reject(message) => Ok(json!({
                "outcome": "rejected",
                "message": message,
                "doc": self.doc_json()?,
            })),
        }
    }

    fn save(&mut self) -> R<()> {
        let doc = self.doc_mut()?;
        let stamp =
            io::save(&doc.path, doc.ws.project(), doc.stamp.as_ref()).map_err(|e| e.to_string())?;
        doc.stamp = Some(stamp);
        let rev = doc.ws.revision();
        doc.ws.mark_saved(rev);
        Ok(())
    }

    /// Legt ein Bild unter eindeutigem Namen im Ordner `bilder` neben der
    /// Arbeitsdatei ab und liefert den relativen Pfad.
    fn import_image(&mut self, name: &str, bytes: &[u8]) -> R<String> {
        let doc = self.doc_ref()?;
        let name = Path::new(name);
        let stem = name
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "bild".into());
        let ext = name
            .extension()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let magic: &[&[u8]] = match ext.as_str() {
            "png" => &[b"\x89PNG\r\n\x1a\n"],
            "jpg" | "jpeg" => &[b"\xff\xd8\xff"],
            "pdf" => &[b"%PDF-"],
            _ => return Err("Unterstützt werden PNG, JPEG und PDF".into()),
        };
        if !magic.iter().any(|m| bytes.starts_with(m)) {
            return Err(format!(
                "Die Datei ist kein gültiges {}-Bild",
                ext.to_uppercase()
            ));
        }
        let clean: String = stem
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let dir = project_dir(&doc.path).join("bilder");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut name = format!("{clean}.{ext}");
        let mut n = 2;
        while dir.join(&name).exists() {
            name = format!("{clean}-{n}.{ext}");
            n += 1;
        }
        std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
        Ok(format!("bilder/{name}"))
    }

    fn preview(&mut self) -> R<PreviewView> {
        let doc = self.doc_mut()?;
        drain(doc);
        Ok(views::preview_view(
            &doc.preview,
            doc.ws.revision(),
            doc.requested,
            &doc.history,
            doc.last_outcome
                .as_ref()
                .map(|o| (o.revision, o.duration_ms, o.warnings)),
        ))
    }

    /// Revision, erzeugter Quelltext und SyncTeX des angezeigten PDFs.
    fn shown_sync(&mut self) -> Option<(Revision, Arc<Generated>, Arc<formavium_render::SyncMap>)> {
        let doc = self.doc.as_mut()?;
        drain(doc);
        let shown = doc.preview.shown()?;
        let map = shown.pdf.sync.clone()?;
        let rev = shown.revision;
        let g = doc
            .history
            .iter()
            .find(|(r, _)| *r == rev)
            .map(|(_, g)| Arc::clone(g))?;
        Some((rev, g, map))
    }

    fn doc_json(&self) -> R<Json> {
        let doc = self.doc_ref()?;
        // Verhältnis eines Dokumentmoduls zur Bibliothek.
        let status = |def: &ModuleDef| {
            let versions = self
                .library_modules
                .iter()
                .map(|e| &e.def)
                .filter(|d| d.id == def.id);
            ModuleStatus {
                local: !versions.clone().any(|d| d == def),
                newer: versions
                    .map(|d| d.version)
                    .max()
                    .filter(|latest| *latest > def.version),
            }
        };
        serde_json::to_value(views::doc_view(&doc.path, &doc.ws, &doc.generated, &status))
            .map_err(|e| e.to_string())
    }
}

fn request_render(doc: &mut OpenDoc) {
    let rev = doc.ws.revision();
    if rev == doc.requested {
        return;
    }
    doc.requested = rev;
    doc.history.push_back((rev, Arc::clone(&doc.generated)));
    while doc.history.len() > HISTORY {
        doc.history.pop_front();
    }
    let dir = project_dir(&doc.path);
    doc.renderer
        .request(RenderRequest::from_generated(rev, &doc.generated, &dir));
}

fn drain(doc: &mut OpenDoc) {
    while let Ok(outcome) = doc.results.try_recv() {
        let info = OutcomeInfo {
            revision: outcome.revision,
            duration_ms: outcome.duration.as_millis(),
            warnings: outcome.warnings.len(),
        };
        if doc.preview.accept(outcome) {
            doc.last_outcome = Some(info);
        }
    }
}

pub use formavium_core::sample::SAMPLE_PNG;

/// UTF-16-Position einer Byteposition (für Sprünge im Editor).
pub fn utf16(text: &str, byte: usize) -> usize {
    byte_to_utf16(text, byte)
}
