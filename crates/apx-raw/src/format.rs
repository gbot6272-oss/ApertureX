//! Dateiformat-Erkennung: entscheidet, ob ein Pfad über `rawler` (RAW) oder
//! über den `image`-Fallback (JPEG/PNG/TIFF) verarbeitet wird.

use std::path::Path;

/// RAW-Endungen, die Phase 1 laut `PHASE1_PROMPT.md` Abschnitt 3 unterstützt.
/// DNG ist absichtlich enthalten — `rawler` deckt DNG nativ ab.
const RAW_EXTENSIONS: &[&str] = &["cr2", "cr3", "nef", "arw", "raf", "orf", "rw2", "dng"];

const FALLBACK_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tif", "tiff"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Raw,
    Fallback,
}

/// Ordnet einen Pfad anhand seiner Dateiendung `FileKind::Raw` oder
/// `FileKind::Fallback` zu. Unbekannte Endungen werden als `Raw` behandelt
/// und damit an `rawler` weitergereicht — die Bibliothek erkennt intern per
/// Magic Bytes und liefert bei echt unbekannten Formaten ohnehin einen
/// aussagekräftigen Fehler zurück.
pub fn classify(path: &Path) -> FileKind {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase);

    match extension.as_deref() {
        Some(ext) if FALLBACK_EXTENSIONS.contains(&ext) => FileKind::Fallback,
        _ => FileKind::Raw,
    }
}

/// Ob eine Endung zu den in Phase 1 unterstützten Formaten gehört (RAW oder
/// Fallback). Wird vom Import verwendet, um Dateien beim Ordner-Scan zu
/// filtern.
pub fn is_supported_extension(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    let extension = extension.to_lowercase();
    RAW_EXTENSIONS.contains(&extension.as_str())
        || FALLBACK_EXTENSIONS.contains(&extension.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn raw_extensions_classify_as_raw() {
        for ext in RAW_EXTENSIONS {
            let path = PathBuf::from(format!("foto.{ext}"));
            assert_eq!(classify(&path), FileKind::Raw, "Endung {ext}");
        }
    }

    #[test]
    fn fallback_extensions_classify_as_fallback() {
        for ext in FALLBACK_EXTENSIONS {
            let path = PathBuf::from(format!("foto.{ext}"));
            assert_eq!(classify(&path), FileKind::Fallback, "Endung {ext}");
        }
    }

    #[test]
    fn classification_is_case_insensitive() {
        assert_eq!(classify(&PathBuf::from("foto.JPG")), FileKind::Fallback);
        assert_eq!(classify(&PathBuf::from("foto.CR2")), FileKind::Raw);
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        assert!(!is_supported_extension(&PathBuf::from("notiz.txt")));
        assert!(is_supported_extension(&PathBuf::from("foto.dng")));
    }
}
