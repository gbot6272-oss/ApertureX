//! Fokus-Stacking (Phase 9 Schritt 8, siehe `PLAN.md`/`DECISIONS.md`
//! ADR-0035 Punkt 2) — kombiniert mehrere Aufnahmen desselben Motivs mit
//! unterschiedlichem Fokuspunkt zu einem durchgehend scharfen Bild.
//!
//! **Voraussetzung**: alle Quellbilder sind bereits pixelgenau
//! ausgerichtet (z. B. Stativaufnahmen) und haben identische Abmessungen
//! — dieses Modul registriert nicht, es wählt nur je Pixel die schärfste
//! Quelle. Eine echte Ausrichtungs-Bibliothek für Freihandaufnahmen ist
//! hier bewusst nicht enthalten (dasselbe Beschaffungsproblem wie
//! `opencv` in ADR-0035s Kontext-Recherche).

use crate::error::{Result, StackingError};
use crate::luma::rgba8_to_luma_f32;

/// Ein per-Pixel-Schärfemaß über die Luminanz: der quadrierte
/// diskrete Laplace-Operator (4-Nachbarschaft) — Standardmaß für
/// Fokus-Stacking (hohe Werte an scharfen Kanten/Texturen, niedrige
/// Werte in unscharfen Bereichen).
fn laplacian_sharpness_map(luma: &[f32], width: u32, height: u32) -> Vec<f32> {
    let w = width as i32;
    let h = height as i32;
    let at = |x: i32, y: i32| -> f32 {
        let cx = x.clamp(0, w - 1);
        let cy = y.clamp(0, h - 1);
        luma[(cy * w + cx) as usize]
    };
    let mut out = vec![0.0f32; luma.len()];
    for y in 0..h {
        for x in 0..w {
            let center = at(x, y);
            let laplacian =
                4.0 * center - at(x - 1, y) - at(x + 1, y) - at(x, y - 1) - at(x, y + 1);
            out[(y * w + x) as usize] = laplacian * laplacian;
        }
    }
    out
}

/// Fokus-Stacking über `images` (jeweils RGBA8, `width * height * 4`
/// Bytes, identische Abmessungen) — für jeden Pixel wird die Quelle mit
/// der höchsten lokalen Schärfe übernommen (kein Feather-Übergang
/// zwischen Quellen: bei bereits ausgerichteten Stativaufnahmen liegt die
/// Schärfegrenze meist an einer natürlichen Tiefenschärfe-Kante, ein
/// harter Wechsel dort ist unauffällig).
pub fn focus_stack_rgba8(images: &[&[u8]], width: u32, height: u32) -> Result<Vec<u8>> {
    if images.len() < 2 {
        return Err(StackingError::TooFewImages {
            message: format!(
                "Fokus-Stacking braucht mindestens 2 Bilder, {} übergeben",
                images.len()
            ),
        });
    }
    let expected_len = (width as usize) * (height as usize) * 4;
    for (index, image) in images.iter().enumerate() {
        if image.len() != expected_len {
            return Err(StackingError::DimensionMismatch {
                message: format!(
                    "Bild {index} hat {} Bytes, erwartet wurden {expected_len} ({width}x{height} RGBA8)",
                    image.len()
                ),
            });
        }
    }

    let sharpness_maps: Vec<Vec<f32>> = images
        .iter()
        .map(|image| {
            let luma = rgba8_to_luma_f32(image);
            laplacian_sharpness_map(&luma, width, height)
        })
        .collect();

    let pixel_count = (width as usize) * (height as usize);
    let mut out = vec![0u8; expected_len];
    for pixel in 0..pixel_count {
        let mut best_index = 0usize;
        let mut best_sharpness = sharpness_maps[0][pixel];
        for (index, map) in sharpness_maps.iter().enumerate().skip(1) {
            if map[pixel] > best_sharpness {
                best_sharpness = map[pixel];
                best_index = index;
            }
        }
        let src = &images[best_index][pixel * 4..pixel * 4 + 4];
        out[pixel * 4..pixel * 4 + 4].copy_from_slice(src);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
        pixels
    }

    #[test]
    fn rejects_a_single_image() {
        let image = solid(4, 4, 100, 100, 100);
        let result = focus_stack_rgba8(&[&image], 4, 4);
        assert!(matches!(result, Err(StackingError::TooFewImages { .. })));
    }

    #[test]
    fn rejects_mismatched_dimensions() {
        let a = solid(4, 4, 100, 100, 100);
        let b = solid(2, 2, 100, 100, 100);
        let result = focus_stack_rgba8(&[&a, &b], 4, 4);
        assert!(matches!(
            result,
            Err(StackingError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn picks_the_sharper_source_at_a_checkerboard_edge() {
        // Bild A: scharfes Schachbrettmuster (hohe lokale Kontraste).
        // Bild B: flaches Grau (keine Kanten, "unscharf").
        let width = 8u32;
        let height = 8u32;
        let mut sharp = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let value = if (x + y) % 2 == 0 { 20 } else { 230 };
                let index = ((y * width + x) * 4) as usize;
                sharp[index] = value;
                sharp[index + 1] = value;
                sharp[index + 2] = value;
                sharp[index + 3] = 255;
            }
        }
        let flat = solid(width, height, 125, 125, 125);

        let stacked = focus_stack_rgba8(&[&flat, &sharp], width, height).expect("sollte stacken");
        // In der Bildmitte (weg vom geklemmten Rand) muss die scharfe
        // Quelle gewinnen.
        let index = ((4 * width + 4) * 4) as usize;
        assert_ne!(
            stacked[index], 125,
            "die scharfe Quelle sollte an einer Kante gewinnen"
        );
    }

    #[test]
    fn a_uniform_stack_is_identity() {
        let width = 5u32;
        let height = 5u32;
        let a = solid(width, height, 10, 20, 30);
        let b = solid(width, height, 10, 20, 30);
        let stacked = focus_stack_rgba8(&[&a, &b], width, height).expect("sollte stacken");
        assert_eq!(stacked, a, "identische Quellen ergeben dasselbe Ergebnis");
    }

    #[test]
    fn alpha_channel_stays_opaque() {
        let width = 3u32;
        let height = 3u32;
        let a = solid(width, height, 1, 2, 3);
        let b = solid(width, height, 4, 5, 6);
        let stacked = focus_stack_rgba8(&[&a, &b], width, height).expect("sollte stacken");
        for pixel in stacked.chunks_exact(4) {
            assert_eq!(pixel[3], 255);
        }
    }
}
