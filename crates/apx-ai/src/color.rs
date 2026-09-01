//! Gemeinsame Farbraum-Hilfsfunktionen für die Segmentierungs-Heuristiken
//! (`segmentation`-Modul) — reine, allokationsfreie Funktionen auf
//! einzelnen `0.0..=1.0`-normierten RGB-Tripeln.

/// Rec.601-Luminanzgewichte — dieselben wie
/// `apx_pipeline::stages::masks::luminance_range_alpha` (siehe dort),
/// hier erneut definiert statt importiert: `apx-pipeline`s Funktion ist
/// nicht öffentlich, und eine Ein-Zeilen-Formel eigens dafür crate-
/// übergreifend zu exportieren wäre unverhältnismäßig.
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.299 * r + 0.587 * g + 0.114 * b
}

/// Wandelt ein `0.0..=1.0`-normiertes RGB-Tripel nach YCbCr um (ITU-R
/// BT.601, Vollbereich) — `Y` wie [`luminance`], `Cb`/`Cr` liegen
/// ungefähr im Bereich `-0.5..=0.5`. Grundlage der Hautton-Erkennung in
/// `segmentation::person`.
pub fn rgb_to_ycbcr(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let y = luminance(r, g, b);
    let cb = -0.168_736 * r - 0.331_264 * g + 0.5 * b;
    let cr = 0.5 * r - 0.418_688 * g - 0.081_312 * b;
    (y, cb, cr)
}

/// Sättigung eines RGB-Tripels nach der üblichen HSV-Definition
/// (`(max - min) / max`, `0.0` für Schwarz) — dieselbe Formel wie
/// `frontend/src/lib/softProof.ts::saturationOf` (Phase 6 Schritt 10),
/// hier serverseitig für die Motiv-Saliency-Heuristik.
pub fn saturation(r: f32, g: f32, b: f32) -> f32 {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max <= 0.0 {
        0.0
    } else {
        (max - min) / max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_of_white_is_one() {
        assert!((luminance(1.0, 1.0, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ycbcr_of_gray_has_zero_chroma() {
        let (y, cb, cr) = rgb_to_ycbcr(0.5, 0.5, 0.5);
        assert!((y - 0.5).abs() < 1e-6);
        assert!(cb.abs() < 1e-6);
        assert!(cr.abs() < 1e-6);
    }

    #[test]
    fn saturation_of_gray_is_zero_and_of_pure_color_is_one() {
        assert!(saturation(0.5, 0.5, 0.5).abs() < 1e-6);
        assert!((saturation(1.0, 0.0, 0.0) - 1.0).abs() < 1e-6);
    }
}
