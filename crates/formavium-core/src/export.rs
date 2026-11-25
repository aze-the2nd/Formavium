//! Eigenständig kompilierbarer LaTeX-Export.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::latex::{generate, is_safe_resource_path, Generated};
use crate::model::Project;

/// Engine-Profil, mit dem Formavium rendert und den Export prüft.
pub const ENGINE_PROFILE: &str = "Tectonic 0.17.0 (XeTeX-Engine, Standard-Bundle)";

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("Ressource „{0}“ hat einen unzulässigen Pfad")]
    UnsafePath(String),
    #[error("Ressource „{path}“ fehlt neben der Arbeitsdatei ({source})")]
    MissingResource {
        path: String,
        source: std::io::Error,
    },
    #[error("Schreiben nach {path} fehlgeschlagen: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Schreibt `main.tex`, alle referenzierten Ressourcen mit ihren relativen
/// Pfaden und einen Hinweis zur TeX-Umgebung in `out_dir`.
pub fn export_latex(
    project: &Project,
    project_dir: &Path,
    out_dir: &Path,
) -> Result<Generated, ExportError> {
    let generated = generate(project);
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| ExportError::Io { path, source }
    };
    fs::create_dir_all(out_dir).map_err(io(out_dir))?;
    copy_resources(&generated, project_dir, out_dir)?;
    let main = out_dir.join("main.tex");
    fs::write(&main, &generated.text).map_err(io(&main))?;
    let readme = out_dir.join("LIESMICH.txt");
    let text = format!(
        "Erzeugt mit Formavium aus „{}“.\n\nEinstiegspunkt: main.tex\nGeprüftes Profil: {ENGINE_PROFILE}\n\
         Kompilieren: tectonic -X compile main.tex\nAlternativ mit einer XeLaTeX-Installation: xelatex main.tex\n",
        project.title
    );
    fs::write(&readme, text).map_err(io(&readme))?;
    Ok(generated)
}

/// Kopiert die Ressourcen des erzeugten Quelltexts in ein Zielverzeichnis.
pub fn copy_resources(
    generated: &Generated,
    project_dir: &Path,
    out_dir: &Path,
) -> Result<(), ExportError> {
    for rel in &generated.resources {
        if !is_safe_resource_path(rel) {
            return Err(ExportError::UnsafePath(rel.clone()));
        }
        let src = project_dir.join(rel);
        let dst = out_dir.join(rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|source| ExportError::Io {
                path: parent.to_owned(),
                source,
            })?;
        }
        fs::copy(&src, &dst).map_err(|source| ExportError::MissingResource {
            path: rel.clone(),
            source,
        })?;
    }
    Ok(())
}
