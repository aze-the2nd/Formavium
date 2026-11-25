//! Rendersteuerung.
//!
//! * [`RenderRequest`] ist ein unveränderlicher Satz Eingabedateien einer Revision.
//! * [`Engine`] kompiliert ihn; [`TectonicEngine`] startet die Tectonic-CLI
//!   in einem eigenen Prozess mit festen Argumenten.
//! * [`Renderer`] lässt pro Dokument höchstens die jüngste angeforderte
//!   Revision warten.
//! * [`Preview`] entscheidet, welches PDF angezeigt und gespeichert wird.

mod diagnostics;
mod preview;
mod scheduler;
mod tectonic;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub use diagnostics::{parse_tectonic_output, Diagnostic, Severity};
pub use preview::{Preview, PreviewStatus, ShownPdf};
pub use scheduler::Renderer;
pub use tectonic::{locate_tectonic, TectonicEngine};

use formavium_core::Revision;

/// Unveränderliche Eingabe eines Renderauftrags.
#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub revision: Revision,
    /// Inhalt von `main.tex`.
    pub main_tex: String,
    /// Projektressourcen: relativer Zielpfad und Quelldatei.
    pub resources: Vec<(String, PathBuf)>,
}

impl RenderRequest {
    /// Baut einen Auftrag aus dem erzeugten Quelltext eines Projekts.
    pub fn from_generated(
        revision: Revision,
        generated: &formavium_core::Generated,
        project_dir: &std::path::Path,
    ) -> Self {
        RenderRequest {
            revision,
            main_tex: generated.text.clone(),
            resources: generated
                .resources
                .iter()
                .map(|r| (r.clone(), project_dir.join(r)))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedPdf {
    pub bytes: Arc<Vec<u8>>,
    pub sha256: String,
}

impl RenderedPdf {
    pub fn new(bytes: Vec<u8>) -> Self {
        use sha2::{Digest, Sha256};
        let sha256 = formavium_core::io::hex(&Sha256::digest(&bytes));
        RenderedPdf {
            bytes: Arc::new(bytes),
            sha256,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// TeX hat den Quelltext abgelehnt.
    Compile,
    Timeout,
    Cancelled,
    /// Engine nicht startbar oder Arbeitsverzeichnis nicht nutzbar.
    Environment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileFailure {
    pub kind: FailureKind,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct RenderOutcome {
    pub revision: Revision,
    pub result: Result<RenderedPdf, CompileFailure>,
    /// Warnungen eines erfolgreichen Laufs.
    pub warnings: Vec<Diagnostic>,
    pub duration: Duration,
    /// Gesammelte Ausgabe der Engine für die Fehlersuche.
    pub log: String,
}

pub trait Engine: Send + Sync + 'static {
    fn compile(&self, request: &RenderRequest) -> RenderOutcome;
}
