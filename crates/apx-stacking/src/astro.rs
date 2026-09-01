//! Astro-Stacking (Phase 9 Schritt 8, siehe `PLAN.md`/`DECISIONS.md`
//! ADR-0035 Punkt 2) — viele rauschige Kurzbelichtungen (z. B. Sternfeld-
//! Einzelaufnahmen) zu einem rauschärmeren Summenbild mitteln, mit
//! Sigma-Clipping gegen Ausreißer (Flugzeuge/Satellitenspuren,
//! kosmische Strahlung auf dem Sensor).
//!
//! **Bewusste Vereinfachung**: die Registrierung (Ausrichtung der
//! Einzelbilder aufeinander) nutzt dieselbe reine Verschiebungs-
//! Phasenkorrelation wie `panorama` — eine echte Sternzentroid-/
//! Dreiecks-Registrierung (robust gegen Erdrotation zwischen Frames,
//! subpixelgenau) bleibt zurückgestellt, siehe `PLAN.md`.

use crate::error::{Result, StackingError};
use crate::panorama::estimate_shift_rgba8;

/// Verschiebt `pixels` um `(dx, dy)` (ganzzahlige Pixel) — Bereiche, die
/// dadurch außerhalb des Originals liegen würden, übernehmen den
/// nächstgelegenen Randpixel (Clamp, wie beim Bilateral-Filter in
/// `apx_ai::denoise`) statt schwarz/transparent zu werden, damit
/// Sigma-Clipping an den Rändern nicht sofort jeden Frame als Ausreißer
/// verwirft.
fn shift_clamped_rgba8(pixels: &[u8], width: u32, height: u32, dx: i32, dy: i32) -> Vec<u8> {
    let w = width as i32;
    let h = height as i32;
    let mut out = vec![0u8; pixels.len()];
    for y in 0..h {
        for x in 0..w {
            let src_x = (x - dx).clamp(0, w - 1);
            let src_y = (y - dy).clamp(0, h - 1);
            let src_index = ((src_y * w + src_x) * 4) as usize;
            let dst_index = ((y * w + x) * 4) as usize;
            out[dst_index..dst_index + 4].copy_from_slice(&pixels[src_index..src_index + 4]);
        }
    }
    out
}

/// Sigma-geclipptes Mittel über bereits ausgerichtete `images` (alle
/// `width * height` RGBA8): je Pixel und Kanal wird Mittelwert/
/// Standardabweichung über alle Frames berechnet, Werte außerhalb von
/// `mean ± sigma * stddev` werden verworfen, der Rest gemittelt. Bei
/// `stddev == 0` (alle Frames identisch) oder wenn Sigma-Clipping jeden
/// Wert verwerfen würde, fällt der Pixel auf das einfache arithmetische
/// Mittel zurück.
pub fn sigma_clipped_mean_stack_rgba8(
    images: &[&[u8]],
    width: u32,
    height: u32,
    sigma: f32,
) -> Result<Vec<u8>> {
    if images.len() < 3 {
        return Err(StackingError::TooFewImages {
            message: format!(
                "Sigma-geclipptes Astro-Stacking braucht mindestens 3 Frames (sonst lässt sich kein Ausreißer sinnvoll erkennen), {} übergeben",
                images.len()
            ),
        });
    }
    let expected_len = (width as usize) * (height as usize) * 4;
    for (index, image) in images.iter().enumerate() {
        if image.len() != expected_len {
            return Err(StackingError::DimensionMismatch {
                message: format!(
                    "Frame {index} hat {} Bytes, erwartet wurden {expected_len} ({width}x{height} RGBA8)",
                    image.len()
                ),
            });
        }
    }

    let pixel_count = (width as usize) * (height as usize);
    let mut out = vec![0u8; expected_len];
    let n = images.len() as f32;

    for pixel in 0..pixel_count {
        for channel in 0..3 {
            let byte_index = pixel * 4 + channel;
            let values: Vec<f32> = images.iter().map(|img| img[byte_index] as f32).collect();
            let mean = values.iter().sum::<f32>() / n;
            let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
            let stddev = variance.sqrt();

            let (sum, count) = if stddev > 0.0 {
                values
                    .iter()
                    .filter(|&&v| (v - mean).abs() <= sigma * stddev)
                    .fold((0.0f32, 0u32), |(sum, count), &v| (sum + v, count + 1))
            } else {
                (0.0, 0)
            };
            out[byte_index] = if count > 0 {
                (sum / count as f32).round() as u8
            } else {
                mean.round() as u8
            };
        }
        out[pixel * 4 + 3] = 255;
    }
    Ok(out)
}

/// Registriert `frames[1..]` gegen `frames[0]` (Phasenkorrelations-
/// Verschiebung, siehe `panorama::estimate_shift_rgba8`) und wendet
/// anschließend [`sigma_clipped_mean_stack_rgba8`] an — die bequeme
/// End-zu-Ende-Funktion für den Tauri-Command; die beiden Bausteine
/// bleiben einzeln testbar.
pub fn register_and_stack_astro_rgba8(
    frames: &[&[u8]],
    width: u32,
    height: u32,
    sigma: f32,
) -> Result<Vec<u8>> {
    if frames.is_empty() {
        return Err(StackingError::TooFewImages {
            message: "keine Frames übergeben".to_string(),
        });
    }
    let reference = frames[0];
    let mut aligned: Vec<Vec<u8>> = vec![reference.to_vec()];
    for frame in &frames[1..] {
        let (dx, dy) = estimate_shift_rgba8(reference, frame, width, height)?;
        aligned.push(shift_clamped_rgba8(frame, width, height, dx, dy));
    }
    let refs: Vec<&[u8]> = aligned.iter().map(|v| v.as_slice()).collect();
    sigma_clipped_mean_stack_rgba8(&refs, width, height, sigma)
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
    fn rejects_fewer_than_three_frames() {
        let a = solid(2, 2, 10, 10, 10);
        let b = solid(2, 2, 10, 10, 10);
        let result = sigma_clipped_mean_stack_rgba8(&[&a, &b], 2, 2, 3.0);
        assert!(matches!(result, Err(StackingError::TooFewImages { .. })));
    }

    #[test]
    fn rejects_mismatched_dimensions() {
        let a = solid(4, 4, 10, 10, 10);
        let b = solid(4, 4, 10, 10, 10);
        let c = solid(2, 2, 10, 10, 10);
        let result = sigma_clipped_mean_stack_rgba8(&[&a, &b, &c], 4, 4, 3.0);
        assert!(matches!(
            result,
            Err(StackingError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn discards_a_single_outlier_frame() {
        // Neun Frames bei Wert 100, ein zehnter Frame ist ein extremer
        // Ausreißer (z. B. eine Flugzeug-/Satellitenspur bei 255) — das
        // sigma-geclippte Mittel muss deutlich näher an 100 als an einem
        // naiven arithmetischen Mittel (~115.5) liegen.
        let width = 2u32;
        let height = 2u32;
        let mut frames: Vec<Vec<u8>> = (0..9)
            .map(|_| solid(width, height, 100, 100, 100))
            .collect();
        frames.push(solid(width, height, 255, 255, 255));
        let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();

        let stacked =
            sigma_clipped_mean_stack_rgba8(&refs, width, height, 2.0).expect("sollte stacken");
        assert!(
            stacked[0] < 110,
            "Ausreißer sollte weitgehend verworfen werden (bekam {})",
            stacked[0]
        );
    }

    #[test]
    fn a_uniform_stack_reproduces_the_constant_value() {
        let width = 2u32;
        let height = 2u32;
        let frames: Vec<Vec<u8>> = (0..5).map(|_| solid(width, height, 42, 42, 42)).collect();
        let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();
        let stacked =
            sigma_clipped_mean_stack_rgba8(&refs, width, height, 3.0).expect("sollte stacken");
        assert_eq!(stacked[0], 42);
    }

    #[test]
    fn alpha_channel_stays_opaque() {
        let width = 2u32;
        let height = 2u32;
        let frames: Vec<Vec<u8>> = (0..4)
            .map(|i| solid(width, height, i * 10, i * 10, i * 10))
            .collect();
        let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();
        let stacked =
            sigma_clipped_mean_stack_rgba8(&refs, width, height, 3.0).expect("sollte stacken");
        for pixel in stacked.chunks_exact(4) {
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn register_and_stack_handles_already_aligned_uniform_frames() {
        let width = 4u32;
        let height = 4u32;
        let frames: Vec<Vec<u8>> = (0..4).map(|_| solid(width, height, 60, 60, 60)).collect();
        let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();
        let stacked = register_and_stack_astro_rgba8(&refs, width, height, 3.0)
            .expect("sollte registrieren und stacken");
        assert_eq!(stacked[0], 60);
    }
}
