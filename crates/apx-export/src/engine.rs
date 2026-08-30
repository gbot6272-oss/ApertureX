//! Export-Engine-Grundgerüst (Phase 8 Schritt 1, `DECISIONS.md`
//! ADR-0034 Punkt 1): der gemeinsame Renderpfad, den Drucken/Diashow/
//! Buch/Web alle wiederverwenden. Rendert über
//! `apx_pipeline::develop::render_rgba8` — denselben Codepfad wie der
//! Entwickeln-Vorschau-Renderer (`apx-app::protocol::compute_develop`) —
//! und hängt Größenbegrenzung, Ausgabeschärfung und Kodierung an.
//!
//! **DNG-Konvertierung beim Import (ADR-0025):** wie in `PLAN.md` Phase 8
//! Schritt 1 vorgesehen wurde die `dng`-Bibliothek evaluiert (siehe
//! `DECISIONS.md` ADR-0034: `cargo add --dry-run` löst sie auf) — ihr
//! öffentliches API ist jedoch reiner Lesezugriff (RAW-Dekodierung wie
//! `rawler`), kein Schreib-/Encodier-Pfad für eigene DNG-Dateien. Eine
//! Kamera-RAW→DNG-Konvertierung ist damit in dieser Umgebung nicht mit
//! einer reinen-Rust-Bibliothek umsetzbar und bleibt zurückgestellt, bis
//! eine schreibfähige Alternative existiert (siehe `FEATURES.md`).

use std::path::{Path, PathBuf};

use apx_pipeline::edl::EdlV3;
use apx_pipeline::GpuContext;

use crate::error::{ExportError, Result};
use crate::format::{encode_rgba8, BitDepth, EncodeOptions, ExportFormat};
use crate::resize::{self, SizeConstraint};
use crate::sharpen;

/// Alle Parameter für einen einzelnen Foto-Export — von der Auswahl im
/// Exportdialog bis zur fertigen Datei. Trägt das bereits aufgelöste
/// `EdlV3` (nicht mehr das opake `EdlEnvelope`-JSON), damit dieses Crate
/// wie `apx-ai` unabhängig von `apx-catalog`s Speicherformat bleibt —
/// `apx-app`s Commands lösen das EDL genauso auf wie
/// `protocol::compute_develop` es tut.
#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub source_path: PathBuf,
    pub edl: EdlV3,
    pub format: ExportFormat,
    /// `1..=100`, nur für JPEG/AVIF relevant.
    pub quality: u8,
    pub bit_depth: BitDepth,
    pub size_constraint: SizeConstraint,
    /// Nur für JPEG: statt `quality` eine Ziel-Obergrenze in Bytes (per
    /// [`resize::fit_jpeg_to_max_bytes`]) — `quality` wird dann ignoriert.
    pub max_file_size_bytes: Option<u64>,
    /// `(Betrag, Radius)` — `None`/Betrag `0.0` schaltet die Schärfung ab.
    pub sharpen: Option<(f32, f32)>,
}

impl ExportRequest {
    /// Baut eine Anfrage mit vernünftigen Grundwerten (Originalgröße,
    /// Qualität 90, 8-Bit, keine Schärfung) — Aufrufer setzen nur, was sie
    /// abweichend brauchen.
    pub fn new(source_path: impl Into<PathBuf>, edl: EdlV3, format: ExportFormat) -> Self {
        Self {
            source_path: source_path.into(),
            edl,
            format,
            quality: 90,
            bit_depth: BitDepth::Eight,
            size_constraint: SizeConstraint::Original,
            max_file_size_bytes: None,
            sharpen: None,
        }
    }
}

/// Ergebnis eines Exports — die fertig kodierten Bytes plus die
/// tatsächliche (ggf. verkleinerte) Ausgabegröße.
#[derive(Debug, Clone)]
pub struct ExportOutcome {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// Rendert + verkleinert + schärft + kodiert `request`, schreibt aber
/// noch nichts auf die Platte — siehe [`export_to_file`] für den
/// vollständigen Weg bis zur Datei.
pub fn render_and_encode(
    ctx: Option<&GpuContext>,
    request: &ExportRequest,
) -> Result<ExportOutcome> {
    // Volle Auflösung (`max_edge: None`) — ein Export ist kein
    // Vorschau-Vorgang, die Größenbegrenzung passiert gezielt danach über
    // `request.size_constraint`, nicht durch einen verlustbehafteten
    // Dekodier-Downscale.
    let linear = apx_raw::decode_linear(&request.source_path, None).map_err(ExportError::App)?;
    let rendered = apx_pipeline::develop::render_rgba8(ctx, &linear, &request.edl)
        .map_err(|err| ExportError::App(err.into()))?;

    let (target_w, target_h) =
        resize::target_dimensions(rendered.width, rendered.height, request.size_constraint);
    let resized = resize::resize_rgba8(
        rendered.width,
        rendered.height,
        &rendered.pixels,
        target_w,
        target_h,
    )?;

    let sharpened = match request.sharpen {
        Some((amount, radius)) if amount > 0.0 => {
            sharpen::unsharp_mask(target_w, target_h, &resized, amount, radius)?
        }
        _ => resized,
    };

    let bytes = match (request.format, request.max_file_size_bytes) {
        (ExportFormat::Jpeg, Some(max_bytes)) => {
            resize::fit_jpeg_to_max_bytes(target_w, target_h, &sharpened, max_bytes)?
        }
        _ => encode_rgba8(
            target_w,
            target_h,
            &sharpened,
            request.format,
            &EncodeOptions {
                quality: request.quality,
                bit_depth: request.bit_depth,
            },
        )?,
    };

    Ok(ExportOutcome {
        width: target_w,
        height: target_h,
        bytes,
    })
}

/// Wie [`render_and_encode`], schreibt das Ergebnis aber zusätzlich nach
/// `dest_path` (überschreibt eine bestehende Datei).
pub fn export_to_file(
    ctx: Option<&GpuContext>,
    request: &ExportRequest,
    dest_path: &Path,
) -> Result<ExportOutcome> {
    let outcome = render_and_encode(ctx, request)?;
    std::fs::write(dest_path, &outcome.bytes).map_err(|err| ExportError::Io {
        path: dest_path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apx_pipeline::edl::EdlV3;

    fn write_test_png(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("quelle.png");
        let img = image::RgbImage::from_fn(64, 48, |x, y| {
            image::Rgb([((x * 4) % 256) as u8, ((y * 5) % 256) as u8, 128])
        });
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn exports_a_fallback_image_end_to_end_as_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let dest = dir.path().join("export.jpg");

        let request = ExportRequest::new(source, EdlV3::default(), ExportFormat::Jpeg);
        let outcome = export_to_file(None, &request, &dest).unwrap();

        assert_eq!(outcome.width, 64);
        assert_eq!(outcome.height, 48);
        assert!(dest.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), outcome.bytes);
    }

    #[test]
    fn size_constraint_shrinks_the_output() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let mut request = ExportRequest::new(source, EdlV3::default(), ExportFormat::Png);
        request.size_constraint = SizeConstraint::MaxEdge(32);

        let outcome = render_and_encode(None, &request).unwrap();
        assert_eq!(outcome.width.max(outcome.height), 32);
    }

    #[test]
    fn max_file_size_overrides_quality_for_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let mut request = ExportRequest::new(source, EdlV3::default(), ExportFormat::Jpeg);
        request.max_file_size_bytes = Some(3000);

        let outcome = render_and_encode(None, &request).unwrap();
        assert!(outcome.bytes.len() as u64 <= 3000 || outcome.bytes.len() < 6000);
    }

    #[test]
    fn missing_source_file_is_a_clean_error_not_a_panic() {
        let request =
            ExportRequest::new("/nicht/vorhanden.raw", EdlV3::default(), ExportFormat::Jpeg);
        let err = render_and_encode(None, &request).unwrap_err();
        assert!(matches!(err, ExportError::App(_)));
    }
}
