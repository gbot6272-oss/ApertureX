//! Frequenztrennung für Präzisions-Retusche (Phase 14 Schritt 2, siehe
//! `DECISIONS.md` ADR-0041 — Lightroom hat "kein eingebautes
//! Frequenztrennungs-Werkzeug wie Photoshop"). Zerlegt ein RGB-Bild in
//! eine **Tieffrequenz-Ebene** (Farbe/Ton, ein separierbarer
//! Box-Weichzeichner als Tiefpass — dieselbe "Box statt echte
//! Gauß-Unschärfe"-Vereinfachung wie [`super::masks::feather_alpha`]
//! bzw. [`super::details`]s Unsharp-Masking-Referenzweichzeichner) und
//! eine **Hochfrequenz-Ebene** (Textur/Poren/Kanten,
//! `Original − Tiefpass`, um [`HIGH_FREQUENCY_OFFSET`] verschoben zur
//! Sichtbarkeit — dieselbe Mittelgrau-Konvention wie ein
//! Photoshop-Hochpass).
//!
//! Reine Hilfsfunktionen — **kein** eigener Pipeline-Schritt in
//! `develop::render_rgba8`s fester Kette: [`super::repair`] ruft
//! [`split`]/[`combine`] selbst auf, wenn ein [`crate::edl::RepairStroke`]
//! per `layer` gezielt nur auf eine der beiden Ebenen wirken soll (siehe
//! dessen Moduldoku) — die Zerlegung passiert also nur für Striche, die
//! sie tatsächlich brauchen, nicht bei jedem Rendering pauschal.
//!
//! Das Frontend zeigt zusätzlich einen reinen Anzeige-Modus
//! (Normal/Tieffrequenz/Hochfrequenz) direkt auf dem bereits gerenderten
//! Vorschaubild — dieselbe clientseitige Berechnung wie
//! `frontend/src/lib/histogram.ts`, ohne eigenen Backend-Command.

use rayon::prelude::*;

/// Bruchteil der Bildbreite, der den Box-Tiefpass-Radius bestimmt — ein
/// fester, für Haut-/Texturretusche brauchbarer Vorgabewert (bewusst kein
/// eigener Regler in diesem Schritt, siehe `PLAN.md` Phase 14 Schritt 2),
/// dieselbe Art fester Referenzradius wie `details.rs::NR_BLUR_RADIUS`.
pub const SPLIT_RADIUS_FRACTION: f32 = 0.02;

/// Mittelgrau-Verschiebung der Hochfrequenz-Ebene zur Sichtbarkeit
/// (0.5 = 128/255 im `u8`-Raum) — dieselbe Konvention wie ein
/// Photoshop-Hochpass.
pub const HIGH_FREQUENCY_OFFSET: f32 = 0.5;

fn sample_at(pixels: &[f32], width: usize, height: usize, x: i32, y: i32, channel: usize) -> f32 {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    pixels[(cy * width + cx) * 3 + channel]
}

/// Separierbarer Box-Weichzeichner über alle drei Kanäle (zwei
/// eindimensionale Durchläufe statt eines quadratischen Kernels — bei
/// größeren Radien deutlich günstiger als `details.rs`s quadratischer
/// `box_blur_radius`, derselbe Ansatz wie `masks.rs::box_blur_1d`, hier
/// nur dreikanalig statt auf einem Ein-Kanal-Alpha-Puffer).
fn box_blur_1d_rgb(
    src: &[f32],
    width: usize,
    height: usize,
    radius: i32,
    horizontal: bool,
) -> Vec<f32> {
    (0..width * height)
        .into_par_iter()
        .flat_map_iter(move |index| {
            let x = (index % width) as i32;
            let y = (index / width) as i32;
            let mut sum = [0.0f32; 3];
            let mut count = 0.0f32;
            for offset in -radius..=radius {
                let (sx, sy) = if horizontal {
                    (x + offset, y)
                } else {
                    (x, y + offset)
                };
                if sx < 0 || sy < 0 || sx as usize >= width || sy as usize >= height {
                    continue;
                }
                for (c, slot) in sum.iter_mut().enumerate() {
                    *slot += sample_at(src, width, height, sx, sy, c);
                }
                count += 1.0;
            }
            sum.map(|v| if count > 0.0 { v / count } else { 0.0 })
        })
        .collect()
}

/// Der Box-Tiefpass allein (zwei Durchläufe: horizontal, dann vertikal —
/// zusammen eine gute Näherung an eine echte Gauß-Unschärfe).
pub fn low_pass(pixels: &[f32], width: u32, height: u32, radius_px: i32) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    let horizontal = box_blur_1d_rgb(pixels, w, h, radius_px, true);
    box_blur_1d_rgb(&horizontal, w, h, radius_px, false)
}

/// Radius in Pixeln für den Standard-Trennradius (siehe
/// [`SPLIT_RADIUS_FRACTION`]) bei der übergebenen Bildbreite.
pub fn default_split_radius_px(width: u32) -> i32 {
    (SPLIT_RADIUS_FRACTION * width as f32).round().max(1.0) as i32
}

/// Zerlegt `pixels` in (Tieffrequenz, Hochfrequenz) — siehe Moduldoku.
/// Beide Ebenen haben dieselbe Länge wie `pixels` (drei Kanäle je Pixel,
/// linearer `0.0..=1.0`-Wertebereich).
pub fn split(pixels: &[f32], width: u32, height: u32) -> (Vec<f32>, Vec<f32>) {
    let radius_px = default_split_radius_px(width);
    let low = low_pass(pixels, width, height, radius_px);
    let high: Vec<f32> = pixels
        .iter()
        .zip(low.iter())
        .map(|(&original, &blurred)| (original - blurred + HIGH_FREQUENCY_OFFSET).clamp(0.0, 1.0))
        .collect();
    (low, high)
}

/// Setzt eine zuvor per [`split`] zerlegte Tief-/Hochfrequenz-Ebene
/// wieder zu einem vollen Bild zusammen — die Umkehrung von [`split`]
/// (bis auf Rundungs-/Clamping-Verluste an den Rändern des
/// `0.0..=1.0`-Wertebereichs).
pub fn combine(low: &[f32], high: &[f32]) -> Vec<f32> {
    low.iter()
        .zip(high.iter())
        .map(|(&low, &high)| (low + (high - HIGH_FREQUENCY_OFFSET)).clamp(0.0, 1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_gray(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width * height * 3) as usize]
    }

    #[test]
    fn splitting_and_recombining_a_flat_image_is_lossless() {
        let pixels = flat_gray(16, 16, 0.42);
        let (low, high) = split(&pixels, 16, 16);
        let recombined = combine(&low, &high);
        for (original, result) in pixels.iter().zip(recombined.iter()) {
            assert!(
                (original - result).abs() < 1e-5,
                "verlustfrei bei flachem Bild: original={original} ergebnis={result}"
            );
        }
    }

    #[test]
    fn low_frequency_layer_smooths_out_a_sharp_single_pixel_spike() {
        let size = 20u32;
        let mut pixels = flat_gray(size, size, 0.2);
        let center = ((10 * size + 10) * 3) as usize;
        pixels[center] = 0.9;
        pixels[center + 1] = 0.9;
        pixels[center + 2] = 0.9;

        let (low, _high) = split(&pixels, size, size);
        assert!(
            low[center] < 0.9,
            "die Tieffrequenz-Ebene sollte den einzelnen scharfen Spitzenwert wegmitteln, war {}",
            low[center]
        );
        assert!(
            low[center] > 0.2,
            "der Tiefpass sollte den Spitzenwert trotzdem noch etwas anheben, war {}",
            low[center]
        );
    }

    #[test]
    fn high_frequency_layer_is_flat_mid_gray_for_a_flat_image() {
        // Ohne jede Hochfrequenz-Information (gleichmäßiges Bild) sollte
        // die Hochfrequenz-Ebene überall genau bei der Mittelgrau-
        // Verschiebung liegen (0.5) — sichtbar "leer".
        let pixels = flat_gray(12, 12, 0.6);
        let (_low, high) = split(&pixels, 12, 12);
        for value in high {
            assert!(
                (value - HIGH_FREQUENCY_OFFSET).abs() < 1e-5,
                "erwartete {HIGH_FREQUENCY_OFFSET}, war {value}"
            );
        }
    }
}
