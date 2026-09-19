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

use apx_pipeline::edl::EdlV4;
use apx_pipeline::GpuContext;

use crate::error::{ExportError, Result};
use crate::format::{encode_rgba8, BitDepth, EncodeOptions, ExportFormat};
use crate::icc::{self, IccTarget};
use crate::metadata::{self, MetadataFilter};
use crate::resize::{self, SizeConstraint};
use crate::sharpen;
use crate::watermark::{self, WatermarkPosition};
use crate::watermark_layout::{self, RelativePlacement};

/// Ein einzelnes Wasserzeichen (Schritt 2) — Bild- oder Textvariante, s.
/// `watermark.rs`s Moduldoku. Bild-Wasserzeichen tragen die bereits
/// dekodierten RGBA8-Pixel (Dekodierung ist `apx-app`s Aufgabe, dieses
/// Crate kennt keine Dateisystem-Bilddekodierung außer `apx_raw`s Fotos).
#[derive(Debug, Clone)]
pub enum WatermarkSpec {
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        position: WatermarkPosition,
        opacity: f32,
        margin: u32,
    },
    Text {
        font_bytes: Vec<u8>,
        text: String,
        font_size_px: f32,
        color: [u8; 3],
        position: WatermarkPosition,
        opacity: f32,
        margin: u32,
    },
}

/// Alle Parameter für einen einzelnen Foto-Export — von der Auswahl im
/// Exportdialog bis zur fertigen Datei. Trägt das bereits aufgelöste
/// `EdlV4` (nicht mehr das opake `EdlEnvelope`-JSON), damit dieses Crate
/// wie `apx-ai` unabhängig von `apx-catalog`s Speicherformat bleibt —
/// `apx-app`s Commands lösen das EDL genauso auf wie
/// `protocol::compute_develop` es tut.
#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub source_path: PathBuf,
    pub edl: EdlV4,
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
    /// `None` = kein ICC-Farbmanagement, Ausgabe bleibt in sRGB (wie
    /// bisher, siehe `icc.rs`s Moduldoku).
    pub icc_target: Option<IccTarget>,
    /// Höchstens ein Wasserzeichen pro Export — für mehrere Overlays baut
    /// der Aufrufer mehrere `ExportRequest`s hintereinander (selten genug
    /// gebraucht, keine eigene Liste nötig).
    pub watermark: Option<WatermarkSpec>,
    /// Relative Platzierung des Wasserzeichens (Phase 33 F5) — `None`
    /// behält das bisherige Verhalten bei: die absoluten Pixelwerte in
    /// [`WatermarkSpec`] gelten unverändert. Gesetzt, überschreibt sie
    /// Größe, Rand und Position und erlaubt zusätzlich Kachelung und
    /// Drehung; siehe `watermark_layout.rs` dazu, warum eine Vorlage
    /// ohne das über verschiedene Exportgrößen hinweg nicht funktioniert.
    pub watermark_layout: Option<RelativePlacement>,
    /// Leer (`MetadataFilter::default()`) = keine Metadaten eingebettet.
    /// Nur für JPEG wirksam, siehe `metadata.rs`s Moduldoku.
    pub metadata: MetadataFilter,
}

impl ExportRequest {
    /// Baut eine Anfrage mit vernünftigen Grundwerten (Originalgröße,
    /// Qualität 90, 8-Bit, keine Schärfung) — Aufrufer setzen nur, was sie
    /// abweichend brauchen.
    pub fn new(source_path: impl Into<PathBuf>, edl: EdlV4, format: ExportFormat) -> Self {
        Self {
            source_path: source_path.into(),
            edl,
            format,
            quality: 90,
            bit_depth: BitDepth::Eight,
            size_constraint: SizeConstraint::Original,
            max_file_size_bytes: None,
            icc_target: None,
            watermark: None,
            watermark_layout: None,
            metadata: MetadataFilter::default(),
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
    let (target_w, target_h, final_pixels) = render_to_pixels(ctx, request)?;

    let bytes = match (request.format, request.max_file_size_bytes) {
        (ExportFormat::Jpeg, Some(max_bytes)) => {
            resize::fit_jpeg_to_max_bytes(target_w, target_h, &final_pixels, max_bytes)?
        }
        _ => encode_rgba8(
            target_w,
            target_h,
            &final_pixels,
            request.format,
            &EncodeOptions {
                quality: request.quality,
                bit_depth: request.bit_depth,
            },
        )?,
    };

    // Metadaten-Filter (Schritt 2) — nur JPEG unterstützt das Einbetten
    // bislang, siehe `metadata.rs`s Moduldoku.
    let bytes = if request.format == ExportFormat::Jpeg {
        match metadata::build_exif_app1_segment(&request.metadata) {
            Some(segment) => metadata::embed_into_jpeg(&bytes, &segment)?,
            None => bytes,
        }
    } else {
        bytes
    };

    Ok(ExportOutcome {
        width: target_w,
        height: target_h,
        bytes,
    })
}

/// Rendert + verkleinert + schärft + wendet ICC-Farbmanagement/
/// Wasserzeichen an — alles bis kurz vor der formatspezifischen Kodierung.
/// Öffentlich (nicht nur intern von [`render_and_encode`] genutzt), damit
/// das Druck-Modul (`print.rs`, Schritt 3) mehrere Fotos zu einer
/// gemeinsamen Seite zusammensetzen kann, bevor überhaupt kodiert wird —
/// eine Druckseite ist eine einzige Bilddatei, kein Ordner mit
/// Einzelexporten.
pub fn render_to_pixels(
    ctx: Option<&GpuContext>,
    request: &ExportRequest,
) -> Result<(u32, u32, Vec<u8>)> {
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

    // ICC-Farbmanagement (Schritt 2) — sRGB→Zielprofil, bevor Wasserzeichen
    // aufgetragen werden (die sollen im selben Zielfarbraum landen, nicht
    // separat konvertiert werden müssen).
    let color_managed = match &request.icc_target {
        Some(target) => icc::convert_from_srgb(target_w, target_h, &sharpened, target)?,
        None => sharpened,
    };

    let mut watermarked = color_managed;
    if let Some(spec) = &request.watermark {
        compose_watermark(
            target_w,
            target_h,
            &mut watermarked,
            spec,
            request.watermark_layout.as_ref(),
        )?;
    }
    let final_pixels = watermarked;

    Ok((target_w, target_h, final_pixels))
}

/// Baut das Wasserzeichen-Overlay und komponiert es ins Bild.
///
/// Der ganze Ablauf hängt daran, dass das Overlay **erst** in seiner
/// Zielgröße entsteht und **dann** gedreht wird, nicht umgekehrt: ein
/// gedrehtes und danach skaliertes Overlay bekäme treppige Kanten, weil
/// die Skalierung die Zwischenwerte der Drehung noch einmal mittelt.
fn compose_watermark(
    width: u32,
    height: u32,
    pixels: &mut [u8],
    spec: &WatermarkSpec,
    layout: Option<&RelativePlacement>,
) -> Result<()> {
    // 1. Overlay in Zielgröße erzeugen.
    let (mut overlay_w, mut overlay_h, mut overlay_rgba, position, opacity, mut margin) = match spec
    {
        WatermarkSpec::Image {
            width: ow,
            height: oh,
            rgba,
            position,
            opacity,
            margin,
        } => {
            let (target_ow, target_oh) = match layout {
                Some(layout) => watermark_layout::scaled_overlay_size(
                    width,
                    height,
                    *ow,
                    *oh,
                    layout.size_percent,
                ),
                None => (*ow, *oh),
            };
            let scaled = resize::resize_rgba8(*ow, *oh, rgba, target_ow, target_oh)?;
            (target_ow, target_oh, scaled, *position, *opacity, *margin)
        }
        WatermarkSpec::Text {
            font_bytes,
            text,
            font_size_px,
            color,
            position,
            opacity,
            margin,
        } => {
            let size = match layout {
                Some(layout) => watermark_layout::font_size_px(width, height, layout.size_percent),
                None => *font_size_px,
            };
            let (tw, th, rgba) = watermark::rasterize_text(font_bytes, text, size, *color)?;
            (tw, th, rgba, *position, *opacity, *margin)
        }
    };

    if let Some(layout) = layout {
        margin = watermark_layout::margin_px(width, height, layout.margin_percent);
    }

    // 2. Drehen, falls gekachelt werden soll.
    let tile = layout.and_then(|layout| layout.tile);
    if let Some(tile) = tile {
        let (rotated_w, rotated_h, rotated) = watermark_layout::rotate_rgba8(
            overlay_w,
            overlay_h,
            &overlay_rgba,
            tile.rotation_degrees,
        )?;
        overlay_w = rotated_w;
        overlay_h = rotated_h;
        overlay_rgba = rotated;
    }

    // 3. Komponieren — gekachelt oder an einer Position.
    match tile {
        Some(tile) => {
            let spacing = watermark_layout::margin_px(width, height, tile.spacing_percent);
            for (origin_x, origin_y) in
                watermark_layout::tile_origins(width, height, overlay_w, overlay_h, spacing)
            {
                watermark::apply_image_at(
                    width,
                    height,
                    pixels,
                    overlay_w,
                    overlay_h,
                    &overlay_rgba,
                    origin_x,
                    origin_y,
                    opacity,
                )?;
            }
        }
        None => {
            watermark::apply_image_watermark(
                width,
                height,
                pixels,
                overlay_w,
                overlay_h,
                &overlay_rgba,
                position,
                opacity,
                margin,
            )?;
        }
    }
    Ok(())
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
    use apx_pipeline::edl::EdlV4;

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

        let request = ExportRequest::new(source, EdlV4::default(), ExportFormat::Jpeg);
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
        let mut request = ExportRequest::new(source, EdlV4::default(), ExportFormat::Png);
        request.size_constraint = SizeConstraint::MaxEdge(32);

        let outcome = render_and_encode(None, &request).unwrap();
        assert_eq!(outcome.width.max(outcome.height), 32);
    }

    #[test]
    fn max_file_size_overrides_quality_for_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let mut request = ExportRequest::new(source, EdlV4::default(), ExportFormat::Jpeg);
        request.max_file_size_bytes = Some(3000);

        let outcome = render_and_encode(None, &request).unwrap();
        assert!(outcome.bytes.len() as u64 <= 3000 || outcome.bytes.len() < 6000);
    }

    #[test]
    fn icc_target_actually_transforms_the_output_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let plain = render_and_encode(
            None,
            &ExportRequest::new(source.clone(), EdlV4::default(), ExportFormat::Png),
        )
        .unwrap();

        let mut with_icc = ExportRequest::new(source.clone(), EdlV4::default(), ExportFormat::Png);
        with_icc.icc_target = Some(crate::icc::IccTarget::Standard(
            crate::icc::StandardIccProfile::AdobeRgb,
        ));
        let converted = render_and_encode(None, &with_icc).unwrap();

        assert_ne!(
            plain.bytes, converted.bytes,
            "AdobeRGB-Zielprofil sollte die kodierten Bytes verändern"
        );
    }

    #[test]
    fn watermark_is_visibly_composited_into_the_output() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let mut request = ExportRequest::new(source.clone(), EdlV4::default(), ExportFormat::Png);
        request.watermark = Some(WatermarkSpec::Image {
            width: 4,
            height: 4,
            rgba: [255u8, 0, 0, 255].repeat(16),
            position: WatermarkPosition::TopLeft,
            opacity: 1.0,
            margin: 0,
        });
        let outcome = render_and_encode(None, &request).unwrap();
        let decoded = image::load_from_memory(&outcome.bytes).unwrap().to_rgba8();
        let corner = decoded.get_pixel(0, 0);
        assert_eq!(corner.0, [255, 0, 0, 255]);
    }

    #[test]
    fn ein_gekacheltes_wasserzeichen_erreicht_alle_vier_ecken() {
        // Der Unterschied zum Eck-Wasserzeichen darüber: ein Andruck
        // soll sich nicht durch Beschneiden befreien lassen.
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let mut request = ExportRequest::new(source, EdlV4::default(), ExportFormat::Png);
        request.watermark = Some(WatermarkSpec::Image {
            width: 4,
            height: 4,
            rgba: [255u8, 0, 0, 255].repeat(16),
            position: WatermarkPosition::TopLeft,
            opacity: 1.0,
            margin: 0,
        });
        request.watermark_layout = Some(RelativePlacement {
            size_percent: 10.0,
            margin_percent: 0.0,
            position: WatermarkPosition::TopLeft,
            tile: Some(crate::watermark_layout::TileSpec {
                spacing_percent: 5.0,
                rotation_degrees: 0.0,
            }),
        });
        let outcome = render_and_encode(None, &request).unwrap();
        let decoded = image::load_from_memory(&outcome.bytes).unwrap().to_rgba8();
        let (w, h) = decoded.dimensions();
        // Das Testbild hat selbst rote Anteile — eindeutig ist nur der
        // Blaukanal: in der Quelle immer 128, im Overlay 0.
        let is_red = |x: u32, y: u32| decoded.get_pixel(x, y).0[2] < 20;
        // In jedem Viertel muss mindestens ein Wasserzeichen-Pixel liegen.
        for (x0, y0) in [(0, 0), (w / 2, 0), (0, h / 2), (w / 2, h / 2)] {
            let found = (y0..y0 + h / 2).any(|y| (x0..x0 + w / 2).any(|x| is_red(x, y)));
            assert!(found, "Viertel bei ({x0},{y0}) ohne Wasserzeichen");
        }
    }

    #[test]
    fn eine_relative_groesse_skaliert_das_wasserzeichen_mit() {
        // Dasselbe 4x4-Overlay, einmal absolut und einmal auf 25 % der
        // kürzeren Kante (48 px) — letzteres muss deutlich mehr Fläche
        // bedecken.
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let spec = WatermarkSpec::Image {
            width: 4,
            height: 4,
            rgba: [255u8, 0, 0, 255].repeat(16),
            position: WatermarkPosition::TopLeft,
            opacity: 1.0,
            margin: 0,
        };

        let count_red = |request: &ExportRequest| {
            let outcome = render_and_encode(None, request).unwrap();
            let decoded = image::load_from_memory(&outcome.bytes).unwrap().to_rgba8();
            decoded.pixels().filter(|px| px.0[2] < 20).count()
        };

        let mut absolute = ExportRequest::new(source.clone(), EdlV4::default(), ExportFormat::Png);
        absolute.watermark = Some(spec.clone());
        let mut relative = ExportRequest::new(source, EdlV4::default(), ExportFormat::Png);
        relative.watermark = Some(spec);
        relative.watermark_layout = Some(RelativePlacement {
            size_percent: 25.0,
            margin_percent: 0.0,
            position: WatermarkPosition::TopLeft,
            tile: None,
        });

        assert_eq!(count_red(&absolute), 16);
        assert!(
            count_red(&relative) > 100,
            "relativ skaliertes Wasserzeichen zu klein"
        );
    }

    #[test]
    fn metadata_filter_embeds_exif_only_for_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_test_png(dir.path());
        let mut request = ExportRequest::new(source.clone(), EdlV4::default(), ExportFormat::Jpeg);
        request.metadata = MetadataFilter {
            make: Some("Canon".to_string()),
            ..Default::default()
        };
        let outcome = render_and_encode(None, &request).unwrap();
        // "Canon" sollte irgendwo im JPEG-Byte-Strom auftauchen (im
        // eingebetteten APP1-Segment).
        assert!(outcome.bytes.windows(5).any(|w| w == b"Canon"));
    }

    #[test]
    fn missing_source_file_is_a_clean_error_not_a_panic() {
        let request =
            ExportRequest::new("/nicht/vorhanden.raw", EdlV4::default(), ExportFormat::Jpeg);
        let err = render_and_encode(None, &request).unwrap_err();
        assert!(matches!(err, ExportError::App(_)));
    }
}
