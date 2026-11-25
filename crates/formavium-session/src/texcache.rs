//! Mitgeliefertes TeX-Ressourcenpaket (Offline-Erststart).
//!
//! Das Installationspaket enthält einen vorbefüllten Tectonic-Cache mit
//! `SEED_ID`. Beim Start wird er in den beschreibbaren Benutzer-Cache
//! kopiert, wenn dort noch kein oder ein anderes Paket liegt. Zusätzlich
//! geladene Dateien im Benutzer-Cache bleiben erhalten.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SEED_ID: &str = "SEED_ID";
const MARKER: &str = ".formavium-seed";

/// Richtet den Benutzer-Cache ein. `Ok(None)`, wenn kein Paket mitgeliefert
/// wurde (Entwicklung): Tectonic nutzt dann seinen Standard-Cache.
pub fn install_seed_cache(seed: &Path, target: &Path) -> io::Result<Option<PathBuf>> {
    let id = match fs::read_to_string(seed.join(SEED_ID)) {
        Ok(id) => id.trim().to_owned(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let marker = target.join(MARKER);
    if fs::read_to_string(&marker).is_ok_and(|m| m.trim() == id) {
        return Ok(Some(target.to_owned()));
    }
    copy_tree(seed, target)?;
    // Markierung zuletzt schreiben: Ein abgebrochener Kopiervorgang wird
    // beim nächsten Start wiederholt.
    fs::write(&marker, &id)?;
    Ok(Some(target.to_owned()))
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &dst)?;
        } else if entry.file_name() != SEED_ID {
            fs::copy(entry.path(), &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_once_and_again_after_update() {
        let seed = tempfile::tempdir().unwrap();
        let user = tempfile::tempdir().unwrap();
        let target = user.path().join("cache");
        assert_eq!(install_seed_cache(seed.path(), &target).unwrap(), None);

        fs::create_dir_all(seed.path().join("formats")).unwrap();
        fs::write(seed.path().join("formats/a.fmt"), b"1").unwrap();
        fs::write(seed.path().join(SEED_ID), "v1").unwrap();
        assert_eq!(
            install_seed_cache(seed.path(), &target).unwrap(),
            Some(target.clone())
        );
        assert_eq!(fs::read(target.join("formats/a.fmt")).unwrap(), b"1");

        // Später geladene Datei im Benutzer-Cache.
        fs::write(target.join("extra"), b"x").unwrap();
        // Gleiche Kennung: nichts kopieren.
        fs::write(seed.path().join("formats/a.fmt"), b"2").unwrap();
        install_seed_cache(seed.path(), &target).unwrap();
        assert_eq!(fs::read(target.join("formats/a.fmt")).unwrap(), b"1");
        // Neue Kennung: aktualisieren, Zusatzdateien behalten.
        fs::write(seed.path().join(SEED_ID), "v2").unwrap();
        install_seed_cache(seed.path(), &target).unwrap();
        assert_eq!(fs::read(target.join("formats/a.fmt")).unwrap(), b"2");
        assert!(target.join("extra").exists());
    }
}
