//! Laden und sicheres Speichern von `.fvm`-Dateien.
//!
//! Speichern schreibt zuerst eine temporäre Datei im selben Verzeichnis und
//! ersetzt die alte Datei erst nach erfolgreichem Schreiben. Eine extern
//! geänderte Datei wird vor dem Überschreiben erkannt.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::model::{Project, FORMAT_TAG, SCHEMA_VERSION};

#[derive(Debug, Error)]
pub enum IoError {
    #[error("Datei {path} konnte nicht gelesen oder geschrieben werden: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Datei {path} ist keine gültige Formavium-Datei: {message}")]
    Invalid { path: PathBuf, message: String },
    #[error(
        "Datei {path} verwendet Schema {found}, diese Version unterstützt höchstens {supported}"
    )]
    UnsupportedSchema {
        path: PathBuf,
        found: u64,
        supported: u32,
    },
    #[error("Datei {path} wurde seit dem Öffnen extern geändert")]
    Conflict { path: PathBuf },
}

/// Inhaltsprüfsumme einer gelesenen oder geschriebenen Datei.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStamp {
    pub sha256: String,
}

impl FileStamp {
    fn of(bytes: &[u8]) -> Self {
        FileStamp {
            sha256: hex(&Sha256::digest(bytes)),
        }
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> IoError + '_ {
    move |source| IoError::Io {
        path: path.to_owned(),
        source,
    }
}

pub fn parse(path: &Path, bytes: &[u8]) -> Result<Project, IoError> {
    let invalid = |message: String| IoError::Invalid {
        path: path.to_owned(),
        message,
    };
    let raw: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
    if raw.get("format").and_then(|v| v.as_str()) != Some(FORMAT_TAG) {
        return Err(invalid("Kennzeichen „format“ fehlt".into()));
    }
    let schema = raw
        .get("schema")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| invalid("Schemaversion fehlt".into()))?;
    // Neuere Schemas niemals still konvertieren.
    if schema > u64::from(SCHEMA_VERSION) {
        return Err(IoError::UnsupportedSchema {
            path: path.to_owned(),
            found: schema,
            supported: SCHEMA_VERSION,
        });
    }
    serde_json::from_value(raw).map_err(|e| invalid(e.to_string()))
}

pub fn load(path: &Path) -> Result<(Project, FileStamp), IoError> {
    let bytes = fs::read(path).map_err(io_err(path))?;
    let project = parse(path, &bytes)?;
    Ok((project, FileStamp::of(&bytes)))
}

pub fn to_bytes(project: &Project) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(project).expect("Projekt ist serialisierbar");
    bytes.push(b'\n');
    bytes
}

/// Speichert atomar. Mit `expected` wird geprüft, dass die Datei auf der
/// Platte noch dem zuletzt gelesenen oder geschriebenen Stand entspricht.
pub fn save(
    path: &Path,
    project: &Project,
    expected: Option<&FileStamp>,
) -> Result<FileStamp, IoError> {
    if let Some(expected) = expected {
        match fs::read(path) {
            Ok(current) if &FileStamp::of(&current) != expected => {
                return Err(IoError::Conflict {
                    path: path.to_owned(),
                })
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(IoError::Io {
                    path: path.to_owned(),
                    source: e,
                })
            }
        }
    }
    let bytes = to_bytes(project);
    write_atomic(path, &bytes)?;
    Ok(FileStamp::of(&bytes))
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), IoError> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().ok_or_else(|| IoError::Invalid {
        path: path.to_owned(),
        message: "kein Dateiname".into(),
    })?;
    let tmp = dir.join(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if let Err(source) = result {
        let _ = fs::remove_file(&tmp);
        return Err(IoError::Io {
            path: path.to_owned(),
            source,
        });
    }
    Ok(())
}
