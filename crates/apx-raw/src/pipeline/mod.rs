//! Volle Bilddekodierung: RAW-Sensordaten → fertiges, orientiertes 16-Bit-
//! RGB-Bild. Orchestriert die in `PHASE1_PROMPT.md` Abschnitt 3 festgelegte
//! provisorische Kette:
//!
//! 1. CFA-Daten laden (`rawler`).
//! 2. Schwarzpunkt abziehen, auf Weißpunkt normalisieren (`normalize_mosaic`).
//! 3. Demosaicing, bilinear oder half-size (`demosaic`-Modul).
//! 4. Kamera-Weißabgleich (`color`-Modul).
//! 5. Kamera-RGB → sRGB (`color`-Modul).
//! 6. sRGB-Gammakurve (`color`-Modul).
//! 7. Ausgabe als 16-Bit-RGB, danach EXIF-Orientierung genau einmal
//!    angewendet (`orientation`-Modul).

mod color;
mod demosaic;

use std::path::{Path, PathBuf};

use apx_core::{AppError, Result};
use rawler::decoders::RawDecodeParams;
use rawler::imgop::Rect;
use rawler::rawsource::RawSource;
use rawler::{RawImage, RawImageData};

use crate::format::{classify, FileKind};
use crate::orientation::Orientation;

use color::{cam_to_srgb_matrix, to_u16, ColorPipeline};
// `srgb_gamma` wird zusätzlich als Teil der öffentlichen `apx-raw`-API
// re-exportiert (siehe `lib.rs`), damit `apx-pipeline` denselben
// Gamma-Encoder verwendet wie die bestehende `decode()`-Kette — statt die
// Formel für den neuen Entwickeln-Renderpfad ein zweites Mal
// abzuschreiben (siehe `DECISIONS.md` ADR-0019).
pub use color::srgb_gamma;
use demosaic::{demosaic_full, demosaic_half};

/// Ergebnis von [`decode`]: interleaved 16-Bit-RGB, Zeile für Zeile,
/// bereits mit angewendeter EXIF-Orientierung — das Frontend darf keine
/// eigene Rotation mehr vornehmen (siehe `orientation`-Modul).
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// Länge = `width * height * 3`.
    pub pixels: Vec<u16>,
}

/// Ergebnis von [`decode_linear`]: interleaved `f32`-RGB, bereits
/// zugeschnitten und orientiert wie [`DecodedImage`], aber **vor**
/// Weißabgleich und Gammakurve — `apx-pipeline` übernimmt ab hier
/// (siehe `DECISIONS.md` ADR-0015). Additiv zu [`decode`]/[`DecodedImage`]:
/// bestehende Aufrufer (Vorschau-Erzeugung) bleiben unverändert, siehe
/// Modul-Doku oben.
#[derive(Debug, Clone)]
pub struct LinearImage {
    pub width: u32,
    pub height: u32,
    /// Länge = `width * height * 3`. Werte typischerweise in `[0, ~1]`,
    /// bewusst nicht geclamped (siehe `SPEC.md` §2.2: 32-Bit-Float
    /// intern, Clamping erst bei der finalen Ausgabe-Transformation).
    pub pixels: Vec<f32>,
    /// As-shot-Weißabgleich-Koeffizienten `[R, G, B, E]` aus den
    /// RAW-Metadaten (siehe `pipeline::color::ColorPipeline`), damit
    /// `apx-pipeline` den in `WhiteBalanceAdjustment` beschriebenen
    /// *relativen* Shift in tatsächliche Kanal-Gains umrechnen kann. Für
    /// Fallback-Formate (JPEG/PNG/TIFF, siehe [`decode_linear`]) neutral
    /// `[1.0, 1.0, 1.0, 1.0]`, da dort kein Sensor-Weißabgleich existiert.
    pub as_shot_wb_coeffs: [f32; 4],
    /// Feste 3×3-Matrix Kamera-RGB → linear-sRGB (D65), dieselbe Berechnung
    /// wie [`ColorPipeline::from_raw_image`] für [`decode`] nutzt — anders
    /// als der Weißabgleich ist diese Transformation nicht nutzerseitig
    /// verstellbar, `apx-pipeline` wendet sie unverändert an (siehe
    /// `DECISIONS.md` ADR-0019). Für Fallback-Formate (bereits sRGB-nah)
    /// die Einheitsmatrix.
    pub cam_to_srgb: [[f32; 3]; 3],
}

/// Einheitsmatrix — Kamera-RGB-Farbmatrix-Ersatz für Fallback-Formate
/// (JPEG/PNG/TIFF), die keine `xyz_to_cam`-Kameradaten mitbringen und
/// bereits näherungsweise sRGB-kodiert sind (siehe [`decode_linear`]).
const IDENTITY_RGB_MATRIX: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

impl DecodedImage {
    /// Wandelt das Ergebnis in ein `image::DynamicImage` um, z. B. um es
    /// als JPEG zu speichern (Vorschau-Cache) oder über den
    /// Custom-Protokoll-Handler auszuliefern. Gibt `None` zurück, wenn
    /// `pixels` nicht zu `width * height * 3` passt — das würde auf einen
    /// internen Fehler in `decode()` hindeuten, nicht auf einen
    /// Nutzerfehler, daher hier keine `Result`-Rückgabe, sondern ein
    /// dokumentierter Optionswert zur Absicherung durch den Aufrufer.
    pub fn into_dynamic_image(self) -> Option<image::DynamicImage> {
        image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_raw(
            self.width,
            self.height,
            self.pixels,
        )
        .map(image::DynamicImage::ImageRgb16)
    }
}

/// Dekodiert eine RAW- oder Fallback-Bilddatei vollständig.
///
/// `max_edge`: `None` liefert die volle Auflösung; `Some(edge)` liefert ein
/// Bild, dessen längere Kante höchstens `edge` Pixel misst (für RAWs wird
/// dafür, wenn ausreichend, der schnellere Half-Size-Demosaic-Pfad genutzt).
pub fn decode(path: &Path, max_edge: Option<u32>) -> Result<DecodedImage> {
    match classify(path) {
        FileKind::Raw => decode_raw(path, max_edge),
        FileKind::Fallback => crate::fallback::decode(path, max_edge),
    }
}

/// Dekodiert bis kurz vor Weißabgleich/Gammakurve — der Einstiegspunkt für
/// `apx-pipeline` ab Phase 2 (siehe `DECISIONS.md` ADR-0015). Additiv zu
/// [`decode`]: bestehende Aufrufer sind davon nicht betroffen.
///
/// Für Fallback-Formate (JPEG/PNG/TIFF) gibt es keinen Sensor-
/// Weißabgleich zum Rückgängigmachen — hier wird bewusst vereinfacht der
/// bereits fertige sRGB-Puffer aus [`decode`] direkt als `LinearImage`
/// mit neutralen As-shot-Koeffizienten weitergereicht (technisch nicht
/// linear, sondern schon gammakodiert — für Phase 2s Regler auf
/// Fallback-Formaten ausreichend, siehe `PLAN.md` Phase 2 Schritt 4).
pub fn decode_linear(path: &Path, max_edge: Option<u32>) -> Result<LinearImage> {
    match classify(path) {
        FileKind::Raw => decode_raw_linear(path, max_edge),
        FileKind::Fallback => {
            let decoded = crate::fallback::decode(path, max_edge)?;
            let pixels = decoded.pixels.iter().map(|&v| v as f32 / 65535.0).collect();
            Ok(LinearImage {
                width: decoded.width,
                height: decoded.height,
                pixels,
                as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
                cam_to_srgb: IDENTITY_RGB_MATRIX,
            })
        }
    }
}

fn decode_raw_linear(path: &Path, max_edge: Option<u32>) -> Result<LinearImage> {
    let source = RawSource::new(path).map_err(|source| AppError::io(path, source))?;
    let decoder = rawler::get_decoder(&source)
        .map_err(|err| AppError::decode(path, format!("Decoder nicht gefunden: {err}")))?;
    let params = RawDecodeParams::default();

    let image = decoder
        .raw_image(&source, &params, false)
        .map_err(|err| AppError::decode(path, format!("RAW-Dekodierung fehlgeschlagen: {err}")))?;

    if image.cpp != 1 {
        return Err(AppError::Unsupported(format!(
            "'{}': {} Komponenten pro Pixel — nur einkanalige CFA-Sensoren werden unterstützt",
            path.display(),
            image.cpp
        )));
    }

    let full_w = image.width;
    let full_h = image.height;
    let mosaic = normalize_mosaic(&image);
    let cfa = &image.camera.cfa;

    let use_half_size = match max_edge {
        None => false,
        Some(edge) => ((full_w.max(full_h) / 2) as u32) >= edge,
    };

    let (demosaiced, w, h) = if use_half_size {
        demosaic_half(&mosaic, full_w, full_h, cfa)
    } else {
        (demosaic_full(&mosaic, full_w, full_h, cfa), full_w, full_h)
    };

    let scale_divisor = if use_half_size { 2 } else { 1 };
    let (cropped, cw, ch) =
        crop_to_active_area(&demosaiced, w, h, image.active_area.as_ref(), scale_divisor);

    let orientation: Orientation = image.orientation.into();
    let (oriented, ow, oh) = orientation.apply_rgb_f32(&cropped, cw as u32, ch as u32);

    let result = LinearImage {
        width: ow,
        height: oh,
        pixels: oriented,
        as_shot_wb_coeffs: image.wb_coeffs,
        cam_to_srgb: cam_to_srgb_matrix(&image.xyz_to_cam),
    };

    match max_edge {
        Some(edge) => downsample_linear_if_needed(path, result, edge),
        None => Ok(result),
    }
}

/// Wie [`downsample_if_needed`], aber für [`LinearImage`] (`f32`-Puffer).
fn downsample_linear_if_needed(
    path: &Path,
    image: LinearImage,
    max_edge: u32,
) -> Result<LinearImage> {
    if image.width.max(image.height) <= max_edge {
        return Ok(image);
    }

    let scale = max_edge as f32 / image.width.max(image.height) as f32;
    let new_w = ((image.width as f32 * scale).round() as u32).max(1);
    let new_h = ((image.height as f32 * scale).round() as u32).max(1);

    let buffer: image::ImageBuffer<image::Rgb<f32>, Vec<f32>> =
        image::ImageBuffer::from_raw(image.width, image.height, image.pixels).ok_or_else(|| {
            AppError::Decode {
                path: PathBuf::from(path),
                message: "Interner Fehler: Pixelpuffer passt nicht zu den Bilddimensionen"
                    .to_string(),
            }
        })?;
    let resized =
        image::imageops::resize(&buffer, new_w, new_h, image::imageops::FilterType::Lanczos3);

    Ok(LinearImage {
        width: new_w,
        height: new_h,
        pixels: resized.into_raw(),
        as_shot_wb_coeffs: image.as_shot_wb_coeffs,
        cam_to_srgb: image.cam_to_srgb,
    })
}

fn decode_raw(path: &Path, max_edge: Option<u32>) -> Result<DecodedImage> {
    let source = RawSource::new(path).map_err(|source| AppError::io(path, source))?;
    let decoder = rawler::get_decoder(&source)
        .map_err(|err| AppError::decode(path, format!("Decoder nicht gefunden: {err}")))?;
    let params = RawDecodeParams::default();

    let image = decoder
        .raw_image(&source, &params, false)
        .map_err(|err| AppError::decode(path, format!("RAW-Dekodierung fehlgeschlagen: {err}")))?;

    if image.cpp != 1 {
        // Linear-RAW-DNGs (bereits demosaicedes RGB im Sensorformat) und
        // andere Mehrkanal-Rohformate sind in Phase 1 bewusst nicht
        // abgedeckt — siehe PHASE1_PROMPT.md: "keine Platzhalter" gilt
        // auch andersherum, ein stiller Falsch-Fall wäre schlimmer als
        // ein klarer Fehler. Wird in einer späteren Phase ergänzt.
        return Err(AppError::Unsupported(format!(
            "'{}': {} Komponenten pro Pixel — nur einkanalige CFA-Sensoren werden in Phase 1 unterstützt",
            path.display(),
            image.cpp
        )));
    }

    let full_w = image.width;
    let full_h = image.height;
    let mosaic = normalize_mosaic(&image);
    let cfa = &image.camera.cfa;

    // Half-Size nutzen, wenn dessen Auflösung die angeforderte Zielgröße
    // bereits erreicht oder übertrifft — dann lohnt sich das teurere
    // volle Demosaicing nicht.
    let use_half_size = match max_edge {
        None => false,
        Some(edge) => ((full_w.max(full_h) / 2) as u32) >= edge,
    };

    let (demosaiced, w, h) = if use_half_size {
        demosaic_half(&mosaic, full_w, full_h, cfa)
    } else {
        (demosaic_full(&mosaic, full_w, full_h, cfa), full_w, full_h)
    };

    let color = ColorPipeline::from_raw_image(&image);
    let mut rgb16 = vec![0u16; w * h * 3];
    for pixel in 0..(w * h) {
        let demosaiced_rgb = [
            demosaiced[pixel * 3],
            demosaiced[pixel * 3 + 1],
            demosaiced[pixel * 3 + 2],
        ];
        let srgb_linear = color.to_linear_srgb(demosaiced_rgb);
        for (channel, value) in srgb_linear.iter().enumerate() {
            rgb16[pixel * 3 + channel] = to_u16(srgb_gamma(*value));
        }
    }

    let scale_divisor = if use_half_size { 2 } else { 1 };
    let (cropped, cw, ch) =
        crop_to_active_area(&rgb16, w, h, image.active_area.as_ref(), scale_divisor);

    let orientation: Orientation = image.orientation.into();
    let (oriented, ow, oh) = orientation.apply_rgb16(&cropped, cw as u32, ch as u32);

    let result = DecodedImage {
        width: ow,
        height: oh,
        pixels: oriented,
    };

    match max_edge {
        Some(edge) => downsample_if_needed(path, result, edge),
        None => Ok(result),
    }
}

/// Zieht Schwarzpunkt ab und normalisiert auf den Weißpunkt, Kanal für
/// Kanal (Bayer-2×2-Kachel, verallgemeinert auf andere Muster durch
/// `BlackLevel`/`WhiteLevel::as_bayer_array()`, siehe dort). Ergebnis liegt
/// in `[0, 1]`.
fn normalize_mosaic(image: &RawImage) -> Vec<f32> {
    let width = image.width;
    let height = image.height;
    let black = image.blacklevel.as_bayer_array();
    let white = image.whitelevel.as_bayer_array();

    let raw_f32: Vec<f32> = match &image.data {
        RawImageData::Integer(values) => values.iter().map(|&v| v as f32).collect(),
        RawImageData::Float(values) => values.clone(),
    };

    let mut out = vec![0.0_f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let tile = (y % 2) * 2 + (x % 2);
            let black_level = black[tile];
            let white_level = white[tile];
            let denom = (white_level - black_level).max(1.0);
            let idx = y * width + x;
            out[idx] = ((raw_f32[idx] - black_level) / denom).clamp(0.0, 1.0);
        }
    }
    out
}

/// Schneidet den nutzbaren Bereich (`active_area`, sofern vorhanden) aus
/// einem interleaved RGB-Puffer aus. `scale_divisor` berücksichtigt, dass
/// `active_area` in voller Sensorauflösung angegeben ist, während `rgb`
/// bei Half-Size-Demosaicing bereits halbiert wurde.
///
/// Generisch über den Elementtyp (`u16` für [`decode`], `f32` für
/// [`decode_linear`]) — dieselbe Geometrie-Logik, siehe DECISIONS.md
/// ADR-0015.
fn crop_to_active_area<T: Copy + Default>(
    rgb: &[T],
    w: usize,
    h: usize,
    active_area: Option<&Rect>,
    scale_divisor: usize,
) -> (Vec<T>, usize, usize) {
    let Some(rect) = active_area else {
        return (rgb.to_vec(), w, h);
    };

    let x0 = (rect.p.x / scale_divisor).min(w.saturating_sub(1));
    let y0 = (rect.p.y / scale_divisor).min(h.saturating_sub(1));
    let crop_w = (rect.d.w / scale_divisor).min(w - x0).max(1);
    let crop_h = (rect.d.h / scale_divisor).min(h - y0).max(1);

    let mut out = vec![T::default(); crop_w * crop_h * 3];
    for row in 0..crop_h {
        let src_start = ((y0 + row) * w + x0) * 3;
        let dst_start = row * crop_w * 3;
        out[dst_start..dst_start + crop_w * 3]
            .copy_from_slice(&rgb[src_start..src_start + crop_w * 3]);
    }
    (out, crop_w, crop_h)
}

/// Skaliert auf die exakt angeforderte Kantenlänge herunter, falls das
/// Bild (nach Half-Size-Demosaicing oder vollem Decode) noch größer ist.
fn downsample_if_needed(path: &Path, image: DecodedImage, max_edge: u32) -> Result<DecodedImage> {
    if image.width.max(image.height) <= max_edge {
        return Ok(image);
    }

    let scale = max_edge as f32 / image.width.max(image.height) as f32;
    let new_w = ((image.width as f32 * scale).round() as u32).max(1);
    let new_h = ((image.height as f32 * scale).round() as u32).max(1);

    let buffer: image::ImageBuffer<image::Rgb<u16>, Vec<u16>> =
        image::ImageBuffer::from_raw(image.width, image.height, image.pixels).ok_or_else(|| {
            AppError::Decode {
                path: PathBuf::from(path),
                message: "Interner Fehler: Pixelpuffer passt nicht zu den Bilddimensionen"
                    .to_string(),
            }
        })?;
    let resized =
        image::imageops::resize(&buffer, new_w, new_h, image::imageops::FilterType::Lanczos3);

    Ok(DecodedImage {
        width: new_w,
        height: new_h,
        pixels: resized.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_dynamic_image_succeeds_for_matching_buffer() {
        let img = DecodedImage {
            width: 2,
            height: 1,
            pixels: vec![1, 2, 3, 4, 5, 6],
        };
        let dynamic = img.into_dynamic_image().expect("Puffer passt zu den Maßen");
        assert_eq!((dynamic.width(), dynamic.height()), (2, 1));
    }

    #[test]
    fn into_dynamic_image_rejects_mismatched_buffer() {
        let img = DecodedImage {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3], // zu kurz für 2x2x3
        };
        assert!(img.into_dynamic_image().is_none());
    }

    #[test]
    fn crop_without_active_area_is_identity() {
        let rgb = vec![1u16, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let (out, w, h) = crop_to_active_area(&rgb, 2, 2, None, 1);
        assert_eq!((w, h), (2, 2));
        assert_eq!(out, rgb);
    }

    #[test]
    fn crop_extracts_expected_subregion() {
        // 4x4-Bild (nur Rot-Kanal zur Vereinfachung ungleich 0), active
        // area ist das mittlere 2x2-Fenster ab (1,1).
        let w = 4;
        let h = 4;
        let mut rgb = vec![0u16; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                rgb[(y * w + x) * 3] = (y * w + x) as u16;
            }
        }
        let rect = Rect::new(
            rawler::imgop::Point::new(1, 1),
            rawler::imgop::Dim2::new(2, 2),
        );
        let (out, cw, ch) = crop_to_active_area(&rgb, w, h, Some(&rect), 1);
        assert_eq!((cw, ch), (2, 2));
        // Erwartete Originalindizes: (1,1)=5, (2,1)=6, (1,2)=9, (2,2)=10
        assert_eq!(out[0], 5);
        assert_eq!(out[3], 6);
        assert_eq!(out[6], 9);
        assert_eq!(out[9], 10);
    }

    #[test]
    fn downsample_no_op_when_already_small_enough() {
        let img = DecodedImage {
            width: 4,
            height: 4,
            pixels: vec![100u16; 4 * 4 * 3],
        };
        let result =
            downsample_if_needed(Path::new("test.raw"), img, 8).expect("darf nicht fehlschlagen");
        assert_eq!((result.width, result.height), (4, 4));
    }

    #[test]
    fn downsample_shrinks_to_requested_edge() {
        let img = DecodedImage {
            width: 100,
            height: 50,
            pixels: vec![200u16; 100 * 50 * 3],
        };
        let result =
            downsample_if_needed(Path::new("test.raw"), img, 20).expect("darf nicht fehlschlagen");
        assert_eq!(result.width, 20);
        assert_eq!(result.height, 10);
        assert_eq!(result.pixels.len(), 20 * 10 * 3);
    }

    // Ab hier: LinearImage-Gegenstücke (siehe decode_linear,
    // DECISIONS.md ADR-0015) — dieselben Fälle wie oben, für den f32-Pfad.

    #[test]
    fn crop_to_active_area_works_generically_for_f32() {
        let w = 4;
        let h = 4;
        let mut rgb = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                rgb[(y * w + x) * 3] = (y * w + x) as f32;
            }
        }
        let rect = Rect::new(
            rawler::imgop::Point::new(1, 1),
            rawler::imgop::Dim2::new(2, 2),
        );
        let (out, cw, ch) = crop_to_active_area(&rgb, w, h, Some(&rect), 1);
        assert_eq!((cw, ch), (2, 2));
        assert_eq!(out[0], 5.0);
        assert_eq!(out[3], 6.0);
        assert_eq!(out[6], 9.0);
        assert_eq!(out[9], 10.0);
    }

    #[test]
    fn downsample_linear_no_op_when_already_small_enough() {
        let img = LinearImage {
            width: 4,
            height: 4,
            pixels: vec![0.5f32; 4 * 4 * 3],
            as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
            cam_to_srgb: IDENTITY_RGB_MATRIX,
        };
        let result = downsample_linear_if_needed(Path::new("test.raw"), img, 8)
            .expect("darf nicht fehlschlagen");
        assert_eq!((result.width, result.height), (4, 4));
    }

    #[test]
    fn downsample_linear_shrinks_to_requested_edge_and_keeps_wb_coeffs() {
        let img = LinearImage {
            width: 100,
            height: 50,
            pixels: vec![0.3f32; 100 * 50 * 3],
            as_shot_wb_coeffs: [1.2, 1.0, 0.8, 1.0],
            cam_to_srgb: IDENTITY_RGB_MATRIX,
        };
        let result = downsample_linear_if_needed(Path::new("test.raw"), img, 20)
            .expect("darf nicht fehlschlagen");
        assert_eq!(result.width, 20);
        assert_eq!(result.height, 10);
        assert_eq!(result.pixels.len(), 20 * 10 * 3);
        assert_eq!(result.as_shot_wb_coeffs, [1.2, 1.0, 0.8, 1.0]);
    }

    #[test]
    fn downsample_linear_keeps_cam_to_srgb_matrix() {
        let matrix = [[1.1, 0.0, -0.1], [0.0, 1.0, 0.0], [-0.05, 0.0, 1.05]];
        let img = LinearImage {
            width: 8,
            height: 4,
            pixels: vec![0.4f32; 8 * 4 * 3],
            as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
            cam_to_srgb: matrix,
        };
        let result = downsample_linear_if_needed(Path::new("test.raw"), img, 4)
            .expect("darf nicht fehlschlagen");
        assert_eq!(result.cam_to_srgb, matrix);
    }
}
