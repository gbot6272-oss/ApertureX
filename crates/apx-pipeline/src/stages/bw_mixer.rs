//! Schwarzweiß-Mixer (Phase 9 Schritt 5, siehe `PLAN.md`/`DECISIONS.md`
//! ADR-0035) — läuft wie `curves.rs` bewusst *nach* der Farbraum-
//! Konvertierung auf dem fertigen sRGB-RGBA8-Puffer, nicht im linearen
//! Arbeitsraum: die acht Farbton-Bänder sind exakt dieselben, nach denen
//! auch die HSL-Regler gewichten (`hsl_color_mixer.rs`), eine Gauß-
//! Gewichtung nach Farbton-Abstand zum jeweiligen Bandzentrum — dieselbe
//! Konvention wiederzuverwenden statt eine zweite, abweichende
//! Bandaufteilung zu erfinden.

use super::color_math::{circular_distance_degrees, gaussian_weight, rgb_to_hsl};
use crate::edl::v3::BlackAndWhiteMixerAdjustment;

const BAND_COUNT: usize = 8;
const BAND_CENTERS_DEGREES: [f32; BAND_COUNT] =
    [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];
const BAND_SIGMA_DEGREES: f32 = 25.0;

/// Rec.-709-Luminanzgewichte — dieselben, mit denen `lib/histogram.ts`s
/// Frontend-Pendant rechnet, damit Histogramm-Anzeige und tatsächliches
/// Rendern konsistent bleiben.
const LUMA_R: f32 = 0.2126;
const LUMA_G: f32 = 0.7152;
const LUMA_B: f32 = 0.0722;

fn weighted_factor(hue_degrees: f32, mixer: &BlackAndWhiteMixerAdjustment) -> f32 {
    let bands = [
        mixer.red,
        mixer.orange,
        mixer.yellow,
        mixer.green,
        mixer.aqua,
        mixer.blue,
        mixer.purple,
        mixer.magenta,
    ];
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    for (band, &center) in bands.iter().zip(BAND_CENTERS_DEGREES.iter()) {
        let distance = circular_distance_degrees(hue_degrees, center);
        let weight = gaussian_weight(distance, BAND_SIGMA_DEGREES);
        weighted_sum += weight * band;
        weight_sum += weight;
    }
    if weight_sum <= 1e-6 {
        1.0
    } else {
        (weighted_sum / weight_sum) / 100.0
    }
}

/// Wandelt `pixels` (RGBA8) in Graustufen, jeder Pixel gewichtet nach
/// seinem ursprünglichen Farbton über die acht Bänder in `mixer`.
/// Nur aufrufen, wenn `EdlV4::treatment == Treatment::BlackAndWhite` —
/// bei `Treatment::Color` ist dieser Durchlauf schlicht ein No-Op, den
/// `develop.rs` deshalb ganz überspringt (Regelfall).
pub fn apply_rgba8(pixels: &[u8], mixer: &BlackAndWhiteMixerAdjustment) -> Vec<u8> {
    let mut out = pixels.to_vec();
    for chunk in out.chunks_exact_mut(4) {
        let r = chunk[0] as f32 / 255.0;
        let g = chunk[1] as f32 / 255.0;
        let b = chunk[2] as f32 / 255.0;
        let (hue, _saturation, _lightness) = rgb_to_hsl(r, g, b);
        let factor = weighted_factor(hue, mixer);
        let luma = (LUMA_R * r + LUMA_G * g + LUMA_B * b) * factor;
        let value = (luma.clamp(0.0, 1.0) * 255.0).round() as u8;
        chunk[0] = value;
        chunk[1] = value;
        chunk[2] = value;
        // Alpha (chunk[3]) unverändert.
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(r: u8, g: u8, b: u8) -> Vec<u8> {
        vec![r, g, b, 255]
    }

    #[test]
    fn neutral_mixer_matches_plain_rec709_luminance() {
        let pixels = pixel(200, 100, 50);
        let out = apply_rgba8(&pixels, &BlackAndWhiteMixerAdjustment::NEUTRAL);
        let expected =
            (LUMA_R * 200.0 / 255.0 + LUMA_G * 100.0 / 255.0 + LUMA_B * 50.0 / 255.0) * 255.0;
        assert!((out[0] as f32 - expected).abs() <= 1.0);
        assert_eq!(out[0], out[1]);
        assert_eq!(out[1], out[2]);
    }

    #[test]
    fn output_is_always_neutral_gray() {
        let pixels = pixel(10, 200, 30);
        let out = apply_rgba8(&pixels, &BlackAndWhiteMixerAdjustment::NEUTRAL);
        assert_eq!(out[0], out[1]);
        assert_eq!(out[1], out[2]);
    }

    #[test]
    fn lowering_a_bands_weight_darkens_a_pixel_of_that_hue() {
        // Gauß-Gewichtung über alle acht Bänder (wie beim HSL-Mixer) heißt:
        // die Nachbarbänder (Magenta/Orange) tragen bei reinem Rot (0°)
        // noch ein wenig bei, ein einzelnes Band auf 0 macht das Ergebnis
        // also nicht exakt schwarz — aber deutlich dunkler als neutral.
        let mut mixer = BlackAndWhiteMixerAdjustment::NEUTRAL;
        mixer.red = 0.0;
        let pixels = pixel(255, 0, 0); // reiner Rotton, Farbton 0°
        let neutral_out = apply_rgba8(&pixels, &BlackAndWhiteMixerAdjustment::NEUTRAL);
        let out = apply_rgba8(&pixels, &mixer);
        assert!(out[0] < neutral_out[0]);
    }

    #[test]
    fn doubling_a_bands_weight_roughly_doubles_its_luminance_contribution() {
        let mut mixer = BlackAndWhiteMixerAdjustment::NEUTRAL;
        mixer.red = 200.0;
        let pixels = pixel(255, 0, 0);
        let neutral_out = apply_rgba8(&pixels, &BlackAndWhiteMixerAdjustment::NEUTRAL);
        let doubled_out = apply_rgba8(&pixels, &mixer);
        assert!(doubled_out[0] > neutral_out[0]);
    }

    #[test]
    fn alpha_channel_is_preserved() {
        let pixels = vec![10, 20, 30, 128];
        let out = apply_rgba8(&pixels, &BlackAndWhiteMixerAdjustment::NEUTRAL);
        assert_eq!(out[3], 128);
    }
}
