//! Panorama-Zusammenführung (Phase 9 Schritt 8, siehe `PLAN.md`/
//! `DECISIONS.md` ADR-0035 Punkt 2) — **reine Verschiebungs-Registrierung
//! per 2D-Phasenkorrelation** (reines Rust über `rustfft`) für Stativ-/
//! gleicher-Blickpunkt-Aufnahmen mit rein translatorischem Versatz.
//! Liefert nur `dx`/`dy`, keine Rotation/Skalierung/Perspektive.
//!
//! Seit Phase 13 Schritt 5 (siehe `DECISIONS.md` ADR-0040-Nachtrag III)
//! gibt es mit [`super::homography_stitch`] eine zweite, leistungsfähigere
//! Registrierung für Freihandaufnahmen mit Rotation/Perspektive/
//! Parallaxe — `apx-app`s `stack_panorama`-Command versucht diese zuerst
//! und fällt hierher zurück, wenn keine verlässliche Homografie gefunden
//! wird. Dieses Modul bleibt trotzdem bestehen: einfacher, schneller und
//! für reine Stativaufnahmen ausreichend, außerdem weiterhin die
//! Registrierungsgrundlage für [`super::astro`].

use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;

use crate::error::{Result, StackingError};
use crate::luma::rgba8_to_luma_f32;

/// Führt eine 2D-FFT (zeilen- dann spaltenweise, Standardverfahren ohne
/// eigene 2D-FFT-Bibliothek) in-place auf `data` (`width * height`
/// Einträge, zeilenweise/row-major) aus. `inverse` wählt Hin-/Rücktransformation;
/// bewusst unnormiert (die Peak-*Position* interessiert hier, nicht die
/// Amplitude — eine fehlende `1/(width*height)`-Normierung verschiebt sie
/// nicht).
fn fft2d(data: &mut [Complex32], width: usize, height: usize, inverse: bool) {
    let mut planner = FftPlanner::new();

    let row_fft = if inverse {
        planner.plan_fft_inverse(width)
    } else {
        planner.plan_fft_forward(width)
    };
    for row in data.chunks_exact_mut(width) {
        row_fft.process(row);
    }

    let col_fft = if inverse {
        planner.plan_fft_inverse(height)
    } else {
        planner.plan_fft_forward(height)
    };
    let mut column = vec![Complex32::default(); height];
    for x in 0..width {
        for (y, slot) in column.iter_mut().enumerate() {
            *slot = data[y * width + x];
        }
        col_fft.process(&mut column);
        for (y, value) in column.iter().enumerate() {
            data[y * width + x] = *value;
        }
    }
}

/// Schätzt die reine Verschiebung (`dx`, `dy`) zwischen `a` und `b`
/// (beide RGBA8, identische Abmessungen) per 2D-Phasenkorrelation: Kreuz-
/// Leistungsspektrum `FFT(b) · conj(FFT(a)) / |...|`, dessen inverse FFT
/// einen scharfen Peak an der Verschiebungsposition hat. Ein positiver
/// `dx`/`dy` heißt „`b` ist `a`, nach rechts/unten verschoben" (modulo
/// Bildgröße — für Verschiebungen über die halbe Bildgröße hinaus wird
/// automatisch der kürzere, ggf. negative Weg gewählt).
pub fn estimate_shift_rgba8(a: &[u8], b: &[u8], width: u32, height: u32) -> Result<(i32, i32)> {
    let expected_len = (width as usize) * (height as usize) * 4;
    if a.len() != expected_len || b.len() != expected_len {
        return Err(StackingError::DimensionMismatch {
            message: format!(
                "beide Bilder müssen {width}x{height} RGBA8 sein (a={} Bytes, b={} Bytes, erwartet {expected_len})",
                a.len(),
                b.len()
            ),
        });
    }
    let w = width as usize;
    let h = height as usize;

    let mut fa: Vec<Complex32> = rgba8_to_luma_f32(a)
        .into_iter()
        .map(|v| Complex32::new(v, 0.0))
        .collect();
    let mut fb: Vec<Complex32> = rgba8_to_luma_f32(b)
        .into_iter()
        .map(|v| Complex32::new(v, 0.0))
        .collect();
    fft2d(&mut fa, w, h, false);
    fft2d(&mut fb, w, h, false);

    let mut cross: Vec<Complex32> = fa
        .iter()
        .zip(fb.iter())
        .map(|(&x, &y): (&Complex32, &Complex32)| {
            // `fb * conj(fa)` statt `fa * conj(fb)` — die empirisch
            // richtige Multiplikationsreihenfolge für die hier gewählte
            // Vorzeichenkonvention (positives `dx`/`dy` = „b liegt
            // gegenüber a nach rechts/unten verschoben"), siehe
            // Testabdeckung unten (`detects_a_pure_circular_translation`).
            let r = y * x.conj();
            let mag = r.norm();
            if mag > 1e-6 {
                r / mag
            } else {
                Complex32::new(0.0, 0.0)
            }
        })
        .collect();
    fft2d(&mut cross, w, h, true);

    let mut best_index = 0usize;
    let mut best_value = f32::MIN;
    for (index, value) in cross.iter().enumerate() {
        if value.re > best_value {
            best_value = value.re;
            best_index = index;
        }
    }
    let py = (best_index / w) as i32;
    let px = (best_index % w) as i32;
    let dx = if px > w as i32 / 2 { px - w as i32 } else { px };
    let dy = if py > h as i32 / 2 { py - h as i32 } else { py };
    Ok((dx, dy))
}

/// Ein Quellbild mit seinem geschätzten Versatz relativ zum ersten Bild
/// (das immer bei `(0, 0)` steht).
pub struct PositionedImage<'a> {
    pub pixels: &'a [u8],
    pub offset_x: i32,
    pub offset_y: i32,
}

/// Setzt `images` (alle `width * height` RGBA8) auf einer gemeinsamen
/// Leinwand zusammen — Überlappungsbereiche werden gemittelt (kein
/// Feathering/keine Nahtoptimierung, siehe Moduldoku für die v1-
/// Beschränkung auf reine Verschiebung).
pub fn stitch_shift_rgba8(
    images: &[PositionedImage],
    width: u32,
    height: u32,
) -> Result<(u32, u32, Vec<u8>)> {
    if images.len() < 2 {
        return Err(StackingError::TooFewImages {
            message: format!(
                "Panorama-Zusammenführung braucht mindestens 2 Bilder, {} übergeben",
                images.len()
            ),
        });
    }
    let expected_len = (width as usize) * (height as usize) * 4;
    for (index, image) in images.iter().enumerate() {
        if image.pixels.len() != expected_len {
            return Err(StackingError::DimensionMismatch {
                message: format!(
                    "Bild {index} hat {} Bytes, erwartet wurden {expected_len} ({width}x{height} RGBA8)",
                    image.pixels.len()
                ),
            });
        }
    }

    let min_x = images.iter().map(|i| i.offset_x).min().unwrap_or(0);
    let min_y = images.iter().map(|i| i.offset_y).min().unwrap_or(0);
    let max_x = images
        .iter()
        .map(|i| i.offset_x + width as i32)
        .max()
        .unwrap_or(width as i32);
    let max_y = images
        .iter()
        .map(|i| i.offset_y + height as i32)
        .max()
        .unwrap_or(height as i32);
    let canvas_width = (max_x - min_x) as u32;
    let canvas_height = (max_y - min_y) as u32;
    let canvas_pixels = (canvas_width as usize) * (canvas_height as usize);

    let mut sum = vec![[0.0f32; 3]; canvas_pixels];
    let mut count = vec![0u32; canvas_pixels];

    for image in images {
        let origin_x = image.offset_x - min_x;
        let origin_y = image.offset_y - min_y;
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let src_index = ((y * width as i32 + x) * 4) as usize;
                let dst_x = origin_x + x;
                let dst_y = origin_y + y;
                let dst_index = (dst_y as usize) * (canvas_width as usize) + dst_x as usize;
                sum[dst_index][0] += image.pixels[src_index] as f32;
                sum[dst_index][1] += image.pixels[src_index + 1] as f32;
                sum[dst_index][2] += image.pixels[src_index + 2] as f32;
                count[dst_index] += 1;
            }
        }
    }

    let mut canvas = vec![0u8; canvas_pixels * 4];
    for pixel in 0..canvas_pixels {
        let n = count[pixel].max(1) as f32;
        canvas[pixel * 4] = (sum[pixel][0] / n).round() as u8;
        canvas[pixel * 4 + 1] = (sum[pixel][1] / n).round() as u8;
        canvas[pixel * 4 + 2] = (sum[pixel][2] / n).round() as u8;
        canvas[pixel * 4 + 3] = if count[pixel] > 0 { 255 } else { 0 };
    }
    Ok((canvas_width, canvas_height, canvas))
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

    /// Baut ein Testbild mit einem hellen 3×3-Block an `(bx, by)` vor
    /// einem dunklen Hintergrund — ein klar lokalisierbares Merkmal für
    /// die Phasenkorrelation.
    fn image_with_bright_block(width: u32, height: u32, bx: u32, by: u32) -> Vec<u8> {
        let mut pixels = solid(width, height, 10, 10, 10);
        for dy in 0..3u32 {
            for dx in 0..3u32 {
                let x = (bx + dx) % width;
                let y = (by + dy) % height;
                let index = ((y * width + x) * 4) as usize;
                pixels[index] = 250;
                pixels[index + 1] = 250;
                pixels[index + 2] = 250;
            }
        }
        pixels
    }

    /// Zirkuläre Verschiebung (wie `numpy.roll`) — die für
    /// Phasenkorrelation "saubere" Art, ein Testbild zu verschieben:
    /// keine Randartefakte, exakt das Modell, das die Fourier-Methode
    /// annimmt (periodisches Signal).
    fn circular_shift(pixels: &[u8], width: u32, height: u32, dx: i32, dy: i32) -> Vec<u8> {
        let mut out = vec![0u8; pixels.len()];
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let src_x = x.rem_euclid(width as i32);
                let src_y = y.rem_euclid(height as i32);
                let dst_x = (x + dx).rem_euclid(width as i32) as u32;
                let dst_y = (y + dy).rem_euclid(height as i32) as u32;
                let src_index = ((src_y as u32 * width + src_x as u32) * 4) as usize;
                let dst_index = ((dst_y * width + dst_x) * 4) as usize;
                out[dst_index..dst_index + 4].copy_from_slice(&pixels[src_index..src_index + 4]);
            }
        }
        out
    }

    #[test]
    fn identical_images_have_zero_shift() {
        let width = 16u32;
        let height = 16u32;
        let a = image_with_bright_block(width, height, 4, 4);
        let (dx, dy) = estimate_shift_rgba8(&a, &a, width, height).expect("sollte schätzen");
        assert_eq!((dx, dy), (0, 0));
    }

    #[test]
    fn detects_a_pure_circular_translation() {
        let width = 16u32;
        let height = 16u32;
        let a = image_with_bright_block(width, height, 4, 4);
        let shifted = circular_shift(&a, width, height, 3, 5);
        let (dx, dy) = estimate_shift_rgba8(&a, &shifted, width, height).expect("sollte schätzen");
        assert_eq!((dx, dy), (3, 5));
    }

    #[test]
    fn estimate_shift_rejects_mismatched_dimensions() {
        let a = solid(4, 4, 1, 2, 3);
        let b = solid(2, 2, 1, 2, 3);
        let result = estimate_shift_rgba8(&a, &b, 4, 4);
        assert!(matches!(
            result,
            Err(StackingError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn stitch_rejects_a_single_image() {
        let a = solid(4, 4, 1, 2, 3);
        let result = stitch_shift_rgba8(
            &[PositionedImage {
                pixels: &a,
                offset_x: 0,
                offset_y: 0,
            }],
            4,
            4,
        );
        assert!(matches!(result, Err(StackingError::TooFewImages { .. })));
    }

    #[test]
    fn stitch_produces_a_canvas_sized_to_the_combined_extent() {
        let width = 4u32;
        let height = 4u32;
        let a = solid(width, height, 200, 0, 0);
        let b = solid(width, height, 0, 200, 0);
        // `b` liegt komplett rechts von `a`, kein Überlapp.
        let (canvas_w, canvas_h, canvas) = stitch_shift_rgba8(
            &[
                PositionedImage {
                    pixels: &a,
                    offset_x: 0,
                    offset_y: 0,
                },
                PositionedImage {
                    pixels: &b,
                    offset_x: 4,
                    offset_y: 0,
                },
            ],
            width,
            height,
        )
        .expect("sollte zusammensetzen");
        assert_eq!((canvas_w, canvas_h), (8, 4));
        // Links: reines Rot von `a`.
        assert_eq!(&canvas[0..3], &[200, 0, 0]);
        // Rechts: reines Grün von `b`.
        let right_index = (4 * 4) as usize; // Pixel (4,0)
        assert_eq!(&canvas[right_index..right_index + 3], &[0, 200, 0]);
    }

    #[test]
    fn stitch_averages_an_overlapping_region() {
        let width = 4u32;
        let height = 4u32;
        let a = solid(width, height, 200, 0, 0);
        let b = solid(width, height, 0, 200, 0);
        // Volle Überlappung (gleicher Versatz) — jeder Kanal wird gemittelt.
        let (_, _, canvas) = stitch_shift_rgba8(
            &[
                PositionedImage {
                    pixels: &a,
                    offset_x: 0,
                    offset_y: 0,
                },
                PositionedImage {
                    pixels: &b,
                    offset_x: 0,
                    offset_y: 0,
                },
            ],
            width,
            height,
        )
        .expect("sollte zusammensetzen");
        assert_eq!(&canvas[0..3], &[100, 100, 0]);
    }
}
