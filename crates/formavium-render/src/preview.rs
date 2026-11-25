use std::path::Path;

use formavium_core::Revision;

use crate::{CompileFailure, RenderOutcome, RenderedPdf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownPdf {
    pub revision: Revision,
    pub pdf: RenderedPdf,
}

/// Was die Vorschau dem Nutzer zeigt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewStatus {
    /// Noch kein Ergebnis.
    Empty,
    /// Angezeigtes PDF entspricht der jüngsten abgeschlossenen Revision.
    Current { revision: Revision },
    /// Neuere Revision ist fehlgeschlagen; angezeigt wird ein älterer Stand.
    Stale { shown: Revision, failed: Revision },
    /// Noch nie erfolgreich; Ursache und Stelle anzeigen.
    Failed { failed: Revision },
}

/// Vorschauzustand eines Dokuments. Prüft die Revisionszuordnung verspätet
/// eintreffender Ergebnisse.
#[derive(Debug, Default)]
pub struct Preview {
    shown: Option<ShownPdf>,
    failure: Option<(Revision, CompileFailure)>,
}

impl Preview {
    /// Übernimmt ein Ergebnis. Liefert `false`, wenn es veraltet ist.
    pub fn accept(&mut self, outcome: RenderOutcome) -> bool {
        let newest = self.newest_known();
        if newest.is_some_and(|n| outcome.revision <= n) {
            return false;
        }
        match outcome.result {
            Ok(pdf) => {
                self.shown = Some(ShownPdf {
                    revision: outcome.revision,
                    pdf,
                });
                self.failure = None;
            }
            Err(f) => self.failure = Some((outcome.revision, f)),
        }
        true
    }

    fn newest_known(&self) -> Option<Revision> {
        let s = self.shown.as_ref().map(|s| s.revision);
        let f = self.failure.as_ref().map(|f| f.0);
        s.max(f)
    }

    pub fn status(&self) -> PreviewStatus {
        match (&self.shown, &self.failure) {
            (None, None) => PreviewStatus::Empty,
            (Some(s), None) => PreviewStatus::Current {
                revision: s.revision,
            },
            (Some(s), Some((f, _))) => PreviewStatus::Stale {
                shown: s.revision,
                failed: *f,
            },
            (None, Some((f, _))) => PreviewStatus::Failed { failed: *f },
        }
    }

    pub fn shown(&self) -> Option<&ShownPdf> {
        self.shown.as_ref()
    }

    pub fn failure(&self) -> Option<&CompileFailure> {
        self.failure.as_ref().map(|f| &f.1)
    }

    /// „Als PDF speichern“: schreibt exakt die angezeigten Bytes.
    pub fn save_pdf(&self, path: &Path) -> Result<&ShownPdf, formavium_core::io::IoError> {
        let shown = self
            .shown
            .as_ref()
            .ok_or_else(|| formavium_core::io::IoError::Invalid {
                path: path.to_owned(),
                message: "Es liegt noch kein erfolgreich erzeugtes PDF vor".into(),
            })?;
        formavium_core::io::write_atomic(path, &shown.pdf.bytes)?;
        Ok(shown)
    }
}
