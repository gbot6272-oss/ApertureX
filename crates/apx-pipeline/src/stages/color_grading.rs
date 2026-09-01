//! Color Grading (Farbräder) — `SPEC.md` §3.2 „Color Grading". Vier
//! Farbräder (Schatten/Mitteltöne/Lichter/Global) tönen das Bild
//! zonenweise ein, gewichtet nach Luminanz — Grundidee wie Lightrooms
//! Color-Grading-Werkzeug, aber vereinfacht (siehe unten).
//!
//! Läuft — wie [`super::hsl_color_mixer`] — im Ein-Pixel-pro-Invocation-
//! Modell auf linearen Kamera-RGB-Werten, direkt danach in `develop.rs`.
//!
//! **Bewusste Vereinfachung:** statt echter, im Editor verschiebbarer
//! Tonwertzonen-Grenzen verwenden Schatten/Mitteltöne/Lichter feste
//! Gauß-gewichtete Zentren bei Luminanz 0/0,5/1 (analog zu
//! `curves.rs`s parametrischer Kurve und `hsl_color_mixer.rs`s
//! Bandgewichtung). `blending` steuert die Breite dieser drei Zonen
//! (höher = breiterer Überlapp), `balance` verschiebt nicht die Zentren
//! selbst, sondern gewichtet Schatten- und Lichter-Einfluss gegeneinander
//! (positiv = mehr Lichter-, weniger Schatten-Einfluss). Jedes Rad tönt
//! additiv: `(Farbton, Sättigung)` wird über [`color_math::hsl_to_rgb`]
//! bei fester Luminanz 0,5 in eine Farbe umgerechnet, deren Abstand zu
//! Grau (0,5) — skaliert mit der jeweiligen Zonen-Gewichtung — auf die
//! drei Kanäle addiert wird. Das globale Rad wirkt immer mit voller
//! Gewichtung, unabhängig von der Zonen-Luminanz.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use super::color_math::{gaussian_weight, hsl_to_rgb};
use crate::edl::v2::{ColorGradingAdjustment, ColorGradingWheel};
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("color_grading.wgsl");

/// Maximaler Farbabstand von Grau bei voller Sättigung eines Rads.
const TINT_STRENGTH: f32 = 0.4;
/// Skala für den Luminanz-Anteil eines Rads — dieselbe Größenordnung wie
/// `whites_blacks.rs`/`curves.rs`s parametrische Zonen.
const LUMINANCE_STRENGTH: f32 = 0.3;
const BASE_SIGMA: f32 = 0.2;
const BLENDING_SIGMA_RANGE: f32 = 0.3;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct WheelParams {
    hue_degrees: f32,
    saturation: f32,
    luminance: f32,
    _pad: f32,
}

impl From<ColorGradingWheel> for WheelParams {
    fn from(wheel: ColorGradingWheel) -> Self {
        Self {
            hue_degrees: wheel.hue_degrees,
            saturation: wheel.saturation,
            luminance: wheel.luminance,
            _pad: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ColorGradingParams {
    shadows: WheelParams,
    midtones: WheelParams,
    highlights: WheelParams,
    global: WheelParams,
    balance: f32,
    blending: f32,
    _pad: [f32; 2],
}

impl ColorGradingParams {
    pub fn new(adjustment: &ColorGradingAdjustment) -> Self {
        Self {
            shadows: adjustment.shadows.into(),
            midtones: adjustment.midtones.into(),
            highlights: adjustment.highlights.into(),
            global: adjustment.global.into(),
            balance: adjustment.balance,
            blending: adjustment.blending,
            _pad: [0.0; 2],
        }
    }
}

/// Additiver Farbton-/Luminanz-Beitrag eines einzelnen Rads, gewichtet
/// mit `weight` (siehe Moduldoku für die Herleitung).
fn wheel_delta(wheel: &WheelParams, weight: f32) -> (f32, f32, f32, f32) {
    let (tr, tg, tb) = hsl_to_rgb(wheel.hue_degrees, wheel.saturation, 0.5);
    let scale = TINT_STRENGTH * weight;
    (
        (tr - 0.5) * scale,
        (tg - 0.5) * scale,
        (tb - 0.5) * scale,
        (wheel.luminance / 100.0) * LUMINANCE_STRENGTH * weight,
    )
}

fn tonal_shift(r: f32, g: f32, b: f32, params: &ColorGradingParams) -> (f32, f32, f32) {
    let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
    let sigma = BASE_SIGMA + (params.blending / 100.0) * BLENDING_SIGMA_RANGE;

    let shadow_factor = (1.0 - params.balance / 200.0).max(0.0);
    let highlight_factor = (1.0 + params.balance / 200.0).max(0.0);

    let shadow_weight = gaussian_weight(luminance, sigma) * shadow_factor;
    let midtone_weight = gaussian_weight(luminance - 0.5, sigma);
    let highlight_weight = gaussian_weight(luminance - 1.0, sigma) * highlight_factor;

    let (mut dr, mut dg, mut db, mut dl) = wheel_delta(&params.shadows, shadow_weight);
    let (dr2, dg2, db2, dl2) = wheel_delta(&params.midtones, midtone_weight);
    let (dr3, dg3, db3, dl3) = wheel_delta(&params.highlights, highlight_weight);
    // Das globale Rad wirkt immer mit voller Gewichtung.
    let (dr4, dg4, db4, dl4) = wheel_delta(&params.global, 1.0);
    dr += dr2 + dr3 + dr4;
    dg += dg2 + dg3 + dg4;
    db += db2 + db3 + db4;
    dl += dl2 + dl3 + dl4;

    (
        (r + dr + dl).clamp(0.0, 1.0),
        (g + dg + dl).clamp(0.0, 1.0),
        (b + db + dl).clamp(0.0, 1.0),
    )
}

/// CPU-Fallback — dieselbe Formel wie `color_grading.wgsl`.
pub fn apply_cpu(pixels: &[f32], adjustment: &ColorGradingAdjustment) -> Vec<f32> {
    let params = ColorGradingParams::new(adjustment);
    pixels
        .par_chunks_exact(3)
        .flat_map_iter(|rgb| {
            let (r, g, b) = tonal_shift(rgb[0], rgb[1], rgb[2], &params);
            [r, g, b]
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    adjustment: &ColorGradingAdjustment,
) -> Result<Vec<f32>> {
    let params = ColorGradingParams::new(adjustment);
    dispatch::run_compute_f32(ctx, "color_grading", SHADER, "main", params, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = vec![0.1, 0.5, 0.9, 0.2, 0.6, 0.8];
        let result = apply_cpu(&pixels, &ColorGradingAdjustment::NEUTRAL);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-4,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn shadow_wheel_tints_dark_pixels_more_than_bright_ones() {
        let adjustment = ColorGradingAdjustment {
            shadows: ColorGradingWheel {
                hue_degrees: 220.0, // bläulich
                saturation: 1.0,
                luminance: 0.0,
            },
            ..ColorGradingAdjustment::NEUTRAL
        };
        let dark = vec![0.05, 0.05, 0.05];
        let bright = vec![0.9, 0.9, 0.9];
        let dark_result = apply_cpu(&dark, &adjustment);
        let bright_result = apply_cpu(&bright, &adjustment);

        // Blau-Tönung sollte den Blau-Kanal relativ zu Rot/Grün anheben —
        // beim dunklen Pixel deutlich stärker als beim hellen.
        let blue_shift =
            |before: &[f32], after: &[f32]| (after[2] - before[2]) - (after[0] - before[0]);
        assert!(
            blue_shift(&dark, &dark_result) > blue_shift(&bright, &bright_result),
            "Schatten-Rad sollte dunkle Pixel stärker tönen als helle"
        );
    }

    #[test]
    fn highlight_wheel_tints_bright_pixels_more_than_dark_ones() {
        let adjustment = ColorGradingAdjustment {
            highlights: ColorGradingWheel {
                hue_degrees: 40.0, // orange
                saturation: 1.0,
                luminance: 0.0,
            },
            ..ColorGradingAdjustment::NEUTRAL
        };
        let dark = vec![0.05, 0.05, 0.05];
        let bright = vec![0.9, 0.9, 0.9];
        let dark_result = apply_cpu(&dark, &adjustment);
        let bright_result = apply_cpu(&bright, &adjustment);

        let red_shift = |before: &[f32], after: &[f32]| after[0] - before[0];
        assert!(
            red_shift(&bright, &bright_result) > red_shift(&dark, &dark_result),
            "Lichter-Rad sollte helle Pixel stärker tönen als dunkle"
        );
    }

    #[test]
    fn global_wheel_tints_regardless_of_luminance() {
        let adjustment = ColorGradingAdjustment {
            global: ColorGradingWheel {
                hue_degrees: 0.0,
                saturation: 1.0,
                luminance: 0.0,
            },
            ..ColorGradingAdjustment::NEUTRAL
        };
        let dark = vec![0.05, 0.05, 0.05];
        let bright = vec![0.9, 0.9, 0.9];
        let dark_result = apply_cpu(&dark, &adjustment);
        let bright_result = apply_cpu(&bright, &adjustment);
        assert!(
            dark_result[0] > dark[0],
            "Globales Rad sollte auch dunkle Pixel tönen"
        );
        assert!(
            bright_result[0] > bright[0],
            "Globales Rad sollte auch helle Pixel tönen"
        );
    }

    #[test]
    fn positive_balance_favors_highlight_influence_over_shadow_influence() {
        let base = ColorGradingWheel {
            hue_degrees: 200.0,
            saturation: 1.0,
            luminance: 0.0,
        };
        let neutral_balance = ColorGradingAdjustment {
            shadows: base,
            highlights: base,
            balance: 0.0,
            ..ColorGradingAdjustment::NEUTRAL
        };
        let positive_balance = ColorGradingAdjustment {
            balance: 80.0,
            ..neutral_balance
        };

        // Absichtlich nicht zu nah an 0/1 — sonst verdeckt das
        // abschließende Clamping den eigentlich zu prüfenden Effekt.
        let dark = vec![0.2, 0.2, 0.2];
        let bright = vec![0.7, 0.7, 0.7];

        let dark_neutral = apply_cpu(&dark, &neutral_balance);
        let dark_positive = apply_cpu(&dark, &positive_balance);
        let bright_neutral = apply_cpu(&bright, &neutral_balance);
        let bright_positive = apply_cpu(&bright, &positive_balance);

        let change = |before: &[f32], after: &[f32]| (before[0] - after[0]).abs(); // Rot-Kanal-Änderung
        let dark_change_drop = change(&dark_neutral, &dark) - change(&dark_positive, &dark);
        let bright_change_gain =
            change(&bright_positive, &bright) - change(&bright_neutral, &bright);

        assert!(
            dark_change_drop > 0.0,
            "Positive Balance sollte den Schatten-Einfluss verringern"
        );
        assert!(
            bright_change_gain > 0.0,
            "Positive Balance sollte den Lichter-Einfluss verstärken"
        );
    }

    #[test]
    fn gpu_matches_cpu() {
        let ctx = match GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };
        let adjustment = ColorGradingAdjustment {
            shadows: ColorGradingWheel {
                hue_degrees: 210.0,
                saturation: 0.6,
                luminance: 10.0,
            },
            midtones: ColorGradingWheel {
                hue_degrees: 90.0,
                saturation: 0.3,
                luminance: -5.0,
            },
            highlights: ColorGradingWheel {
                hue_degrees: 45.0,
                saturation: 0.5,
                luminance: 5.0,
            },
            global: ColorGradingWheel {
                hue_degrees: 300.0,
                saturation: 0.2,
                luminance: 0.0,
            },
            balance: -20.0,
            blending: 70.0,
        };
        let pixels = crate::test_support::ramp(300);
        let cpu = apply_cpu(&pixels, &adjustment);
        let gpu = apply_gpu(&ctx, &pixels, &adjustment).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-3, "CPU={c} GPU={g}");
        }
    }
}
