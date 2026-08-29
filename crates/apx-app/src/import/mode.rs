//! Import-Modi: Foto an Ort und Stelle katalogisieren (Standard seit
//! Phase 1) oder vorher in einen Zielordner kopieren/verschieben (additiv,
//! siehe `DECISIONS.md` ADR-0025). DNG-Konvertierung ist bewusst kein Teil
//! davon — siehe `FEATURES.md` §3.1, verschoben auf die Export-Phase.

use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use super::rename::{render_rename_pattern, RenameTokens};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportMode {
    /// Bisheriges Verhalten (Phase 1/2): Datei bleibt, wo sie ist.
    AddInPlace,
    /// Kopiert die Originaldatei nach `PathBuf`, bevor der übliche
    /// Scan-/Metadaten-/Thumbnail-Ablauf auf der Kopie weiterläuft.
    Copy(PathBuf),
    /// Wie `Copy`, verschiebt aber statt zu kopieren (Originaldatei
    /// existiert danach nicht mehr am alten Ort).
    Move(PathBuf),
}

/// Bringt `source` an den vom Import-Modus verlangten Ort und gibt den
/// Pfad zurück, unter dem die Datei ab jetzt zu behandeln ist (für
/// [`ImportMode::AddInPlace`] unverändert `source` selbst). `rename_pattern`
/// gilt nur für `Copy`/`Move` — bei `AddInPlace` bleibt der Dateiname immer
/// unverändert (siehe `PLAN.md` Phase 3, Schritt 4).
pub(crate) fn stage_file_for_mode(
    mode: &ImportMode,
    source: &Path,
    rename_pattern: Option<&str>,
    seq: usize,
    camera: Option<&str>,
    captured_at: Option<OffsetDateTime>,
    fallback_date: OffsetDateTime,
) -> Result<PathBuf, String> {
    let target_dir = match mode {
        ImportMode::AddInPlace => return Ok(source.to_path_buf()),
        ImportMode::Copy(dir) | ImportMode::Move(dir) => dir,
    };

    std::fs::create_dir_all(target_dir).map_err(|err| {
        format!(
            "Zielordner '{}' nicht anlegbar: {err}",
            target_dir.display()
        )
    })?;

    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    let original_stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("datei");

    let target_filename = match rename_pattern {
        Some(pattern) => {
            let tokens = RenameTokens {
                date: captured_at.unwrap_or(fallback_date),
                seq,
                camera,
                original_stem,
            };
            let base = render_rename_pattern(pattern, &tokens);
            if extension.is_empty() {
                base
            } else {
                format!("{base}.{extension}")
            }
        }
        None => source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "Dateiname ist kein gültiges UTF-8".to_string())?
            .to_string(),
    };

    let target_path = target_dir.join(target_filename);
    match mode {
        ImportMode::Copy(_) => {
            std::fs::copy(source, &target_path).map_err(|err| {
                format!(
                    "Kopieren von '{}' nach '{}' fehlgeschlagen: {err}",
                    source.display(),
                    target_path.display()
                )
            })?;
        }
        ImportMode::Move(_) => {
            std::fs::rename(source, &target_path).map_err(|err| {
                format!(
                    "Verschieben von '{}' nach '{}' fehlgeschlagen: {err}",
                    source.display(),
                    target_path.display()
                )
            })?;
        }
        ImportMode::AddInPlace => unreachable!("früher zurückgegeben"),
    }
    Ok(target_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    #[test]
    fn add_in_place_returns_source_unchanged() {
        let source = Path::new("/fotos/a.cr2");
        let result =
            stage_file_for_mode(&ImportMode::AddInPlace, source, None, 1, None, None, now())
                .expect("ok");
        assert_eq!(result, source);
    }

    #[test]
    fn copy_duplicates_file_and_keeps_original() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let source = tmp.path().join("original.cr2");
        std::fs::write(&source, b"raw-bytes").expect("Datei schreibbar");
        let target_dir = tmp.path().join("ziel");

        let result = stage_file_for_mode(
            &ImportMode::Copy(target_dir.clone()),
            &source,
            None,
            1,
            None,
            None,
            now(),
        )
        .expect("ok");

        assert!(source.exists(), "Original muss bei Copy erhalten bleiben");
        assert!(result.exists());
        assert_eq!(result, target_dir.join("original.cr2"));
        assert_eq!(std::fs::read(&result).expect("lesbar"), b"raw-bytes");
    }

    #[test]
    fn move_relocates_file_and_removes_original() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let source = tmp.path().join("original.cr2");
        std::fs::write(&source, b"raw-bytes").expect("Datei schreibbar");
        let target_dir = tmp.path().join("ziel");

        let result = stage_file_for_mode(
            &ImportMode::Move(target_dir.clone()),
            &source,
            None,
            1,
            None,
            None,
            now(),
        )
        .expect("ok");

        assert!(
            !source.exists(),
            "Original darf bei Move nicht mehr existieren"
        );
        assert_eq!(result, target_dir.join("original.cr2"));
    }

    #[test]
    fn copy_with_rename_pattern_uses_tokens_and_keeps_extension() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let source = tmp.path().join("IMG_0001.CR2");
        std::fs::write(&source, b"x").expect("Datei schreibbar");
        let target_dir = tmp.path().join("ziel");

        let result = stage_file_for_mode(
            &ImportMode::Copy(target_dir.clone()),
            &source,
            Some("{seq}_{original}"),
            5,
            Some("EOS R5"),
            None,
            now(),
        )
        .expect("ok");

        assert_eq!(result, target_dir.join("0005_IMG_0001.CR2"));
    }
}
