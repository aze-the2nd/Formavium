//! `formavium` – Kommandozeile für `.fvm`-Dateien.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use formavium_core::catalog::{content_kinds, ContentKind};
use formavium_core::latex::Origin;
use formavium_core::{
    export, generate, io, sample, Command, PartContent, Project, Value, Workspace,
};
use formavium_render::{locate_tectonic, Engine, RenderRequest, Severity, TectonicEngine};

const USAGE: &str = "\
Verwendung:
  formavium new <datei.fvm>                       Beispiel „Montageanleitung“ anlegen
  formavium info <datei.fvm>                      Teile, Module und Felder anzeigen
  formavium latex <datei.fvm>                     Erzeugtes main.tex ausgeben
  formavium render <datei.fvm> [-o <pdf>] [--offline]
  formavium export <datei.fvm> <zielordner>       Eigenständiges LaTeX-Projekt schreiben
  formavium set <datei.fvm> <teil> <feldpfad> <text>
  formavium set-latex <datei.fvm> <teil> <quelltext>
  formavium move-column <datei.fvm> <teil> <tabelle> <von> <nach>

Umgebung: FORMAVIUM_TECTONIC setzt den Pfad zur Tectonic-Programmdatei.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Fehler: {e}");
            ExitCode::from(2)
        }
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run(args: &[String]) -> Result<ExitCode> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["new", file] => cmd_new(Path::new(file)),
        ["info", file] => cmd_info(Path::new(file)),
        ["latex", file] => {
            let (p, _) = io::load(Path::new(file))?;
            print!("{}", generate(&p).text);
            Ok(ExitCode::SUCCESS)
        }
        ["render", file, rest @ ..] => cmd_render(Path::new(file), rest),
        ["export", file, out] => {
            let path = Path::new(file);
            let (p, _) = io::load(path)?;
            let g = export::export_latex(&p, project_dir(path), Path::new(out))?;
            println!("Exportiert nach {out} ({} Ressourcen)", g.resources.len());
            Ok(ExitCode::SUCCESS)
        }
        ["set", file, part, field, text] => edit(Path::new(file), |p| {
            let value = match p
                .instance(&(*part).into())
                .and_then(|i| p.scope_module(&i.module, field))
            {
                Some((def, f)) => match def.field(f).map(|f| f.kind) {
                    Some(formavium_core::FieldKind::Number) => Value::Number((*text).into()),
                    Some(formavium_core::FieldKind::Image) => Value::Image((*text).into()),
                    _ => Value::Text((*text).into()),
                },
                None => Value::Text((*text).into()),
            };
            Command::SetValue {
                part: (*part).into(),
                path: (*field).into(),
                value,
            }
        }),
        ["set-latex", file, part, source] => edit(Path::new(file), |_| Command::SetLatex {
            part: (*part).into(),
            source: (*source).into(),
        }),
        ["move-column", file, part, table, from, to] => {
            let (from, to) = (from.parse()?, to.parse()?);
            edit(Path::new(file), |_| Command::MoveColumn {
                part: (*part).into(),
                table: (*table).into(),
                from,
                to,
            })
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(64))
        }
    }
}

fn project_dir(file: &Path) -> &Path {
    file.parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn cmd_new(file: &Path) -> Result<ExitCode> {
    if file.exists() {
        return Err(format!("{} existiert bereits", file.display()).into());
    }
    let image = project_dir(file).join(sample::SAMPLE_IMAGE);
    std::fs::create_dir_all(image.parent().expect("Bildordner"))?;
    std::fs::write(&image, include_bytes!("../assets/beispiel.png"))?;
    io::save(file, &sample::anleitung(), None)?;
    println!("Angelegt: {} und {}", file.display(), image.display());
    Ok(ExitCode::SUCCESS)
}

fn cmd_info(file: &Path) -> Result<ExitCode> {
    let (p, _) = io::load(file)?;
    println!("{} (Schema {})", p.title, p.schema);
    for part in &p.parts {
        match &part.content {
            PartContent::Latex { source } => {
                let first = source.lines().next().unwrap_or("");
                println!("  {:<4} LaTeX   {first}", part.id);
            }
            PartContent::Module(inst) => {
                let name = p.module(&inst.module).map_or("?", |d| d.name.as_str());
                let req = if inst.required { " (erforderlich)" } else { "" };
                println!("  {:<4} Modul   {name} {}{req}", part.id, inst.module);
                if let Some(def) = p.module(&inst.module) {
                    for f in &def.fields {
                        let v = match p.effective_value(inst, f.id.as_str()) {
                            Some(Value::Text(s) | Value::Number(s) | Value::Image(s)) => s.clone(),
                            Some(Value::Table(rows)) => format!("{} Zeilen", rows.len()),
                            None => "—".into(),
                        };
                        println!("         {:<10} {:<14} {v}", f.id, f.label);
                    }
                }
            }
        }
    }
    for def in &p.modules {
        let kinds = content_kinds(def, &|r| p.module(r))?;
        let names: Vec<&str> = kinds
            .iter()
            .map(|k| match k {
                ContentKind::Text => "Text",
                ContentKind::Table => "Tabelle",
                ContentKind::Image => "Bild",
                ContentKind::Latex => "LaTeX",
            })
            .collect();
        println!(
            "Moduldefinition {} „{}“: {}",
            def.reference(),
            def.name,
            names.join(", ")
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn edit(file: &Path, make: impl FnOnce(&Project) -> Command) -> Result<ExitCode> {
    let (project, stamp) = io::load(file)?;
    let cmd = make(&project);
    let mut ws = Workspace::opened(project);
    let (rev, _) = ws.apply(ws.revision(), cmd)?;
    io::save(file, ws.project(), Some(&stamp))?;
    ws.mark_saved(rev);
    println!("Gespeichert ({})", file.display());
    Ok(ExitCode::SUCCESS)
}

fn describe(origin: &Origin) -> String {
    origin.to_string()
}

fn cmd_render(file: &Path, rest: &[&str]) -> Result<ExitCode> {
    let mut out: Option<PathBuf> = None;
    let mut offline = false;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "-o" => out = Some(PathBuf::from(it.next().ok_or("-o erwartet einen Pfad")?)),
            "--offline" => offline = true,
            other => return Err(format!("Unbekannte Option {other}").into()),
        }
    }
    let out = out.unwrap_or_else(|| file.with_extension("pdf"));
    let (p, _) = io::load(file)?;
    let generated = generate(&p);
    for n in &generated.notices {
        eprintln!("Hinweis: {} ({})", n.message, describe(&n.origin));
    }
    let engine = TectonicEngine {
        executable: locate_tectonic(),
        only_cached: offline,
        ..Default::default()
    };
    let outcome = engine.compile(&RenderRequest::from_generated(
        1,
        &generated,
        project_dir(file),
    ));
    let mut preview = formavium_render::Preview::default();
    let warnings = outcome.warnings.clone();
    let result = outcome.result.clone();
    preview.accept(outcome);
    match result {
        Ok(_) => {
            let shown = preview.save_pdf(&out)?;
            for w in &warnings {
                eprintln!("Warnung: {}", w.message);
            }
            println!(
                "PDF geschrieben: {} (sha256 {})",
                out.display(),
                shown.pdf.sha256
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(f) => {
            eprintln!("Kompilierung fehlgeschlagen ({:?})", f.kind);
            for d in f
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
            {
                let place = match (d.file.as_deref(), d.line) {
                    (Some("main.tex"), Some(line)) => generated
                        .origin_at_line(line)
                        .map_or(format!("Zeile {line}"), |o| {
                            format!("Zeile {line}, {}", describe(o))
                        }),
                    (Some(f), Some(line)) => format!("{f}:{line}"),
                    _ => "ohne Ortsangabe".into(),
                };
                eprintln!("  {place}: {}", d.message);
            }
            Ok(ExitCode::from(1))
        }
    }
}
