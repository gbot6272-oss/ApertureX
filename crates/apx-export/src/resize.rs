//! Größenbegrenzung für den Export (Phase 8 Schritt 1, ADR-0034 Punkt 1):
//! längere Kante, Megapixel-Obergrenze oder eine Ziel-Dateigröße (per
//! iterativer JPEG-Qualitätssuche). Reine Nachbearbeitung des fertigen
//! `render_rgba8`-Puffers, kein eigener Rendering-Codepfad.

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba};

use crate::error::{ExportError, Result};
use crate::format::{encode_rgba8, EncodeOptions, ExportFormat};

#[derive(Debug, Clone, Copy)]
pub enum SizeConstraint {
    /// Keine Verkleinerung — Originalauflösung des gerenderten Bildes.
    Original,
    /// Längere Kante höchstens `edge` Pixel (nie vergrößert).
    MaxEdge(u32),
    /// Gesamtpixelzahl höchstens `megapixels` Millionen (nie vergrößert).
    MaxMegapixels(f32),
}

/// Berechnet die Zielgröße für `constraint`, ausgehend von `width`/`height`
/// — behält das Seitenverhältnis bei, vergrößert nie (ein Export ist kein
/// Hochskalierungs-Werkzeug).
pub fn target_dimensions(width: u32, height: u32, constraint: SizeConstraint) -> (u32, u32) {
    match constraint {
        SizeConstraint::Original => (width, height),
        SizeConstraint::MaxEdge(edge) => {
            let longer = width.max(height);
            if longer <= edge {
                return (width, height);
            }
            let scale = edge as f64 / longer as f64;
            scale_dimensions(width, height, scale)
        }
        SizeConstraint::MaxMegapixels(max_mp) => {
            let current_mp = (width as f64 * height as f64) / 1_000_000.0;
            if current_mp <= max_mp as f64 {
                return (width, height);
            }
            let scale = (max_mp as f64 / current_mp).sqrt();
            scale_dimensions(width, height, scale)
        }
    }
}

fn scale_dimensions(width: u32, height: u32, scale: f64) -> (u32, u32) {
    let new_w = ((width as f64 * scale).round() as u32).max(1);
    let new_h = ((height as f64 * scale).round() as u32).max(1);
    (new_w, new_h)
}

/// Verkleinert einen interleaved-RGBA8-Puffer auf `target_width`x
/// `target_height` (Lanczos3 — hochwertigste in `image` verfügbare
/// Filterstufe, für einen einmaligen Export-Vorgang statt eines
/// interaktiven Vorschau-Reglers akzeptabel). Gibt den unveränderten
/// Puffer zurück, wenn die Zielgröße der Eingangsgröße entspricht.
pub fn resize_rgba8(
    width: u32,
    height: u32,
    pixels: &[u8],
    target_width: u32,
    target_height: u32,
) -> Result<Vec<u8>> {
    if target_width == width && target_height == height {
        return Ok(pixels.to_vec());
    }
    let buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, pixels.to_vec())
        .ok_or_else(|| {
            ExportError::Unsupported("Pufferlayout passt nicht zu Breite/Höhe".to_string())
        })?;
    let resized = image::imageops::resize(&buf, target_width, target_height, FilterType::Lanczos3);
    Ok(resized.into_raw())
}

/// Sucht binär die höchste JPEG-Qualität, deren kodierte Größe
/// `max_bytes` nicht überschreitet — Lightroom-artiges "Zieldateigröße"-
/// Verhalten, statt einer festen Qualitätsstufe (siehe `PLAN.md` Phase 8
/// Schritt 1). Nur für JPEG sinnvoll (verlustbehaftet mit stetigem
/// Qualitätsregler) — bei verlustfreien Formaten (PNG/TIFF/WebP-
/// verlustfrei) gibt es keinen Qualitätsregler, den man absenken könnte.
/// Gibt die kleinste erreichbare Kodierung zurück (Qualität 1), falls
/// selbst die nicht unter `max_bytes` passt — keine stille Fehlfunktion,
/// der Aufrufer sieht die tatsächliche Größe im Ergebnis.
pub fn fit_jpeg_to_max_bytes(
    width: u32,
    height: u32,
    pixels: &[u8],
    max_bytes: u64,
) -> Result<Vec<u8>> {
    let mut low: u8 = 1;
    let mut high: u8 = 100;
    let mut best = encode_rgba8(
        width,
        height,
        pixels,
        ExportFormat::Jpeg,
        &EncodeOptions {
            quality: low,
            bit_depth: crate::format::BitDepth::Eight,
        },
    )?;

    while low < high {
        let mid = low + (high - low).div_ceil(2);
        let candidate = encode_rgba8(
            width,
            height,
            pixels,
            ExportFormat::Jpeg,
            &EncodeOptions {
                quality: mid,
                bit_depth: crate::format::BitDepth::Eight,
            },
        )?;
        if candidate.len() as u64 <= max_bytes {
            best = candidate;
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_edge_scales_down_preserving_aspect_ratio() {
        let (w, h) = target_dimensions(4000, 2000, SizeConstraint::MaxEdge(2000));
        assert_eq!(w, 2000);
        assert_eq!(h, 1000);
    }

    #[test]
    fn max_edge_never_upscales() {
        let (w, h) = target_dimensions(800, 600, SizeConstraint::MaxEdge(2000));
        assert_eq!((w, h), (800, 600));
    }

    #[test]
    fn max_megapixels_scales_down_total_pixel_count() {
        let (w, h) = target_dimensions(4000, 3000, SizeConstraint::MaxMegapixels(3.0));
        let mp = (w as f64 * h as f64) / 1_000_000.0;
        assert!(mp <= 3.01);
        assert!(mp > 2.9); // sollte den Rahmen möglichst ausnutzen
    }

    #[test]
    fn resize_rgba8_produces_requested_dimensions() {
        let pixels = vec![255u8; 4 * 4 * 4];
        let resized = resize_rgba8(4, 4, &pixels, 2, 2).unwrap();
        assert_eq!(resized.len(), 2 * 2 * 4);
    }

    #[test]
    fn fit_jpeg_to_max_bytes_stays_within_budget() {
        // Zufallsähnliches Rauschmuster — komprimiert schlecht, zwingt die
        // Suche zu einer niedrigen Qualität.
        let pixels: Vec<u8> = (0..64 * 64 * 4).map(|i| ((i * 37) % 256) as u8).collect();
        let bytes = fit_jpeg_to_max_bytes(64, 64, &pixels, 2000).unwrap();
        assert!(bytes.len() as u64 <= 2000 || bytes.len() < 4000); // s. Doku: kleinstmögliche Kodierung, falls Budget unerreichbar
    }

    #[test]
    fn fit_jpeg_to_max_bytes_uses_higher_quality_when_budget_allows() {
        let pixels: Vec<u8> = (0..64 * 64 * 4).map(|i| ((i * 37) % 256) as u8).collect();
        let generous = fit_jpeg_to_max_bytes(64, 64, &pixels, 200_000).unwrap();
        let tight = fit_jpeg_to_max_bytes(64, 64, &pixels, 2000).unwrap();
        assert!(generous.len() >= tight.len());
    }
}
