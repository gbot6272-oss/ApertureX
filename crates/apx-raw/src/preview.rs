//! Extraktion eingebetteter Vorschaubilder aus RAW-Dateien — der schnelle
//! Weg für den Import (siehe `PHASE1_PROMPT.md` Abschnitt 3 und 5): fast
//! jede RAW-Datei enthält ein oder mehrere fertig gerenderte JPEGs, deren
//! Extraktion um Größenordnungen billiger ist als eine eigene Dekodierung.

use std::path::Path;

use apx_core::{AppError, Result};
use image::DynamicImage;
use rawler::decoders::RawDecodeParams;
use rawler::rawsource::RawSource;

use crate::format::{classify, FileKind};

/// Extrahiert die eingebettete Vorschau, falls vorhanden. Für
/// Fallback-Formate (JPEG/PNG/TIFF) gibt es keine separate eingebettete
/// Vorschau — dort ist die volle Dekodierung über [`crate::decode`] bereits
/// günstig genug, daher liefert diese Funktion dort `None`.
pub fn extract_embedded_preview(path: &Path) -> Result<Option<DynamicImage>> {
    if classify(path) == FileKind::Fallback {
        return Ok(None);
    }

    let source = RawSource::new(path).map_err(|source| AppError::io(path, source))?;
    let decoder = rawler::get_decoder(&source)
        .map_err(|err| AppError::decode(path, format!("Decoder nicht gefunden: {err}")))?;
    let params = RawDecodeParams::default();

    // Erst das kleine Thumbnail versuchen (am schnellsten zu lesen), dann
    // die größere Preview als Fallback, falls kein Thumbnail eingebettet
    // ist.
    if let Some(thumbnail) = decoder.thumbnail_image(&source, &params).map_err(|err| {
        AppError::decode(path, format!("Thumbnail-Extraktion fehlgeschlagen: {err}"))
    })? {
        return Ok(Some(thumbnail));
    }

    if let Some(preview) = decoder.preview_image(&source, &params).map_err(|err| {
        AppError::decode(path, format!("Preview-Extraktion fehlgeschlagen: {err}"))
    })? {
        return Ok(Some(preview));
    }

    Ok(None)
}
